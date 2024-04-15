use std::fs;

use config::{
    filters::Filter, strategy::*, Config, ConfigType, StreamerConfig
};
use indexmap::IndexMap;

mod config;
mod types;

fn main() {
    let config = Config {
        watch_priority: Some(vec!["streamer_b".to_owned()]),
        analytics_db: "/my/custom/path/to/analytics".to_owned(),
        streamers: IndexMap::from([
            (
                "streamer_a".to_owned(),
                ConfigType::Specific(StreamerConfig {
                    strategy: Strategy::Detailed(Detailed {
                        detailed: Some(vec![
                            DetailedOdds {
                                _type: OddsComparisonType::Ge,
                                threshold: 90.0,
                                attempt_rate: 100.0,
                                points: Points {
                                    max_value: 1000,
                                    percent: 1.0,
                                },
                            },
                            DetailedOdds {
                                _type: OddsComparisonType::Le,
                                threshold: 10.0,
                                attempt_rate: 1.0,
                                points: Points {
                                    max_value: 1000,
                                    percent: 1.0,
                                },
                            },
                            DetailedOdds {
                                _type: OddsComparisonType::Ge,
                                threshold: 70.0,
                                attempt_rate: 100.0,
                                points: Points {
                                    max_value: 5000,
                                    percent: 5.0,
                                },
                            },
                            DetailedOdds {
                                _type: OddsComparisonType::Le,
                                threshold: 30.0,
                                attempt_rate: 3.0,
                                points: Points {
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
                    filters: vec![Filter::DelayPercentage(50.0), Filter::TotalUsers(300)],
                }),
            ),
            (
                "streamer_b".to_owned(),
                ConfigType::Preset("small".to_owned())
            )
        ]),
        presets: Some(IndexMap::from([
            (
                "small".to_owned(),
                StreamerConfig {
                    strategy: Strategy::Detailed(Detailed::default()),
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
