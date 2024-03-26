use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Strategy {
    #[serde(rename = "smart")]
    Smart(Smart),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
#[validate(nested)]
pub struct Smart {
    #[validate(range(min = 0.0, max = 100.0))]
    #[serde(default = "defaults::_smart_low_threshold_default")]
    pub low_threshold: f64,
    #[validate(range(min = 0.0, max = 100.0))]
    #[serde(default = "defaults::_smart_high_threshold_default")]
    pub high_threshold: f64,
    #[validate(range(min = 0.0, max = 100.0))]
    #[serde(default = "defaults::_smart_high_odds_attempt_rate_default")]
    pub high_odds_attempt_rate: f64,
    #[validate(nested)]
    pub points: Points,
    #[validate(nested)]
    pub high_odds_points: Points,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
#[validate(nested)]
pub struct Points {
    pub max_value: u32,
    #[validate(range(min = 0.0, max = 100.0))]
    pub percent: f64,
}

#[rustfmt::skip]
mod defaults {
    pub const fn _smart_low_threshold_default() -> f64 { 40.0 }
    pub const fn _smart_high_threshold_default() -> f64 { 60.0 }
    pub const fn _smart_high_odds_attempt_rate_default() -> f64 { 50.0 }
}

impl<'v_a> ::validator::ValidateNested<'v_a> for Strategy {
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
            Strategy::Smart(t) => {
                ::validator::ValidationErrors::merge(result, "Smart", t.validate())
            }
        }
    }
}

impl Smart {
    pub fn normalize(&mut self) {
        self.low_threshold /= 100.0;
        self.high_threshold /= 100.0;
        self.high_odds_attempt_rate /= 100.0;
        self.points.normalize();
        self.high_odds_points.normalize();
    }
}

impl Strategy {
    pub fn normalize(&mut self) {
        match self {
            Strategy::Smart(s) => s.normalize(),
        }
    }
}

impl Points {
    pub fn value(&self, current_points: u32) -> u32 {
        if self.max_value == 0 {
            (self.percent * current_points as f64) as u32
        } else {
            if self.max_value < current_points {
                self.max_value
            } else {
                (self.percent * current_points as f64) as u32
            }
        }
    }

    pub fn normalize(&mut self) {
        self.percent /= 100.0;
    }
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Smart(Default::default())
    }
}
