use std::{collections::HashMap, fs};

use config::{
    filters::Filter, strategy::{DefaultPrediction, HighOdds, Points, Smart, Strategy}, Config, ConfigType, StreamerConfig
};

mod config;
mod types;

fn main() {
    let config = Config {
        streamers: HashMap::from([
            (
                "streamer_a".to_owned(),
                ConfigType::Specific(StreamerConfig {
                    strategy: Strategy::Smart(Smart {
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
                }),
            ),
            (
                "streamer_b".to_owned(),
                ConfigType::Preset("small".to_owned())
            )
        ]),
        presets: Some(HashMap::from([
            (
                "small".to_owned(),
                StreamerConfig {
                    strategy: Strategy::Smart(Smart::default()),
                    filters: vec![],
                }
            )
        ])),
    };

    fs::write(
        "example.config.yaml",
        &serde_yaml::to_string(&config).unwrap(),
    )
    .unwrap();
}
