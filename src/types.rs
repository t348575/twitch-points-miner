use std::{collections::HashMap, time::Instant};

use serde::Serialize;
use twitch_api::{pubsub::predictions::Event as PredictionEvent, types::UserId};

use crate::config::{self, StreamerConfig};

#[derive(Debug, Clone, Serialize)]
pub struct Streamer {
    pub name: String,
    pub live: bool,
    pub predictions: HashMap<String, (PredictionEvent, bool)>,
    pub config: config::StreamerConfig,
    pub points: u32,
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
    pub fn init(x: (UserId, (String, StreamerConfig))) -> StarterInformation {
        StarterInformation {
            user_id: x.0,
            name: x.1 .0,
            config: x.1 .1,
        }
    }
}
