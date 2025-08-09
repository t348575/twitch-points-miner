use std::path::PathBuf;

use eyre::{eyre, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use twitch_api::{
    pubsub::predictions::{Event, Outcome},
    types::Timestamp,
};
use validator::{Validate, ValidateArgs, ValidationError, ValidationErrors};

use crate::{execute_js, types::StreamerState};

use self::{filters::Filter, strategy::Strategy};

pub mod filters;
pub mod strategy;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub watch_priority: Option<Vec<String>>,
    pub streamers: IndexMap<String, ConfigType>,
    pub presets: Option<IndexMap<String, StreamerConfig>>,
    pub watch_streak: Option<bool>,
}

pub trait Normalize {
    fn normalize(&mut self);
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct StreamerConfig {
    pub follow_raid: bool,
    pub prediction: PredictionConfig,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct PredictionConfig {
    pub strategy: Strategy,
    pub filters: Vec<Filter>,
}

impl<'v_a> ValidateArgs<'v_a> for PredictionConfig {
    type Args = &'v_a PathBuf;
    fn validate_with_args(&self, args: Self::Args) -> Result<(), ValidationErrors> {
        use validator::ValidateLength;
        let mut errors = ValidationErrors::new();

        errors.merge_self("strategy", validate_strategy(&self.strategy, args));
        if !self.filters.validate_length(Some(0), None, None) {
            let mut err = ValidationError::new("length");
            err.add_param("min".into(), &0);
            err.add_param("value".into(), &self.filters);
            errors.add("filters", err);
        }
        errors.merge_self("filters", validate_filters(&self.filters, args));

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub enum ConfigType {
    Preset(String),
    Specific(StreamerConfig),
}

#[cfg(feature = "web_api")]
impl ConfigType {
    pub fn name_schema() -> (
        std::borrow::Cow<'static, str>,
        utoipa::openapi::RefOr<utoipa::openapi::Schema>,
    ) {
        use utoipa::{PartialSchema, ToSchema};
        (Self::name(), Self::schema())
    }
}

impl Config {
    pub fn parse_and_validate(&mut self, js_dir: &PathBuf) -> Result<()> {
        for (streamer, c) in &mut self.streamers {
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
                    if let Err(err) = s.unwrap().prediction.validate_with_args(js_dir) {
                        println!("Config for preset {s_name} failed validation:");
                        Err(err)?;
                    }
                }
                ConfigType::Specific(s) => {
                    if let Err(err) = s.prediction.validate_with_args(js_dir) {
                        println!("Config for streamer {streamer} failed validation:");
                        Err(err)?;
                    }
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
#[validate(context = "ExternalContext<'v_a>")]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub struct External {
    #[serde(rename = "type")]
    pub _type: ExternalType,
    #[validate(custom(function = "validate_external", use_context), length(min = 1))]
    pub data: String,
}

impl External {
    pub fn attach_dir(&mut self, js_dir: &PathBuf) {
        if self._type == ExternalType::File {
            self.data = js_dir.join(&self.data).to_str().unwrap().to_owned();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub enum ExternalType {
    #[default]
    Inline,
    File,
}

pub struct ExternalContext<'a> {
    _type: &'a ExternalType,
    js_dir: &'a PathBuf,
}

fn validate_external(value: &String, context: &ExternalContext) -> Result<(), ValidationError> {
    use validator::ValidateLength;
    match context._type {
        ExternalType::Inline => validate_js(External {
            _type: context._type.to_owned(),
            data: value.clone(),
        }),
        ExternalType::File => {
            let real_path = context.js_dir.join(value);
            if !std::path::Path::new(&real_path).exists() {
                let mut err = ValidationError::new("invalid-path");
                err.add_param("value".into(), &value);
                return Err(err);
            }

            let js = std::fs::read_to_string(&real_path)
                .map_err(|err| {
                    let mut e = ValidationError::new("read-error");
                    e.add_param("value".into(), &value);
                    e.with_message(err.to_string().into())
                })?
                .trim()
                .to_owned();

            if !js.validate_length(Some(1), None, None) {
                let mut err = ValidationError::new("length");
                err.add_param("min".into(), &0);
                err.add_param("value".into(), &value);
                return Err(err);
            }

            // safe to unwrap since this was checked earlier
            validate_js(External {
                _type: context._type.to_owned(),
                data: real_path.to_str().unwrap().to_string(),
            })
        }
    }
}

fn validate_js(external: External) -> Result<(), ValidationError> {
    execute_js(
        &StreamerState::default(),
        &Event {
            id: "1".to_owned(),
            channel_id: "2".to_owned(),
            created_at: Timestamp::now(),
            ended_at: None,
            locked_at: None,
            outcomes: vec![
                Outcome {
                    id: "1".to_owned(),
                    title: "a".to_owned(),
                    total_points: 0,
                    total_users: 0,
                    top_predictors: vec![],
                    color: "PINK".to_owned(),
                },
                Outcome {
                    id: "2".to_owned(),
                    title: "b".to_owned(),
                    total_points: 0,
                    total_users: 0,
                    top_predictors: vec![],
                    color: "BLUE".to_owned(),
                },
            ],
            prediction_window_seconds: 30,
            status: "".to_owned(),
            title: "test".to_owned(),
            winning_outcome_id: None,
        },
        external,
    )
    .map_err(|base_err| {
        let mut err = ValidationError::new("js");
        err.add_param(
            "js-test-error".into(),
            &"Failed to test validity of js script",
        );
        err.with_message(base_err.to_string().into())
    })
}

fn validate_strategy(value: &Strategy, js_dir: &PathBuf) -> Result<(), ValidationErrors> {
    match value {
        Strategy::Detailed(t) => t.validate(),
        Strategy::External(t) => t.validate_with_args(&ExternalContext {
            _type: &t._type,
            js_dir,
        }),
    }
}

fn validate_filters(value: &Vec<Filter>, js_dir: &PathBuf) -> Result<(), ValidationErrors> {
    for f in value {
        f.validate_with_args(js_dir)?;
    }
    Ok(())
}
