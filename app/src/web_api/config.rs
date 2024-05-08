use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use common::config::{Config, ConfigType, Normalize, StreamerConfig};
use http::StatusCode;
use indexmap::IndexMap;
use serde::Deserialize;
use thiserror::Error;
use twitch_api::types::UserId;
use utoipa::ToSchema;

use crate::{make_paths, pubsub::PubSub, sub_error};

use super::{
    ApiError, ApiState, ConfigTypeRef, RouterBuild, StreamerConfigRef, StreamerConfigRefWrapper,
    WebApiError,
};

pub fn build(state: ApiState) -> RouterBuild {
    let routes = Router::new()
        .route("/presets", get(get_presets))
        .route("/presets/", post(add_update_preset))
        .route("/presets/:name", delete(remove_preset))
        .route("/streamer/:name", post(update_streamer_config))
        .route("/watch_priority", get(get_watch_priority))
        .route("/watch_priority/", post(update_watch_priority))
        .with_state(state);

    let schemas = vec![AddUpdatePreset::schema()];

    let paths = make_paths!(
        __path_get_presets,
        __path_add_update_preset,
        __path_remove_preset,
        __path_get_watch_priority,
        __path_update_watch_priority,
        __path_update_streamer_config
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
    fn make_response(&self) -> axum::response::Response {
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
    let reader = data.read().await;
    let mut presets = IndexMap::new();
    for item in reader.config.presets.clone().unwrap_or_default().keys() {
        presets.insert(
            item.clone(),
            reader
                .configs
                .get(item)
                .unwrap()
                .0
                .read()
                .unwrap()
                .config
                .clone(),
        );
    }
    Json(presets)
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

    let mut preset_normalized = preset.config.clone();
    preset_normalized.prediction.strategy.normalize();
    match writer.configs.get_mut(&preset.name) {
        Some(c) => c.0.write().unwrap().config = preset_normalized,
        None => {
            writer.configs.insert(
                preset.name.clone(),
                StreamerConfigRefWrapper::new(StreamerConfigRef {
                    _type: ConfigTypeRef::Preset(preset.name.clone()),
                    config: preset_normalized,
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
    delete,
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
    writer.config.presets.as_mut().unwrap().shift_remove(&name);
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

#[utoipa::path(
    post,
    path = "/api/config/streamer/{channel_name}",
    responses(
        (status = 200, description = "Successfully updated streamer config"),
        (status = 404, description = "Could not find streamer")
    ),
    params(
        ("channel_name" = String, Path, description = "Name of streamer whose config to update")
    ),
    request_body = ConfigType
)]
async fn update_streamer_config(
    State(data): State<ApiState>,
    Path(channel_name): Path<String>,
    Json(payload): Json<ConfigType>,
) -> Result<(), ApiError> {
    let mut writer = data.write().await;

    let id = match writer.get_id_by_name(&channel_name) {
        Some(s) => UserId::from(s.to_owned()),
        None => return Err(ApiError::StreamerDoesNotExist),
    };

    let config = writer.insert_config(&payload, &channel_name)?;
    writer.streamers.get_mut(&id).unwrap().config = config;
    *writer.config.streamers.get_mut(&channel_name).unwrap() = payload;

    writer.save_config("Update streamer config").await?;

    Ok(())
}

impl PubSub {
    #[allow(private_interfaces)]
    pub fn insert_config(
        &mut self,
        config: &ConfigType,
        channel_name: &str,
    ) -> Result<StreamerConfigRefWrapper, ApiError> {
        match config {
            ConfigType::Preset(name) => match self.configs.get(name) {
                Some(c) => Ok(c.clone()),
                None => sub_error!(ConfigError::PresetConfigDoesNotExist),
            },
            ConfigType::Specific(s) => {
                let mut default = Config::default();
                default
                    .streamers
                    .insert(channel_name.to_owned(), ConfigType::Specific(s.clone()));
                if let Err(err) = default.parse_and_validate() {
                    return sub_error!(ConfigError::InvalidConfig(err.to_string()));
                }

                let s = StreamerConfigRefWrapper::new(StreamerConfigRef {
                    _type: ConfigTypeRef::Specific,
                    config: if let ConfigType::Specific(s) =
                        default.streamers.shift_remove(channel_name).unwrap()
                    {
                        s
                    } else {
                        unreachable!()
                    },
                });
                self.configs
                    .entry(channel_name.to_owned())
                    .or_insert(s.clone());
                Ok(s)
            }
        }
    }
}
