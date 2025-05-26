use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use validator::{ValidateArgs, ValidateRange, ValidationError, ValidationErrors};

use crate::config::ExternalContext;

use super::External;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub enum Filter {
    TotalUsers(u32),
    DelaySeconds(u32),
    DelayPercentage(f64),
    External(External),
}

impl Filter {
    pub fn validate_with_args(&self, js_dir: &PathBuf) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();

        use ValidateRange;
        match self {
            Filter::TotalUsers(t) => {
                if !t.validate_range(Some(0), None, None, None) {
                    let mut err = ValidationError::new("range");
                    err.add_param("min".into(), &0);
                    err.add_param("value".into(), &t);
                    errors.add("TotalUsers", err);
                }
            }
            Filter::DelaySeconds(t) => {
                if !t.validate_range(Some(0), None, None, None) {
                    let mut err = ValidationError::new("range");
                    err.add_param("min".into(), &0);
                    err.add_param("value".into(), &t);
                    errors.add("DelaySeconds", err);
                }
            }
            Filter::DelayPercentage(t) => {
                if !t.validate_range(Some(0.0), None, None, None) {
                    let mut err = ValidationError::new("range");
                    err.add_param("min".into(), &0);
                    err.add_param("value".into(), &t);
                    errors.add("DelayPercentage", err);
                }
            }
            Filter::External(js) => ValidationErrors::merge(
                Ok(()),
                "External",
                js.validate_with_args(&ExternalContext {
                    _type: &js._type,
                    js_dir,
                }),
            )?,
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
