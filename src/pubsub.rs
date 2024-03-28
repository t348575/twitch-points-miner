use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    thread::sleep,
    time::{Duration, Instant},
};

use color_eyre::{eyre::Context, Result};
use futures::StreamExt;
use rand::{
    distributions::{Alphanumeric, DistString},
    Rng,
};
use serde::Serialize;
use tokio::{
    spawn,
    sync::{
        mpsc::{self, Receiver, Sender},
        RwLock,
    },
};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};
use twitch_api::{
    pubsub::{
        community_points::CommunityPointsUserV1, listen_command, predictions::PredictionsChannelV1,
        unlisten_command, Response, TopicData, Topics,
    },
    types::UserId,
};

use crate::{
    auth::Token,
    common::{connect_twitch_ws, get_channel_points, make_prediction, ping_loop, writer},
    config::{self, filters::filter_matches, Config},
    live::Events,
    types::{StarterInformation, Streamer},
};

#[derive(Debug, Clone, Serialize)]
pub struct PubSub {
    pub streamers: HashMap<UserId, Streamer>,
    pub simulate: bool,
    #[serde(skip)]
    token: Token,
    #[serde(skip)]
    spade_url: Option<String>,
    pub user_id: String,
}

impl PubSub {
    pub fn new(
        channels: Vec<StarterInformation>,
        simulate: bool,
        token: Token,
        user_id: String,
    ) -> PubSub {
        let streamers = channels
            .into_iter()
            .map(|x| {
                (
                    x.user_id,
                    Streamer {
                        name: x.name,
                        live: false,
                        predictions: HashMap::new(),
                        config: x.config,
                        points: 0,
                        last_points_refresh: Instant::now(),
                        broadcast_id: None,
                    },
                )
            })
            .collect();
        PubSub {
            streamers,
            simulate,
            token,
            spade_url: None,
            user_id,
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

    pub async fn run(
        token: Token,
        config: Config,
        events_rx: Receiver<Events>,
        pubsub: Arc<RwLock<PubSub>>,
    ) -> Result<()> {
        let (write, mut read) =
            connect_twitch_ws("wss://pubsub-edge.twitch.tv", &token.access_token).await?;
        info!("Connected to pubsub");

        let (tx, rx) = mpsc::channel(128);
        spawn(writer(rx, write));
        spawn(ping_loop(tx.clone()));
        spawn(event_listener(
            pubsub.clone(),
            events_rx,
            tx.clone(),
            token.access_token.clone(),
        ));
        spawn(view_points(pubsub.clone()));

        {
            let mut writer = pubsub.write().await;
            for (_, s) in writer.streamers.iter_mut() {
                let points = get_channel_points(&s.name, &token)
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
                            let mut writer = pubsub.write().await;
                            writer
                                .handle_response(data)
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

    async fn handle_response(&mut self, data: TopicData) -> Result<()> {
        match data {
            TopicData::PredictionsChannelV1 { topic, reply } => {
                debug!("Got reply {:#?}", topic);
                let event = reply.data.event;
                let streamer = UserId::from_str(&event.channel_id)?;

                if event.ended_at.is_none()
                    && self.streamers.contains_key(&streamer)
                    && !self.streamers[&streamer]
                        .predictions
                        .contains_key(event.id.as_str())
                {
                    let s = self.streamers.get_mut(&streamer).unwrap();
                    info!("Prediction {} started", event.id);
                    let event_id = event.id.clone();
                    s.predictions.insert(event.id.clone(), (event, false));
                    self.try_prediction(&streamer, &event_id).await?;
                } else if event.ended_at.is_some() {
                    info!("Prediction {} ended", event.id);
                    self.streamers
                        .get_mut(&streamer)
                        .unwrap()
                        .predictions
                        .remove(event.id.as_str());
                } else if self.streamers.contains_key(&streamer)
                    && self.streamers[&streamer]
                        .predictions
                        .contains_key(event.id.as_str())
                {
                    info!("Prediction {} updated", event.id);
                    self.try_prediction(&streamer, &event.id).await?;
                }
            }
            TopicData::CommunityPointsUserV1 { topic, reply } => {
                debug!("Got reply {:#?}", topic);
                let streamer = UserId::from_str(&reply.data.channel_id)?;

                if self.streamers.contains_key(&streamer) {
                    debug!("Channel points updated for {}", streamer);
                    let s = self.streamers.get_mut(&streamer).unwrap();
                    s.points = reply.data.balance.balance as u32;
                    s.last_points_refresh = Instant::now();
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn try_prediction(&mut self, streamer: &UserId, event_id: &str) -> Result<()> {
        let s = self.streamers.get(streamer).unwrap().clone();

        if s.predictions[event_id].1 {
            return Ok(());
        }
        if s.last_points_refresh.elapsed() > Duration::from_secs(5) {
            let points = get_channel_points(&s.name, &self.token)
                .await
                .context("Get channel points")?;
            let s = self.streamers.get_mut(streamer).unwrap();
            s.points = points;
            s.last_points_refresh = Instant::now();
        }

        if let Some((outcome_id, points)) = prediction_logic(&s, event_id)
            .await
            .context("Prediction logic")?
        {
            info!("Attempting prediction {}, with points {}", event_id, points);
            make_prediction(points, event_id, outcome_id, &self.token, self.simulate)
                .await
                .context("Make prediction")?;
            let s = self.streamers.get_mut(streamer).unwrap();
            s.predictions.get_mut(event_id).unwrap().1 = true;
        }
        Ok(())
    }
}

async fn event_listener(
    pubsub: Arc<RwLock<PubSub>>,
    mut events_rx: Receiver<Events>,
    tx: Sender<String>,
    access_token: String,
) -> Result<()> {
    while let Some(events) = events_rx.recv().await {
        let mut writer = pubsub.write().await;
        match events {
            Events::Live {
                channel_id,
                broadcast_id,
            } => {
                if let Some(s) = writer.streamers.get_mut(&channel_id) {
                    info!("Live status of {} is {}", s.name, broadcast_id.is_some());
                    s.live = broadcast_id.is_some();
                    s.broadcast_id = broadcast_id;

                    let channel_id = channel_id.as_str().parse()?;
                    let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
                    let topics = &[
                        Topics::PredictionsChannelV1(PredictionsChannelV1 { channel_id }),
                        Topics::CommunityPointsUserV1(CommunityPointsUserV1 { channel_id }),
                    ];
                    let cmd = if s.live {
                        listen_command(topics, access_token.as_str(), nonce.as_str())
                            .context("Generate listen command")?
                    } else {
                        unlisten_command(topics, nonce.as_str())
                            .context("Generate unlisten command")?
                    };
                    info!("{}", cmd);
                    tx.send(cmd).await?;
                }
            }
            Events::SpadeUpdate(s) => writer.spade_url = Some(s),
        }
    }
    Ok(())
}

pub async fn prediction_logic(
    streamer: &Streamer,
    event_id: &str,
) -> Result<Option<(String, u32)>> {
    let prediction = streamer.predictions.get(event_id);
    if prediction.is_none() {
        return Ok(None);
    }

    let prediction = prediction.unwrap();
    for filter in &streamer.config.filters {
        if !filter_matches(&prediction.0, &filter, &streamer).context("Checking filter")? {
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
                debug!("Odds for {}: {}", prediction.0.outcomes[idx].id, p);

                let empty_vec = Vec::new();
                let points = s
                    .high_odds
                    .as_ref()
                    .unwrap_or(&empty_vec)
                    .into_iter()
                    .filter(|x| -> bool {
                        if (p <= x.low_threshold || p >= x.high_threshold)
                            && rng.gen_bool(x.high_odds_attempt_rate)
                        {
                            return true;
                        }
                        false
                    })
                    .map(|x| &x.high_odds_points)
                    .next();

                match points {
                    Some(s) => {
                        return Ok(Some((
                            prediction.0.outcomes[idx].id.clone(),
                            s.value(streamer.points),
                        )))
                    }
                    None => {
                        if p >= s.default.min_percentage && p <= s.default.max_percentage {
                            return Ok(Some((
                                prediction.0.outcomes[idx].id.clone(),
                                s.default.points.value(streamer.points),
                            )));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

async fn view_points(pubsub: Arc<RwLock<PubSub>>) {
    loop {
        let streamer = {
            let pubsub = pubsub.read().await;
            pubsub
                .streamers
                .values()
                .filter(|x| x.live)
                .next()
                .map(|x| x.clone())
        };

        if streamer.is_none() {
            sleep(Duration::from_secs(60));
            continue;
        }

        let streamer = streamer.unwrap();

        sleep(Duration::from_secs(60))
    }
}
