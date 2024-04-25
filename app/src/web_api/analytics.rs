use std::sync::Arc;

use axum::{extract::State, routing::post, Extension, Json, Router};
use common::twitch::auth::Token;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    analytics::{model::Outcome, TimelineResult},
    make_paths,
};

use super::{ApiError, ApiState, RouterBuild};

pub fn build(state: ApiState, token: Arc<Token>) -> RouterBuild {
    let routes = Router::new()
        .route("/timeline", post(points_timeline))
        .layer(Extension(token))
        .with_state(state);

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
    State(data): State<ApiState>,
    axum::extract::Json(timeline): axum::extract::Json<Timeline>,
) -> Result<Json<Vec<TimelineResult>>, ApiError> {
    use chrono::FixedOffset;

    let from = chrono::DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.from)?.naive_local();
    let to = chrono::DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.to)?.naive_local();

    let writer = data.write().await;
    let res = writer
        .analytics
        .execute(|analytics| analytics.timeline(from, to, &timeline.channels))
        .await?;
    Ok(Json(res))
}
