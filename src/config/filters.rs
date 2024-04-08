use chrono::{DateTime, Local};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use twitch_api::pubsub::predictions::Event;

use crate::types::Streamer;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web_api", derive(utoipa::ToSchema))]
pub enum Filter {
    TotalUsers(u32),
    DelaySeconds(u32),
    DelayPercentage(f64),
}

pub fn filter_matches(prediction: &Event, filter: &Filter, _: &Streamer) -> Result<bool> {
    let res = match filter {
        Filter::TotalUsers(t) => {
            prediction.outcomes.iter().fold(0, |a, b| a + b.total_users) as u32 >= *t
        }
        Filter::DelaySeconds(d) => {
            let created_at: DateTime<Local> =
                DateTime::parse_from_rfc3339(prediction.created_at.as_str())?
                    .try_into()
                    .unwrap();
            (chrono::Local::now() - created_at).num_seconds() as u32 >= *d
        }
        Filter::DelayPercentage(d) => {
            let created_at: DateTime<Local> =
                DateTime::parse_from_rfc3339(prediction.created_at.as_str())?
                    .try_into()
                    .unwrap();
            let d = prediction.prediction_window_seconds as f64 * (d / 100.0);
            (chrono::Local::now() - created_at).num_seconds() as f64 >= d
        }
    };
    Ok(res)
}
