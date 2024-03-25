use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use color_eyre::{eyre::Context, Result};
use flume::{Receiver, Sender};
use futures::StreamExt;
use rand::{
    distributions::{Alphanumeric, DistString},
    Rng,
};
use serde::Serialize;
use tokio::{spawn, sync::RwLock};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};
use twitch_api::{
    pubsub::{
        listen_command,
        predictions::{Event as PredictionEvent, PredictionsChannelV1},
        unlisten_command, Response, TopicData, Topics,
    },
    types::UserId,
};

use crate::{
    auth::Token,
    common::{connect_twitch_ws, get_channel_points, make_prediction, ping_loop, writer},
    config::{self, Config},
    live::Events,
};

#[derive(Debug, Clone, Serialize)]
pub struct PubSub {
    pub streamers: HashMap<UserId, Streamer>,
    pub simulate: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Streamer {
    pub name: String,
    pub live: bool,
    pub predictions: HashMap<String, (PredictionEvent, bool)>,
    pub config: config::Streamer,
    pub points: u32,
    #[serde(skip)]
    pub last_points_refresh: Instant,
}

pub async fn run(
    token: Token,
    config: Config,
    events_rx: Receiver<Events>,
    pubsub: Arc<RwLock<PubSub>>,
) -> Result<()> {
    let (write, mut read) =
        connect_twitch_ws("wss://pubsub-edge.twitch.tv", &token.access_token).await?;
    info!("Connected to pubsub");

    let (tx, rx) = flume::unbounded();
    spawn(writer(rx, write));
    spawn(ping_loop(tx.clone()));
    spawn(event_listener(
        pubsub.clone(),
        events_rx,
        tx.clone(),
        token.access_token.clone(),
    ));

    {
        let mut writer = pubsub.write().await;
        for (_, s) in writer.streamers.iter_mut() {
            let points = get_channel_points(s.name.clone(), &token)
                .await
                .context("Get channel points")?;
            s.points = points;
            s.last_points_refresh = Instant::now();
        }
    }

    while let Some(Ok(msg)) = read.next().await {
        if let Message::Text(m) = msg {
            match Response::parse(&m) {
                Ok(r) => {
                    if let Response::Message { data } = r {
                        handle_response(data, &pubsub, &token)
                            .await
                            .context("Handle pubsub response")?;
                    }
                }
                Err(err) => warn!("Failed to parse message {:#?}", err),
            }
        }
    }
    Ok(())
}

async fn event_listener(
    pubsub: Arc<RwLock<PubSub>>,
    events_rx: Receiver<Events>,
    tx: Sender<String>,
    access_token: String,
) -> Result<()> {
    while let Ok(events) = events_rx.recv_async().await {
        let mut writer = pubsub.write().await;
        match events {
            Events::Live(id, status) => {
                if let Some(s) = writer.streamers.get_mut(&id) {
                    s.live = status;
                    info!("Live status of {} is {}", s.name, status);

                    let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
                    let topics = &[Topics::PredictionsChannelV1(PredictionsChannelV1 {
                        channel_id: id.as_str().parse()?,
                    })];
                    let cmd = if s.live {
                        listen_command(topics, access_token.as_str(), nonce.as_str())
                            .context("Generate listen command")?
                    } else {
                        unlisten_command(topics, nonce.as_str())
                            .context("Generate unlisten command")?
                    };
                    tx.send_async(cmd).await?;
                }
            }
        }
    }
    Ok(())
}

impl PubSub {
    pub fn new(channels: Vec<(UserId, String, config::Streamer)>, simulate: bool) -> PubSub {
        let streamers = channels
            .into_iter()
            .map(|(id, name, config)| {
                (
                    id,
                    Streamer {
                        name,
                        live: false,
                        predictions: HashMap::new(),
                        config,
                        points: 0,
                        last_points_refresh: Instant::now(),
                    },
                )
            })
            .collect();
        PubSub {
            streamers,
            simulate,
        }
    }

    #[cfg(feature = "api")]
    pub fn get_by_name(&self, name: &str) -> Option<&Streamer> {
        self.streamers.values().find(|s| s.name == name)
    }

    #[cfg(feature = "api")]
    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut Streamer> {
        self.streamers.values_mut().find(|s| s.name == name)
    }
}

async fn handle_response(
    data: TopicData,
    pubsub: &Arc<RwLock<PubSub>>,
    token: &Token,
) -> Result<()> {
    match data {
        TopicData::PredictionsChannelV1 { topic, reply } => {
            debug!("Got reply {:#?}", topic);
            let event = reply.data.event;
            let streamer = UserId::from_str(&event.channel_id)?;

            let mut writer = pubsub.write().await;
            if event.ended_at.is_none()
                && writer.streamers.contains_key(&streamer)
                && !writer.streamers[&streamer]
                    .predictions
                    .contains_key(event.id.as_str())
            {
                let s = writer.streamers.get_mut(&streamer).unwrap();
                info!("Prediction {} started", event.id);
                s.predictions.insert(event.id.clone(), (event, false));
            } else if event.ended_at.is_some() {
                debug!("Prediction {} ended", event.id);
                info!("Prediction {} ended", event.id);
                writer
                    .streamers
                    .get_mut(&streamer)
                    .unwrap()
                    .predictions
                    .remove(event.id.as_str());
            } else if writer.streamers.contains_key(&streamer)
                && writer.streamers[&streamer]
                    .predictions
                    .contains_key(event.id.as_str())
            {
                info!("Prediction {} updated", event.id);
                let s = writer.streamers.get(&streamer).unwrap().clone();

                if s.predictions[event.id.as_str()].1 {
                    return Ok(());
                }
                if s.last_points_refresh.elapsed() > Duration::from_secs(5) {
                    let points = get_channel_points(s.name.clone(), token)
                        .await
                        .context("Get channel points")?;
                    let s = writer.streamers.get_mut(&streamer).unwrap();
                    s.points = points;
                    s.last_points_refresh = Instant::now();
                }
                if let Some((outcome_id, points)) = prediction_logic(&s, &event.id)
                    .await
                    .context("Prediction logic")?
                {
                    info!("Prediction {} with {} points", event.id, points);
                    make_prediction(points, event.id.clone(), outcome_id, token, writer.simulate)
                        .await
                        .context("Make prediction")?;
                    let mut writer = pubsub.write().await;
                    let s = writer.streamers.get_mut(&streamer).unwrap();
                    s.predictions.get_mut(event.id.as_str()).unwrap().1 = true;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn prediction_logic(
    streamer: &Streamer,
    event_id: &String,
) -> Result<Option<(String, u32)>> {
    let prediction = streamer.predictions.get(event_id);
    if prediction.is_none() {
        return Ok(None);
    }

    let prediction = prediction.unwrap();
    for filter in &streamer.config.filters {
        if filter_matches(&prediction.0, &filter, &streamer).context("Checking filter")? {
            info!("Filter matches {:#?}", filter);
            return Ok(None);
        }
    }

    match &streamer.config.strategy {
        config::strategy::Strategy::Smart(s) => {
            if prediction.0.outcomes.len() < 2 {
                return Ok(None);
            }

            let total_points = prediction
                .0
                .outcomes
                .iter()
                .fold(0, |a, b| a + b.total_points);

            let mut odds_percentage = Vec::new();
            odds_percentage.reserve_exact(prediction.0.outcomes.len());
            for o in &prediction.0.outcomes {
                let odds = if o.total_points == 0 {
                    0.0
                } else {
                    total_points as f64 / o.total_points as f64
                };
                odds_percentage.push(if odds == 0.0 { 0.0 } else { 100.0 / odds });
            }

            let mut rng = rand::thread_rng();
            for (idx, p) in odds_percentage.into_iter().enumerate() {
                let value = if p > s.low_threshold && p < s.high_threshold {
                    s.points.value(streamer.points)
                } else if rng.gen_bool(s.high_odds_attempt_rate) {
                    s.high_odds_points.value(streamer.points)
                } else {
                    0
                };
                if value > 0 {
                    return Ok(Some((prediction.0.outcomes[idx].id.clone(), value)));
                }
            }
        }
    }
    Ok(None)
}

fn filter_matches(
    prediction: &PredictionEvent,
    filter: &config::Filter,
    _: &Streamer,
) -> Result<bool> {
    let res = match filter {
        config::Filter::TotalUsers(t) => {
            prediction.outcomes.iter().fold(0, |a, b| a + b.total_users) as u32 >= *t
        }
        config::Filter::DelaySeconds(d) => {
            let created_at: DateTime<Local> =
                DateTime::parse_from_rfc3339(prediction.created_at.as_str())?
                    .try_into()
                    .unwrap();
            (chrono::Local::now() - created_at).num_seconds() as u32 >= *d
        }
        config::Filter::DelayPercentage(d) => {
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
