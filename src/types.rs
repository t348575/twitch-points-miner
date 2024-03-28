use std::{collections::HashMap, time::Instant};

use serde::{Deserialize, Serialize};
use twitch_api::{pubsub::predictions::Event as PredictionEvent, types::UserId};

use crate::config::{self, StreamerConfig};

#[derive(Debug, Clone, Serialize)]
pub struct Streamer {
    pub name: String,
    pub live: bool,
    pub predictions: HashMap<String, (PredictionEvent, bool)>,
    pub config: config::StreamerConfig,
    pub points: u32,
    pub broadcast_id: Option<UserId>,
    #[serde(skip)]
    pub last_points_refresh: Instant,
}

#[derive(Debug, Clone)]
pub struct StarterInformation {
    pub user_id: UserId,
    pub name: String,
    pub config: config::StreamerConfig,
}

impl StarterInformation {
    pub fn init(x: (UseLiveReplyUser, (String, StreamerConfig))) -> StarterInformation {
        StarterInformation {
            user_id: x.0.id,
            name: x.1 .0,
            config: x.1 .1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UseLiveReplyUser {
    pub id: UserId,
    pub login: String,
    pub stream: Option<UseLiveReplyStream>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UseLiveReplyStream {
    pub id: UserId,
}
