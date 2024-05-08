use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    analytics::{model::Outcome, AnalyticsWrapper, TimelineResult},
    make_paths,
};

use super::{ApiError, RouterBuild};

pub fn build(analytics: Arc<AnalyticsWrapper>) -> RouterBuild {
    let routes = Router::new()
        .route("/timeline", post(points_timeline))
        .with_state(analytics);

    let schemas = vec![Outcome::schema(), Timeline::schema()];

    let paths = make_paths!(__path_points_timeline);

    (routes, schemas, paths)
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
/// Timeline information, RFC3339 strings
struct Timeline {
    /// GE time
    from: String,
    /// LE time
    to: String,
    /// Channels
    channels: Vec<i32>,
}

#[utoipa::path(
    post,
    path = "/api/analytics/timeline",
    responses(
        (status = 200, description = "Timeline of point information in the specified range", body = Vec<TimelineResult>),
    ),
    request_body = Timeline
)]
async fn points_timeline(
    State(analytics): State<Arc<AnalyticsWrapper>>,
    axum::extract::Json(timeline): axum::extract::Json<Timeline>,
) -> Result<Json<Vec<TimelineResult>>, ApiError> {
    let from = DateTime::from(DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.from)?);
    let to = DateTime::from(DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.to)?);

    let res = analytics
        .execute(|analytics| analytics.timeline(from, to, &timeline.channels))
        .await?;
    Ok(Json(res))
}
