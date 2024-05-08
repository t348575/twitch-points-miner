use std::thread::spawn;

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
use tracing::{error, trace};

use crate::analytics::model::{PredictionBet, PredictionBetWrapper};

use self::model::{Outcomes, Point, PointsInfo, Prediction, Streamer};

pub mod model;
mod schema;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

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
        if let Some(analytics) = self.0.lock().await.as_mut() {
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
    pub fn new(url: &str) -> Result<(Analytics, Sender<Request>), AnalyticsError> {
        let mut conn = SqliteConnection::establish(url)?;
        let conn_thread = SqliteConnection::establish(url)?;
        _ = conn
            .run_pending_migrations(MIGRATIONS)
            .map_err(AnalyticsError::DbInit);

        let (tx, rx) = flume::unbounded();
        spawn(move || {
            Analytics::run(
                Analytics {
                    conn: Some(conn_thread),
                },
                rx,
            );
        });
        Ok((Analytics { conn: Some(conn) }, tx))
    }

    pub fn run(mut self, rx: Receiver<Request>) {
        while let Ok(data) = rx.recv() {
            trace!("got analytics request");
            if let Err(err) = data(&mut self) {
                error!("{err:#?}");
            }
        }
    }

    pub fn insert_streamer(&mut self, id: i32, name: String) -> Result<bool, AnalyticsError> {
        let res = diesel::insert_into(schema::streamers::table)
            .values(&Streamer {
                id,
                name: name.clone(),
            })
            .execute(self.conn.as_mut().unwrap());
        if let Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) =
            res
        {
            return Ok(false);
        }
        res.map_err(|err| {
            AnalyticsError::from_diesel_error(err, format!("Upsert streamer {name}"))
        })?;
        Ok(true)
    }

    pub fn insert_points(
        &mut self,
        channel_id: i32,
        points_value: i32,
        points_info: PointsInfo,
    ) -> Result<(), AnalyticsError> {
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
        let current_pv: Result<i32, diesel::result::Error> = points
            .filter(channel_id.eq(c_id))
            .order(created_at.desc())
            .select(points_value)
            .first(self.conn.as_mut().unwrap());

        let mut func = || {
            self.insert_points(c_id, pv, pi.clone())?;
            Ok(true)
        };

        match current_pv {
            Ok(current_pv) => {
                if current_pv == pv {
                    Ok(false)
                } else {
                    func()
                }
            }
            Err(err) => match err {
                diesel::result::Error::NotFound => func(),
                err => Err(AnalyticsError::from_diesel_error(
                    err,
                    format!("Insert points if updated {c_id}, {pi:?}"),
                )),
            },
        }
    }

    pub fn upsert_prediction(&mut self, prediction: &Prediction) -> Result<(), AnalyticsError> {
        use schema::predictions::dsl::*;
        let last_prediction_id = predictions
            .filter(channel_id.eq(&prediction.channel_id))
            .order_by(id.desc())
            .select(prediction_id)
            .first::<String>(self.conn.as_mut().unwrap());

        let mut insert_prediction = |prediction: &Prediction| -> Result<(), AnalyticsError> {
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

        trace!("Timeline {from} {to} {channels:?}");
        let query = format!(
            r#"select a.*, a.points_value - LAG(a.points_value) OVER (PARTITION BY a.channel_id ORDER BY a.created_at) AS difference, b.* from points a left join
                predictions b on a.points_info ->> '$.Prediction[0]' == b.prediction_id and a.points_info ->> '$.Prediction[1]' == b.id
                where a.created_at >= '{}' and a.created_at <= '{}' and a.channel_id in ({}) order by a.created_at asc"#,
            from,
            to,
            channels
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let items = sql_query(query)
            .get_results(self.conn.as_mut().unwrap())
            .map_err(|err| AnalyticsError::from_diesel_error(err, format!("Points timeline")))?;
        Ok(items)
    }

    pub fn last_prediction_id(&mut self, c_id: i32, p_id: &str) -> Result<i32, AnalyticsError> {
        use schema::predictions::dsl::*;
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
