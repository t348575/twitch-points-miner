use serde::{Deserialize, Serialize};
use validator::Validate;

use super::Normalize;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub enum Strategy {
    Detailed(Detailed),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
#[validate(nested)]
pub struct Detailed {
    #[validate(nested)]
    pub detailed: Option<Vec<DetailedOdds>>,
    #[validate(nested)]
    pub default: DefaultPrediction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
#[validate(nested)]
pub struct DefaultPrediction {
    #[validate(range(min = 0.0, max = 100.0))]
    #[serde(default = "defaults::_detailed_high_threshold_default")]
    pub max_percentage: f64,
    #[validate(range(min = 0.0, max = 100.0))]
    #[serde(default = "defaults::_detailed_low_threshold_default")]
    pub min_percentage: f64,
    #[validate(nested)]
    pub points: Points,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub enum OddsComparisonType {
    #[default]
    Le,
    Ge,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
#[validate(nested)]
pub struct DetailedOdds {
    pub _type: OddsComparisonType,
    #[validate(range(min = 0.0, max = 100.0))]
    pub threshold: f64,
    #[validate(range(min = 0.0, max = 100.0))]
    pub attempt_rate: f64,
    #[validate(nested)]
    pub points: Points,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
#[validate(nested)]
pub struct Points {
    pub max_value: u32,
    #[validate(range(min = 0.0, max = 100.0))]
    pub percent: f64,
}

#[rustfmt::skip]
mod defaults {
    pub const fn _detailed_low_threshold_default() -> f64 { 40.0 }
    pub const fn _detailed_high_threshold_default() -> f64 { 60.0 }
}

impl<'v_a, 'a> ::validator::ValidateNested<'v_a> for Strategy {
    type Args = ();
    fn validate_nested(
        &self,
        field_name: &'static str,
        _: Self::Args,
    ) -> ::std::result::Result<(), ::validator::ValidationErrors> {
        let res = self.validate();
        if let Err(e) = res {
            let new_err = validator::ValidationErrorsKind::Struct(::std::boxed::Box::new(e));
            std::result::Result::Err(validator::ValidationErrors(
                ::std::collections::HashMap::from([(field_name, new_err)]),
            ))
        } else {
            std::result::Result::Ok(())
        }
    }
}

impl Validate for Strategy {
    #[allow(unused_mut)]
    fn validate(&self) -> ::std::result::Result<(), ::validator::ValidationErrors> {
        let mut errors = ::validator::ValidationErrors::new();
        let mut result = if errors.is_empty() {
            ::std::result::Result::Ok(())
        } else {
            ::std::result::Result::Err(errors)
        };
        match self {
            Strategy::Detailed(t) => {
                ::validator::ValidationErrors::merge(result, "detailed", t.validate())
            }
        }
    }
}

impl Normalize for Detailed {
    fn normalize(&mut self) {
        self.default.normalize();

        if let Some(h) = self.detailed.as_mut() {
            h.iter_mut().for_each(|x| {
                x.threshold /= 100.0;
                x.attempt_rate /= 100.0;
                x.points.normalize();
            });
        }
    }
}

impl Normalize for DefaultPrediction {
    fn normalize(&mut self) {
        self.max_percentage /= 100.0;
        self.min_percentage /= 100.0;
        self.points.normalize();
    }
}

impl Points {
    pub fn value(&self, current_points: u32) -> u32 {
        if self.max_value == 0 {
            (self.percent * current_points as f64) as u32
        } else {
            let percent_value = (self.percent * current_points as f64) as u32;
            if percent_value < self.max_value {
                percent_value
            } else {
                self.max_value
            }
        }
    }
}

impl Normalize for Points {
    fn normalize(&mut self) {
        self.percent /= 100.0;
    }
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Detailed(Default::default())
    }
}

impl Normalize for Strategy {
    fn normalize(&mut self) {
        match self {
            Strategy::Detailed(s) => s.normalize(),
        }
    }
}
