use color_eyre::{eyre::eyre, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use validator::Validate;

use self::{filters::Filter, strategy::Strategy};

pub mod filters;
pub mod strategy;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub watch_priority: Option<Vec<String>>,
    pub streamers: IndexMap<String, ConfigType>,
    pub presets: Option<IndexMap<String, StreamerConfig>>,
    pub watch_streak: Option<bool>,
    pub analytics_db: Option<String>,
}

pub trait Normalize {
    fn normalize(&mut self);
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Validate)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct StreamerConfig {
    pub follow_raid: bool,
    #[validate(nested)]
    pub prediction: PredictionConfig,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Validate)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
#[validate(nested)]
pub struct PredictionConfig {
    #[validate(nested)]
    pub strategy: Strategy,
    #[validate(length(min = 0))]
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
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
                    s.prediction.strategy.normalize();
                }
            }
        }

        if let Some(p) = self.presets.as_mut() {
            for (key, c) in p {
                if self.streamers.contains_key(key) {
                    return Err(eyre!("Preset {key} already in use as a streamer. Preset names cannot be the same as a streamer mentioned in the config"));
                }

                c.prediction.strategy.normalize();
            }
        }
        Ok(())
    }
}
