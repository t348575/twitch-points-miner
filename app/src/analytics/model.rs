use chrono::NaiveDateTime;
use diesel::{
    deserialize::FromSql,
    prelude::*,
    serialize::{IsNull, ToSql},
    sql_types::Text,
    sqlite::{Sqlite, SqliteValue},
    AsExpression, FromSqlRow,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(
    Queryable, Identifiable, Selectable, Insertable, Debug, PartialEq, Clone, Serialize, Deserialize,
)]
#[diesel(table_name = super::schema::streamers, primary_key(id))]
pub struct Streamer {
    pub id: i32,
    pub name: String,
}

#[derive(
    Queryable,
    Selectable,
    Insertable,
    Debug,
    PartialEq,
    Clone,
    Serialize,
    Deserialize,
    QueryableByName,
    utoipa::ToSchema,
)]
#[diesel(table_name = super::schema::points)]
pub struct Point {
    pub channel_id: i32,
    pub points_value: i32,
    #[diesel(sql_type = Text)]
    pub points_info: PointsInfo,
    pub created_at: NaiveDateTime,
}

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, FromSqlRow, AsExpression, utoipa::ToSchema,
)]
#[diesel(sql_type = Text)]
pub enum PointsInfo {
    FirstEntry,
    Watching,
    CommunityPointsClaimed,
    /// prediction event id
    Prediction(String, i32),
}

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, FromSqlRow, AsExpression, utoipa::ToSchema,
)]
#[diesel(sql_type = Text)]
pub struct Outcomes(pub Vec<Outcome>);

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, FromSqlRow, AsExpression, utoipa::ToSchema,
)]
#[diesel(sql_type = Text)]
pub struct Outcome {
    pub id: String,
    pub title: String,
    pub total_points: i64,
    pub total_users: i64,
}

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, FromSqlRow, AsExpression, utoipa::ToSchema,
)]
#[diesel(sql_type = Text)]
pub enum PredictionBetWrapper {
    None,
    Some(PredictionBet),
}

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, FromSqlRow, AsExpression, utoipa::ToSchema,
)]
#[diesel(sql_type = Text)]
pub struct PredictionBet {
    pub outcome_id: String,
    pub points: u32,
}

#[derive(
    Queryable,
    Selectable,
    Insertable,
    Debug,
    PartialEq,
    Clone,
    Serialize,
    Deserialize,
    QueryableByName,
    utoipa::ToSchema,
)]
#[diesel(table_name = super::schema::predictions, primary_key(id))]
pub struct Prediction {
    pub channel_id: i32,
    pub prediction_id: String,
    pub title: String,
    pub prediction_window: i64,
    #[diesel(sql_type = Text)]
    pub outcomes: Outcomes,
    pub winning_outcome_id: Option<String>,
    #[diesel(sql_type = Text)]
    pub placed_bet: PredictionBetWrapper,
    pub created_at: NaiveDateTime,
    pub closed_at: Option<NaiveDateTime>,
}

impl From<Vec<twitch_api::pubsub::predictions::Outcome>> for Outcomes {
    fn from(value: Vec<twitch_api::pubsub::predictions::Outcome>) -> Self {
        Self(
            value
                .into_iter()
                .map(|x| Outcome {
                    id: x.id,
                    title: x.title,
                    total_points: x.total_points,
                    total_users: x.total_users,
                })
                .collect(),
        )
    }
}

pub fn from_sql<T: DeserializeOwned>(
    bytes: SqliteValue<'_, '_, '_>,
) -> diesel::deserialize::Result<T> {
    let s: String = FromSql::<Text, Sqlite>::from_sql(bytes)?;
    Ok(serde_json::from_str(&s)?)
}

pub fn to_sql<T: Serialize>(
    data: &T,
    out: &mut diesel::serialize::Output<'_, '_, Sqlite>,
) -> diesel::serialize::Result {
    out.set_value(serde_json::to_string(&data)?);
    Ok(IsNull::No)
}

impl FromSql<Text, Sqlite> for PointsInfo {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
        from_sql(bytes)
    }
}

impl ToSql<Text, Sqlite> for PointsInfo {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Sqlite>,
    ) -> diesel::serialize::Result {
        to_sql(self, out)
    }
}

impl FromSql<Text, Sqlite> for Outcomes {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
        from_sql(bytes)
    }
}

impl ToSql<Text, Sqlite> for Outcomes {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Sqlite>,
    ) -> diesel::serialize::Result {
        to_sql(self, out)
    }
}

impl FromSql<Text, Sqlite> for PredictionBetWrapper {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
        from_sql(bytes)
    }
}

impl ToSql<Text, Sqlite> for PredictionBetWrapper {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Sqlite>,
    ) -> diesel::serialize::Result {
        to_sql(self, out)
    }
}
