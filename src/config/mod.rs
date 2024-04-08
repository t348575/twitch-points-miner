use std::collections::HashMap;

use color_eyre::{eyre::eyre, Result};
use serde::{Deserialize, Serialize};
use validator::Validate;

use self::{filters::Filter, strategy::Strategy};

pub mod filters;
pub mod strategy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub streamers: HashMap<String, ConfigType>,
    pub presets: Option<HashMap<String, StreamerConfig>>,
    #[cfg(feature = "analytics")]
    #[serde(default = "defaults::_db_path_default")]
    pub analytics_db: String,
}

pub trait Normalize {
    fn normalize(&mut self);
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct StreamerConfig {
    #[validate(nested)]
    pub strategy: Strategy,
    #[validate(length(min = 0))]
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigType {
    Preset(String),
    Specific(StreamerConfig),
}

impl Config {
    pub fn parse_and_validate(&mut self) -> Result<()> {
        for (_, c) in &mut self.streamers {
            match c {
                ConfigType::Preset(s_name) => {
                    if self.presets.is_none() {
                        return Err(eyre!(
                            "No preset strategies given, so {s_name} cannot be used"
                        ));
                    }

                    let s = self.presets.as_ref().unwrap().get(s_name);
                    if s.is_none() {
                        return Err(eyre!("Preset strategy {s_name} not found"));
                    }
                    s.unwrap().validate()?;
                }
                ConfigType::Specific(s) => {
                    s.validate()?;
                    s.strategy.normalize();
                }
            }
        }

        if let Some(p) = self.presets.as_mut() {
            for (key, c) in p {
                if self.streamers.contains_key(key) {
                    return Err(eyre!("Preset {key} already in use as a streamer. Preset names cannot be the same as a streamer mentioned in the config"));
                }

                c.strategy.normalize();
            }
        }
        Ok(())
    }
}

#[rustfmt::skip]
mod defaults {
    pub fn _db_path_default() -> String { "analytics.db".to_owned() }
}
