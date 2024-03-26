use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod filters;
pub mod strategy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub streamers: HashMap<String, Streamer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Streamer {
    #[serde(default = "Default::default")]
    #[validate(nested)]
    pub strategy: strategy::Strategy,
    #[validate(length(min = 0))]
    pub filters: Vec<filters::Filter>,
}