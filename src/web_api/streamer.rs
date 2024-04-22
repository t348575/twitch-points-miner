use std::{collections::HashMap, sync::Arc, time::Instant};

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, put},
    Extension, Json, Router,
};

#[cfg(feature = "analytics")]
use color_eyre::eyre::Context;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use twitch_api::{pubsub::predictions::Event, types::UserId};
use utoipa::ToSchema;

use crate::{
    config::ConfigType,
    make_paths, sub_error,
    twitch::{auth::Token, gql},
    types::*,
};

use super::{ApiError, ApiState, RouterBuild, WebApiError};

pub fn build(state: ApiState, token: Arc<Token>) -> RouterBuild {
    let routes = Router::new()
        .route("/live", get(live_streamers))
        .route("/mine/:streamer", put(mine_streamer))
        .route("/mine/:streamer/", delete(remove_streamer))
        .route("/:streamer", get(streamer))
        .layer(Extension(token))
        .with_state(state);

    let schemas = vec![
        MineStreamer::schema(),
        ConfigType::schema(),
        LiveStreamer::schema(),
    ];

    let paths = make_paths!(
        __path_streamer,
        __path_live_streamers,
        __path_mine_streamer,
        __path_remove_streamer
    );

    (routes, schemas, paths)
}

#[derive(Debug, Error)]
pub enum StreamerError {
    #[error("Streamer is already being mined")]
    StreamerAlreadyMined,
}

impl WebApiError for StreamerError {
    fn make_response(&self) -> axum::response::Response {
        use StreamerError::*;
        let status_code = match self {
            StreamerAlreadyMined => StatusCode::CONFLICT,
        };

        (status_code, self.to_string()).into_response()
    }
}

#[utoipa::path(
    get,
    path = "/api/streamers/{streamer}",
    responses(
        (status = 200, description = "Get the entire application state information", body = [StreamerState]),
        (status = 404, description = "Could not find streamer")
    ),
    params(
        ("streamer" = String, Path, description = "Name of streamer to get state for")
    )
)]
async fn streamer(State(data): State<ApiState>, Path(streamer): Path<String>) -> impl IntoResponse {
    let data = data.read().await;
    match data.get_by_name(streamer.as_str()) {
        Some(s) => Json(s.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "Streamer not found").into_response(),
    }
}

#[derive(Serialize, ToSchema)]
struct LiveStreamer {
    id: i32,
    state: StreamerState,
}

#[utoipa::path(
    get,
    path = "/api/streamers/live",
    responses(
        (status = 200, description = "List of live streamers and their state", body = Vec<LiveStreamer>)
    )
)]
async fn live_streamers(State(data): State<ApiState>) -> Json<Vec<LiveStreamer>> {
    let data = data.read().await;
    let items = data
        .streamers
        .iter()
        .filter(|x| x.1.info.live)
        .map(|x| LiveStreamer {
            id: x.0.as_str().parse().unwrap(),
            state: x.1.clone(),
        })
        .collect::<Vec<_>>();
    Json(items)
}

#[derive(Deserialize, ToSchema)]
struct MineStreamer {
    config: ConfigType,
}

#[utoipa::path(
    put,
    path = "/api/streamers/mine/{channel_name}",
    responses(
        (status = 200, description = "Add streamer to mine", body = ()),
    ),
    params(
        ("channel_name" = String, Path, description = "Name of streamer to watch")
    ),
    request_body = MineStreamer
)]
#[axum::debug_handler]
async fn mine_streamer(
    State(data): State<ApiState>,
    Path(channel_name): Path<String>,
    Extension(token): Extension<Arc<Token>>,
    Json(payload): Json<MineStreamer>,
) -> Result<(), ApiError> {
    let res = gql::streamer_metadata(&[&channel_name], &token.access_token)
        .await
        .map_err(ApiError::twitch_api_error)?;
    if res.is_empty() || (!res.is_empty() && res[0].is_none()) {
        return Err(ApiError::StreamerDoesNotExist);
    }

    let mut writer = data.write().await;
    if writer
        .streamers
        .contains_key(&UserId::from(channel_name.clone()))
    {
        return sub_error!(StreamerError::StreamerAlreadyMined);
    }

    let config = writer.insert_config(&payload.config, &channel_name)?;

    let streamer = res[0].clone().unwrap();
    async fn rollback_steps(
        channel_name: &str,
        access_token: &str,
    ) -> Result<(u32, Vec<(Event, bool)>), ApiError> {
        let points = gql::get_channel_points(&[channel_name], access_token)
            .await
            .map_err(ApiError::twitch_api_error)?[0]
            .0;
        let active_predictions = gql::channel_points_context(&[channel_name], access_token)
            .await
            .map_err(ApiError::twitch_api_error)?[0]
            .clone();
        Ok((points, active_predictions))
    }

    // rollback if any config was added, and an error occurred after
    let (points, active_predictions) =
        match rollback_steps(&channel_name, &token.access_token).await {
            Ok(s) => s,
            Err(err) => {
                if let ConfigType::Specific(_) = &payload.config {
                    writer.configs.remove(&channel_name);
                }
                return Err(err);
            }
        };

    writer.config.streamers.insert(channel_name, payload.config);
    writer.streamers.insert(
        streamer.0.clone(),
        StreamerState {
            config,
            info: streamer.1.clone(),
            predictions: active_predictions
                .into_iter()
                .map(|x| (x.0.channel_id.clone(), x))
                .collect::<HashMap<_, _>>(),
            points,
            last_points_refresh: Instant::now(),
        },
    );

    writer.save_config("Mine streamer").await?;
    writer.restart_live_watcher();

    #[cfg(feature = "analytics")]
    {
        let id = streamer
            .0
            .as_str()
            .parse()
            .context("Parse streamer id")
            .map_err(ApiError::internal_error)?;
        let inserted = writer
            .analytics
            .execute(|analytics| analytics.insert_streamer(id, streamer.1.channel_name))
            .await?;
        if inserted {
            writer
                .analytics
                .execute(|analytics| {
                    analytics.insert_points(
                        id,
                        points as i32,
                        crate::analytics::model::PointsInfo::FirstEntry,
                    )
                })
                .await?;
        }
    }

    Ok(())
}

#[utoipa::path(
    delete,
    path = "/api/streamers/mine/{channel_name}/",
    responses(
        (status = 200, description = "Successfully removed streamer from the mine list"),
        (status = 404, description = "Could not find streamer")
    ),
    params(
        ("channel_name" = String, Path, description = "Name of streamer to delete")
    )
)]
async fn remove_streamer(
    State(data): State<ApiState>,
    Path(channel_name): Path<String>,
) -> Result<(), ApiError> {
    let mut writer = data.write().await;

    let id = match writer.get_id_by_name(&channel_name) {
        Some(s) => UserId::from(s.to_owned()),
        None => return Err(ApiError::StreamerDoesNotExist),
    };

    writer.streamers.remove(&id);
    writer.config.streamers.shift_remove(&channel_name);
    writer.configs.remove(&channel_name);

    writer.save_config("Remove streamer").await?;
    writer.restart_live_watcher();
    Ok(())
}
