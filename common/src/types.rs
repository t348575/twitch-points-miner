use std::{collections::HashMap, sync::Arc, time::Instant};

use serde::{Deserialize, Serialize, Serializer};
use twitch_api::{pubsub::predictions::Event, types::UserId};

use crate::config::StreamerConfig;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct StreamerState {
    pub info: StreamerInfo,
    pub predictions: HashMap<String, (Event, bool)>,
    pub config: StreamerConfigRefWrapper,
    pub points: u32,
    #[serde(skip)]
    pub last_points_refresh: Instant,
}

impl Default for StreamerState {
    fn default() -> Self {
        Self {
            info: Default::default(),
            predictions: Default::default(),
            config: Default::default(),
            points: Default::default(),
            last_points_refresh: Instant::now(),
        }
    }
}

impl StreamerState {
    pub fn new(live: bool, channel_name: String) -> Self {
        StreamerState {
            info: StreamerInfo {
                live,
                channel_name,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct StreamerConfigRef {
    pub _type: ConfigTypeRef,
    pub config: StreamerConfig,
}

#[derive(Debug, Default, Clone)]
pub struct StreamerConfigRefWrapper(pub Arc<std::sync::RwLock<StreamerConfigRef>>);

impl Serialize for StreamerConfigRefWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let data = { self.0.read().map_err(serde::ser::Error::custom)?.clone() };
        serializer.serialize_newtype_struct("StreamerConfigRef", &data)
    }
}

#[cfg(feature = "web_api")]
impl utoipa::PartialSchema for StreamerConfigRefWrapper {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        StreamerConfigRef::schema()
    }
}

#[cfg(feature = "web_api")]
impl utoipa::ToSchema for StreamerConfigRefWrapper {
    fn schemas(
        schemas: &mut Vec<(
            String,
            utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
        )>,
    ) {
        schemas.push((
            "StreamerConfigRefWrapper".to_owned(),
            <StreamerConfigRef as utoipa::PartialSchema>::schema(),
        ));
    }
}

impl StreamerConfigRefWrapper {
    pub fn new(config: StreamerConfigRef) -> Self {
        Self(Arc::new(std::sync::RwLock::new(config)))
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub enum ConfigTypeRef {
    Preset(String),
    #[default]
    Specific,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct StreamerInfo {
    pub broadcast_id: Option<UserId>,
    pub live: bool,
    pub channel_name: String,
    pub game: Option<Game>,
}

impl StreamerInfo {
    pub fn with_channel_name(channel_name: &str) -> Self {
        Self {
            channel_name: channel_name.to_owned(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct Game {
    pub id: String,
    pub name: String,
}
