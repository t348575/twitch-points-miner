use std::{thread::spawn, time::Duration};

use chrono::{DateTime, Local, NaiveDateTime};
use diesel::{
    deserialize, result::DatabaseErrorKind, row::NamedRow, sqlite::Sqlite, Connection,
    ConnectionError, ExpressionMethods, QueryDsl, QueryableByName, RunQueryDsl, SqliteConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use flume::{Receiver, Sender};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};

use crate::analytics::model::{PredictionBet, PredictionBetWrapper};

use self::model::{Outcomes, Point, PointsInfo, Prediction, Streamer};

pub mod model;
mod schema;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Timeout for acquiring the analytics mutex lock
const ANALYTICS_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

pub struct AnalyticsWrapper(pub Mutex<Option<Analytics>>);

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("Analytics not initialized")]
    NotInitialized,
    #[error("Could not connect to database: {0}")]
    ConnectionError(ConnectionError),
    #[error("SQL execute error: {0} at {1}")]
    SqlError(diesel::result::Error, String),
    #[error("Could not initialize database: {0}")]
    DbInit(Box<dyn std::error::Error + Send + Sync>),
    #[error("Analytics operation timed out after {0:?}")]
    Timeout(Duration),
}

impl axum::response::IntoResponse for AnalyticsError {
    fn into_response(self) -> axum::response::Response {
        format!("{self:#?}").into_response()
    }
}

impl AnalyticsError {
    fn from_diesel_error(err: diesel::result::Error, context: String) -> AnalyticsError {
        AnalyticsError::SqlError(err, context)
    }
}

impl AnalyticsWrapper {
    pub fn new(analytics: Analytics) -> AnalyticsWrapper {
        AnalyticsWrapper(Mutex::new(Some(analytics)))
    }

    pub async fn execute<F, R>(&self, func: F) -> Result<R, AnalyticsError>
    where
        F: FnOnce(&mut Analytics) -> Result<R, AnalyticsError>,
    {
        // Use timeout to prevent deadlocks from blocking forever
        let lock_result = tokio::time::timeout(ANALYTICS_LOCK_TIMEOUT, self.0.lock()).await;

        let mut guard = match lock_result {
            Ok(guard) => guard,
            Err(_) => {
                error!(
                    "Analytics mutex lock timed out after {:?} - possible deadlock",
                    ANALYTICS_LOCK_TIMEOUT
                );
                return Err(AnalyticsError::Timeout(ANALYTICS_LOCK_TIMEOUT));
            }
        };

        if let Some(analytics) = guard.as_mut() {
            func(analytics)
        } else {
            Err(AnalyticsError::NotInitialized)
        }
    }
}

pub struct Analytics {
    conn: Option<SqliteConnection>,
}

pub type Request = Box<dyn Fn(&mut Analytics) -> Result<(), AnalyticsError> + Send>;

impl Analytics {
    /// Enable WAL mode on the connection for better concurrent read/write access
    fn enable_wal_mode(conn: &mut SqliteConnection) -> Result<(), AnalyticsError> {
        use diesel::sql_query;
        sql_query("PRAGMA journal_mode=WAL")
            .execute(conn)
            .map_err(|err| AnalyticsError::from_diesel_error(err, "Enable WAL mode".to_string()))?;
        // Set busy timeout to 30 seconds to handle lock contention
        sql_query("PRAGMA busy_timeout=30000")
            .execute(conn)
            .map_err(|err| {
                AnalyticsError::from_diesel_error(err, "Set busy timeout".to_string())
            })?;
        Ok(())
    }

    pub fn new(url: &str) -> Result<(Analytics, Sender<Request>), AnalyticsError> {
        info!("Initializing analytics database at {}", url);

        let mut conn = SqliteConnection::establish(url)?;
        let mut conn_thread = SqliteConnection::establish(url)?;

        // Enable WAL mode on both connections for better concurrency
        Self::enable_wal_mode(&mut conn)?;
        Self::enable_wal_mode(&mut conn_thread)?;
        info!("SQLite WAL mode enabled for better concurrent access");

        _ = conn
            .run_pending_migrations(MIGRATIONS)
            .map_err(AnalyticsError::DbInit);
        info!("Database migrations completed");

        let (tx, rx) = flume::unbounded();
        spawn(move || {
            info!("Analytics background thread started");
            Analytics::run(
                Analytics {
                    conn: Some(conn_thread),
                },
                rx,
            );
            warn!("Analytics background thread exited");
        });
        Ok((Analytics { conn: Some(conn) }, tx))
    }

    pub fn run(mut self, rx: Receiver<Request>) {
        while let Ok(data) = rx.recv() {
            trace!("Processing analytics request");
            if let Err(err) = data(&mut self) {
                error!("Analytics operation failed: {err:#?}");
            }
        }
        error!("Analytics channel closed unexpectedly");
    }

    pub fn insert_streamer(&mut self, id: i32, name: String) -> Result<bool, AnalyticsError> {
        debug!("Adding streamer to database: {} (id: {})", name, id);
        let res = diesel::insert_into(schema::streamers::table)
            .values(&Streamer {
                id,
                name: name.clone(),
            })
            .execute(self.conn.as_mut().unwrap());
        if let Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) =
            res
        {
            trace!("Streamer {} already exists in database", name);
            return Ok(false);
        }
        res.map_err(|err| {
            AnalyticsError::from_diesel_error(err, format!("Upsert streamer {name}"))
        })?;
        info!("New streamer added to database: {} (id: {})", name, id);
        Ok(true)
    }

    pub fn insert_points(
        &mut self,
        channel_id: i32,
        points_value: i32,
        points_info: PointsInfo,
    ) -> Result<(), AnalyticsError> {
        debug!(
            "Recording points: channel={}, points={}, reason={:?}",
            channel_id, points_value, points_info
        );
        diesel::insert_into(schema::points::table)
            .values(&Point {
                channel_id,
                points_value,
                points_info: points_info.clone(),
                created_at: Local::now().naive_local(),
            })
            .execute(self.conn.as_mut().unwrap())
            .map_err(|err| {
                AnalyticsError::from_diesel_error(
                    err,
                    format!("Insert points for {channel_id} {points_info:?}"),
                )
            })?;
        Ok(())
    }

    pub fn insert_points_if_updated(
        &mut self,
        c_id: i32,
        pv: i32,
        pi: PointsInfo,
    ) -> Result<bool, AnalyticsError> {
        use schema::points::dsl::*;
        trace!(
            "Checking if points changed: channel={}, new_value={}",
            c_id, pv
        );
        let current_pv: Result<i32, diesel::result::Error> = points
            .filter(channel_id.eq(c_id))
            .order(created_at.desc())
            .select(points_value)
            .first(self.conn.as_mut().unwrap());

        let insert_new = || {
            debug!(
                "Points changed for channel {}: {} -> {} ({:?})",
                c_id,
                current_pv.as_ref().ok().copied().unwrap_or(0),
                pv,
                pi
            );
            self.insert_points(c_id, pv, pi.clone())?;
            Ok(true)
        };

        match current_pv {
            Ok(current_pv) => {
                if current_pv == pv {
                    trace!("Points unchanged for channel {} (still {})", c_id, pv);
                    Ok(false)
                } else {
                    insert_new()
                }
            }
            Err(err) => match err {
                diesel::result::Error::NotFound => {
                    debug!("First points entry for channel {}: {}", c_id, pv);
                    self.insert_points(c_id, pv, pi.clone())?;
                    Ok(true)
                }
                err => Err(AnalyticsError::from_diesel_error(
                    err,
                    format!("Insert points if updated {c_id}, {pi:?}"),
                )),
            },
        }
    }

    pub fn upsert_prediction(&mut self, prediction: &Prediction) -> Result<(), AnalyticsError> {
        use schema::predictions::dsl::*;
        trace!(
            "Upserting prediction {} for channel {}",
            prediction.prediction_id,
            prediction.channel_id
        );
        let last_prediction_id = predictions
            .filter(channel_id.eq(&prediction.channel_id))
            .order_by(id.desc())
            .select(prediction_id)
            .first::<String>(self.conn.as_mut().unwrap());

        let insert_prediction = |prediction: &Prediction| -> Result<(), AnalyticsError> {
            debug!(
                "Recording new prediction: {} - \"{}\"",
                prediction.prediction_id, prediction.title
            );
            diesel::insert_into(schema::predictions::table)
                .values(prediction)
                .execute(self.conn.as_mut().unwrap())
                .map_err(|err| {
                    AnalyticsError::from_diesel_error(
                        err,
                        format!("Create prediction {prediction:?}"),
                    )
                })?;
            Ok(())
        };

        match last_prediction_id {
            Ok(last_prediction_id) => {
                if last_prediction_id != prediction.prediction_id {
                    insert_prediction(prediction)
                } else {
                    trace!("Prediction {} already recorded", prediction.prediction_id);
                    Ok(())
                }
            }
            Err(err) => match err {
                diesel::result::Error::NotFound => insert_prediction(prediction),
                err => {
                    return Err(AnalyticsError::from_diesel_error(
                        err,
                        format!("Upsert prediction {prediction:?}"),
                    ))
                }
            },
        }
    }

    pub fn place_bet(
        &mut self,
        p_id: &str,
        c_id: i32,
        o_id: &str,
        p: u32,
    ) -> Result<(), AnalyticsError> {
        use schema::predictions::dsl::*;
        debug!(
            "Recording bet: prediction={}, channel={}, outcome={}, points={}",
            p_id, c_id, o_id, p
        );
        diesel::update(predictions)
            .filter(channel_id.eq(c_id))
            .filter(prediction_id.eq(p_id))
            .set(placed_bet.eq(PredictionBetWrapper::Some(PredictionBet {
                outcome_id: o_id.to_owned(),
                points: p,
            })))
            .execute(self.conn.as_mut().unwrap())
            .map_err(|err| {
                AnalyticsError::from_diesel_error(err, format!("Place bet on {c_id} event {p_id}"))
            })?;
        Ok(())
    }

    pub fn end_prediction(
        &mut self,
        p_id: &str,
        c_id: i32,
        w_o_id: Option<String>,
        o_s: Outcomes,
        c_at: NaiveDateTime,
    ) -> Result<(), AnalyticsError> {
        use schema::predictions::dsl::*;
        debug!(
            "Ending prediction: id={}, channel={}, winner={:?}",
            p_id, c_id, w_o_id
        );
        diesel::update(predictions)
            .filter(channel_id.eq(c_id))
            .filter(prediction_id.eq(p_id))
            .set((
                winning_outcome_id.eq(w_o_id),
                outcomes.eq(o_s),
                closed_at.eq(Some(c_at)),
            ))
            .execute(self.conn.as_mut().unwrap())
            .map_err(|err| {
                AnalyticsError::from_diesel_error(
                    err,
                    format!("End prediction on {c_id} event {p_id}"),
                )
            })?;
        Ok(())
    }

    pub fn timeline(
        &mut self,
        from: DateTime<Local>,
        to: DateTime<Local>,
        channels: &[i32],
    ) -> Result<Vec<TimelineResult>, AnalyticsError> {
        use diesel::sql_query;

        // Convert to NaiveDateTime to match the database storage format (no timezone)
        let from_naive = from.naive_local();
        let to_naive = to.naive_local();
        debug!(
            "Querying timeline: {} to {} for {} channels",
            from_naive,
            to_naive,
            channels.len()
        );
        let query = format!(
            r#"select a.*, a.points_value - LAG(a.points_value) OVER (PARTITION BY a.channel_id ORDER BY a.created_at) AS difference, b.* from points a left join
                predictions b on a.points_info ->> '$.Prediction[0]' == b.prediction_id and a.points_info ->> '$.Prediction[1]' == b.id
                where a.created_at >= '{}' and a.created_at <= '{}' and a.channel_id in ({}) order by a.created_at asc"#,
            from_naive,
            to_naive,
            channels
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let items: Vec<TimelineResult> = sql_query(query)
            .get_results(self.conn.as_mut().unwrap())
            .map_err(|err| AnalyticsError::from_diesel_error(err, format!("Points timeline")))?;
        trace!("Timeline query returned {} results", items.len());
        Ok(items)
    }

    pub fn last_prediction_id(&mut self, c_id: i32, p_id: &str) -> Result<i32, AnalyticsError> {
        use schema::predictions::dsl::*;
        trace!("Looking up prediction entry: channel={}, prediction={}", c_id, p_id);
        let entry_id = predictions
            .filter(channel_id.eq(c_id))
            .filter(prediction_id.eq(p_id))
            .order(created_at.desc())
            .select(id)
            .first(self.conn.as_mut().unwrap())
            .map_err(|err| {
                AnalyticsError::from_diesel_error(err, format!("Last prediction by ID"))
            })?;
        Ok(entry_id)
    }

    pub fn get_live_prediction(
        &mut self,
        c_id: i32,
        p_id: &str,
    ) -> Result<Option<Prediction>, AnalyticsError> {
        use diesel::SelectableHelper;
        use schema::predictions::dsl::*;
        let res = predictions
            .filter(channel_id.eq(c_id))
            .filter(prediction_id.eq(p_id))
            .order(id.desc())
            .select(Prediction::as_select())
            .first(self.conn.as_mut().unwrap());
        match res {
            Ok(res) => Ok(Some(res)),
            Err(err) => match err {
                diesel::result::Error::NotFound => Ok(None),
                err => Err(AnalyticsError::from_diesel_error(
                    err,
                    format!("Get live prediction {c_id}, {p_id}"),
                )),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TimelineResult {
    point: Point,
    difference: Option<i32>,
    prediction: Option<Prediction>,
}

impl QueryableByName<Sqlite> for TimelineResult {
    fn build<'a>(row: &impl NamedRow<'a, Sqlite>) -> deserialize::Result<Self> {
        let prediction = match <Prediction as diesel::QueryableByName<Sqlite>>::build(row) {
            Ok(p) => Some(p),
            Err(_) => None,
        };
        let point = <Point as diesel::QueryableByName<Sqlite>>::build(row)?;
        let difference = {
            let field = diesel::row::NamedRow::get(row, "difference")?;
            <Option<i32> as Into<Option<i32>>>::into(field)
        };
        Ok(Self {
            point,
            prediction,
            difference,
        })
    }
}

impl std::fmt::Debug for AnalyticsWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AnalyticsWrapper").finish()
    }
}

impl From<ConnectionError> for AnalyticsError {
    fn from(value: ConnectionError) -> Self {
        AnalyticsError::ConnectionError(value)
    }
}
