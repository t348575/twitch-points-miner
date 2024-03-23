use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub streamers: HashMap<String, Streamer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Streamer {}
