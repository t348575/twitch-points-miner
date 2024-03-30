use std::{collections::HashMap, sync::Arc, time::Instant};

use serde::{Deserialize, Serialize, Serializer};
use twitch_api::{pubsub::predictions::Event as PredictionEvent, types::UserId};

use crate::config;

#[derive(Debug, Clone, Serialize)]
pub struct Streamer {
    pub info: StreamerInfo,
    pub predictions: HashMap<String, (PredictionEvent, bool)>,
    pub config: StreamerConfigRefWrapper,
    pub points: u32,
    #[serde(skip)]
    pub last_points_refresh: Instant,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamerConfigRef {
    pub _type: ConfigTypeRef,
    pub config: config::StreamerConfig,
}

#[derive(Debug, Clone)]
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

impl StreamerConfigRefWrapper {
    pub fn new(config: StreamerConfigRef) -> Self {
        Self(Arc::new(std::sync::RwLock::new(config)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigTypeRef {
    Preset,
    Specific,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamerInfo {
    pub broadcast_id: Option<UserId>,
    pub live: bool,
    pub channel_name: String,
    pub game: Option<Game>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteWatched {
    pub channel_id: UserId,
    pub broadcast_id: Option<UserId>,
    pub live: bool,
    /// Channel name string
    pub channel: String,
    pub game: Option<String>,
    pub game_id: Option<String>,
    /// constant, "site"
    pub player: &'static str,
    pub player_state: &'static str,
    /// Login user ID
    pub user_id: u32,
    pub login: String,
}

impl MinuteWatched {
    pub fn from_streamer_info(
        user_name: String,
        user_id: u32,
        channel_id: UserId,
        value: StreamerInfo,
    ) -> Self {
        Self {
            channel_id,
            broadcast_id: value.broadcast_id,
            live: value.live,
            channel: value.channel_name,
            game: value.game.clone().map(|x| x.name),
            game_id: value.game.map(|x| x.id),
            player: "site",
            user_id,
            player_state: "Playing",
            login: user_name,
        }
    }
}
