use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use http::StatusCode;
use indexmap::IndexMap;
use serde::Deserialize;
use thiserror::Error;
use utoipa::ToSchema;
use validator::Validate;

use crate::{config::StreamerConfig, make_paths, sub_error};

use super::{
    ApiError, ApiState, ConfigTypeRef, RouterBuild, StreamerConfigRef, StreamerConfigRefWrapper,
    WebApiError,
};

pub fn build(state: ApiState) -> RouterBuild {
    let routes = Router::new()
        .route("/presets", get(get_presets))
        .route("/presets/", post(add_update_preset))
        .route("/presets/:name", delete(remove_preset))
        .route("/watch_priority", get(get_watch_priority))
        .route("/watch_priority/", post(update_watch_priority))
        .with_state(state);

    let schemas = vec![];

    let paths = make_paths!(
        __path_get_presets,
        __path_add_update_preset,
        __path_remove_preset,
        __path_get_watch_priority,
        __path_update_watch_priority
    );

    (routes, schemas, paths)
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Preset config does not exist")]
    PresetConfigDoesNotExist,
    #[error("Preset config name cannot be the same as a streamer name")]
    PresetConfigNameEqualsStreamerName,
    #[error("Preset configuration is in use by streamer: {0}")]
    PresetConfigInUse(String),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
}

impl WebApiError for ConfigError {
    fn into_response(&self) -> axum::response::Response {
        use ConfigError::*;
        let status_code = match self {
            PresetConfigNameEqualsStreamerName | PresetConfigDoesNotExist => {
                StatusCode::BAD_REQUEST
            }
            InvalidConfig(_) => StatusCode::BAD_REQUEST,
            PresetConfigInUse(_) => StatusCode::BAD_REQUEST,
        };

        (status_code, self.to_string()).into_response()
    }
}

#[utoipa::path(
    get,
    path = "/api/config/presets",
    responses(
        (status = 200, description = "Get all preset configurations", body = [HashMap<String, StreamerConfig>]),
    )
)]
async fn get_presets(State(data): State<ApiState>) -> Json<IndexMap<String, StreamerConfig>> {
    Json(data.read().await.config.presets.clone().unwrap_or_default())
}

#[derive(Deserialize, ToSchema)]
struct AddUpdatePreset {
    name: String,
    config: StreamerConfig,
}

#[utoipa::path(
    post,
    path = "/api/config/presets/",
    responses(
        (status = 200, description = "Successfully created a preset configuration"),
    ),
    request_body = AddUpdatePreset
)]
async fn add_update_preset(
    State(data): State<ApiState>,
    Json(preset): Json<AddUpdatePreset>,
) -> Result<(), ApiError> {
    preset
        .config
        .validate()
        .map_err(|err| ApiError::SubError(Box::new(ConfigError::InvalidConfig(err.to_string()))))?;

    let mut writer = data.write().await;
    if writer.get_by_name(&preset.name).is_some() {
        return sub_error!(ConfigError::PresetConfigNameEqualsStreamerName);
    }

    match writer.configs.get_mut(&preset.name) {
        Some(c) => c.0.write().unwrap().config = preset.config.clone(),
        None => {
            writer.configs.insert(
                preset.name.clone(),
                StreamerConfigRefWrapper::new(StreamerConfigRef {
                    _type: ConfigTypeRef::Preset(preset.name.clone()),
                    config: preset.config.clone(),
                }),
            );
        }
    }

    match writer.config.presets.as_mut() {
        Some(s) => {
            s.insert(preset.name, preset.config);
        }
        None => writer.config.presets = Some(IndexMap::from([(preset.name, preset.config)])),
    }

    writer.save_config("Update preset").await?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/config/presets/{name}",
    responses(
        (status = 200, description = "Successfully removed the preset configuration"),
    ),
    params(
        ("name" = String, Path, description = "Name of the preset to delete")
    )
)]
async fn remove_preset(
    State(data): State<ApiState>,
    Path(name): Path<String>,
) -> Result<(), ApiError> {
    let mut writer = data.write().await;

    if !writer.configs.contains_key(&name) {
        return sub_error!(ConfigError::PresetConfigDoesNotExist);
    }

    for item in writer.streamers.values() {
        if item.config.0.read().unwrap()._type == ConfigTypeRef::Preset(name.clone()) {
            return sub_error!(ConfigError::PresetConfigInUse(
                item.info.channel_name.clone()
            ));
        }
    }

    writer.configs.remove(&name);
    writer.save_config("Remove preset").await?;
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/config/watch_priority",
    responses(
        (status = 200, description = "Successfully removed the preset configuration", body = Vec<String>),
    )
)]
async fn get_watch_priority(State(data): State<ApiState>) -> Json<Vec<String>> {
    Json(
        data.read()
            .await
            .config
            .watch_priority
            .clone()
            .unwrap_or_default(),
    )
}

#[utoipa::path(
    post,
    path = "/api/config/watch_priority/",
    responses(
        (status = 200, description = "Successfully created a preset configuration"),
    ),
    request_body = Vec<String>
)]
async fn update_watch_priority(
    State(data): State<ApiState>,
    Json(priority): Json<Vec<String>>,
) -> Result<(), ApiError> {
    let mut writer = data.write().await;

    for item in &priority {
        if writer.get_by_name(item).is_none() {
            return Err(ApiError::StreamerDoesNotExist);
        }
    }

    writer.config.watch_priority = Some(priority);
    writer.save_config("Update watch priority").await?;
    Ok(())
}
