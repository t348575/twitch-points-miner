use chrono::{Local, NaiveDateTime};
use diesel::{
    result::DatabaseErrorKind, Connection, ConnectionError, ExpressionMethods, QueryDsl,
    RunQueryDsl, SqliteConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::analytics::model::PredictionBet;

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
    #[error("SQL execute error: {0}")]
    SqlError(diesel::result::Error),
    #[error("Could not initialize database: {0}")]
    DbInit(Box<dyn std::error::Error + Send + Sync>),
}

#[cfg(feature = "web_api")]
impl axum::response::IntoResponse for AnalyticsError {
    fn into_response(self) -> axum::response::Response {
        format!("{self:#?}").into_response()
    }
}

impl AnalyticsWrapper {
    pub fn new(url: &str) -> Result<AnalyticsWrapper, AnalyticsError> {
        Ok(AnalyticsWrapper(Mutex::new(Some(Analytics::new(url)?))))
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
    conn: SqliteConnection,
}

impl Analytics {
    pub fn new(url: &str) -> Result<Analytics, AnalyticsError> {
        let mut conn = SqliteConnection::establish(&url)?;
        _ = conn
            .run_pending_migrations(MIGRATIONS)
            .map_err(|x| AnalyticsError::DbInit(x));
        Ok(Analytics { conn })
    }

    pub fn upsert_streamer(&mut self, id: i32, name: String) -> Result<(), AnalyticsError> {
        let res = diesel::insert_into(schema::streamers::table)
            .values(&Streamer { id, name })
            .execute(&mut self.conn);
        if let Err(diesel::result::Error::DatabaseError(kind, _)) = res {
            if let DatabaseErrorKind::UniqueViolation = kind {
                return Ok(());
            }
        }
        res?;
        Ok(())
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
                points_info,
                created_at: Local::now().naive_local(),
            })
            .execute(&mut self.conn)?;
        Ok(())
    }

    pub fn insert_points_if_updated(
        &mut self,
        c_id: i32,
        pv: i32,
        pi: PointsInfo,
    ) -> Result<(), AnalyticsError> {
        use schema::points::dsl::*;
        let current_pv: i32 = points
            .filter(channel_id.eq(c_id))
            .order(created_at.desc())
            .select(points_value)
            .first(&mut self.conn)?;
        if current_pv == pv {
            return Ok(());
        }

        self.insert_points(c_id, pv, pi)
    }

    pub fn create_prediction(&mut self, prediction: &Prediction) -> Result<(), AnalyticsError> {
        diesel::insert_into(schema::predictions::table)
            .values(prediction)
            .execute(&mut self.conn)?;
        Ok(())
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
            .set(placed_bet.eq(PredictionBet {
                outcome_id: o_id.to_owned(),
                points: p,
            }))
            .execute(&mut self.conn)?;
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
            .execute(&mut self.conn)?;
        Ok(())
    }

    #[cfg(feature = "web_api")]
    pub fn points_timeline(
        &mut self,
        from: NaiveDateTime,
        to: NaiveDateTime,
    ) -> Result<Vec<Point>, AnalyticsError> {
        use diesel::SelectableHelper;
        use schema::points::dsl::*;
        let items = points
            .filter(created_at.ge(from))
            .filter(created_at.le(to))
            .order(created_at.asc())
            .select(Point::as_select())
            .load(&mut self.conn)?;
        Ok(items)
    }

    #[cfg(feature = "web_api")]
    pub fn predictions_timeline(
        &mut self,
        from: NaiveDateTime,
        to: NaiveDateTime,
    ) -> Result<Vec<Prediction>, AnalyticsError> {
        use diesel::SelectableHelper;
        use schema::predictions::dsl::*;
        let items = predictions
            .filter(created_at.ge(from))
            .filter(created_at.le(to))
            .order(created_at.asc())
            .select(Prediction::as_select())
            .load(&mut self.conn)?;
        Ok(items)
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

impl From<diesel::result::Error> for AnalyticsError {
    fn from(value: diesel::result::Error) -> Self {
        AnalyticsError::SqlError(value)
    }
}
