use std::{collections::HashMap, fs};

use config::{
    filters::Filter,
    strategy::{DefaultPrediction, HighOdds, Points, Smart},
    Config, StreamerConfig,
};

mod config;
mod types;

fn main() {
    let config = Config {
        streamers: HashMap::from([(
            "example_streamer".to_owned(),
            StreamerConfig {
                strategy: config::strategy::Strategy::Smart(Smart {
                    high_odds: Some(vec![
                        HighOdds {
                            low_threshold: 90.0,
                            high_threshold: 10.0,
                            high_odds_attempt_rate: 1.0,
                            high_odds_points: Points {
                                max_value: 1000,
                                percent: 1.0,
                            },
                        },
                        HighOdds {
                            low_threshold: 70.0,
                            high_threshold: 30.0,
                            high_odds_attempt_rate: 3.0,
                            high_odds_points: Points {
                                max_value: 5000,
                                percent: 5.0,
                            },
                        },
                    ]),
                    default: DefaultPrediction {
                        max_percentage: 55.0,
                        min_percentage: 45.0,
                        points: Points {
                            max_value: 100000,
                            percent: 25.0,
                        },
                    },
                }),
                filters: vec![Filter::DelayPercentage(50.0)],
            },
        )]),
    };

    fs::write(
        "example.config.yaml",
        &serde_yaml::to_string(&config).unwrap(),
    )
    .unwrap();
}
