use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use color_eyre::{
    eyre::{eyre, Context, ContextCompat},
    Result,
};
use common::{
    config::{self, filters::filter_matches, strategy, Config, ConfigType, StreamerConfig},
    tokio_tungstenite::tungstenite::Message,
    twitch::{api, auth::Token, gql, ws},
    types::{
        ConfigTypeRef, StreamerConfigRef, StreamerConfigRefWrapper, StreamerInfo, StreamerState,
    },
};
use flume::{unbounded, Receiver, Sender};
use futures::StreamExt;
use indexmap::IndexMap;
use rand::{
    distributions::{Alphanumeric, DistString},
    Rng,
};
use serde::Serialize;
use tokio::task::JoinHandle;
use tokio::{spawn, sync::RwLock, time::sleep};
use tracing::{debug, error, info, warn};
use twitch_api::{
    pubsub::{
        community_points::CommunityPointsUserV1,
        listen_command,
        predictions::{Event, PredictionsChannelV1},
        raid::{Raid, RaidReply},
        unlisten_command, Response, TopicData, Topics,
    },
    types::UserId,
};

use crate::analytics::model::{PointsInfo, Prediction};
use crate::live::Events;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PubSub {
    #[serde(skip)]
    pub config: Config,
    #[serde(skip)]
    pub config_path: String,
    pub streamers: HashMap<UserId, StreamerState>,
    pub simulate: bool,
    #[serde(skip)]
    token: Token,
    #[serde(skip)]
    spade_url: Option<String>,
    pub user_id: String,
    pub user_name: String,
    pub configs: HashMap<String, StreamerConfigRefWrapper>,
    #[serde(skip)]
    pub gql: gql::Client,
    #[serde(skip)]
    pub base_url: String,
    #[serde(skip)]
    pub live_runner: Option<JoinHandle<Result<()>>>,
    #[serde(skip)]
    pub events_tx: Sender<Events>,
    #[serde(skip)]
    pub analytics: Arc<crate::analytics::AnalyticsWrapper>,
}

impl Clone for PubSub {
    fn clone(&self) -> Self {
        Self {
            streamers: self.streamers.clone(),
            simulate: self.simulate,
            token: self.token.clone(),
            spade_url: self.spade_url.clone(),
            user_id: self.user_id.clone(),
            user_name: self.user_name.clone(),
            configs: self.configs.clone(),
            live_runner: None,
            events_tx: self.events_tx.clone(),
            analytics: self.analytics.clone(),
            config: self.config.clone(),
            config_path: self.config_path.clone(),
            gql: self.gql.clone(),
            base_url: self.base_url.clone(),
        }
    }
}

impl PubSub {
    pub fn new(
        config: Config,
        config_path: String,
        channels: Vec<((UserId, StreamerInfo), &ConfigType)>,
        points: Vec<(u32, Option<String>)>,
        active_predictions: Vec<Vec<(Event, bool)>>,
        presets: IndexMap<String, StreamerConfig>,
        simulate: bool,
        token: Token,
        user_info: (String, String),
        gql: gql::Client,
        base_url: &str,
        live_runner: JoinHandle<Result<()>>,
        events_tx: Sender<Events>,
        analytics: Arc<crate::analytics::AnalyticsWrapper>,
    ) -> Result<PubSub> {
        let mut configs = channels
            .iter()
            .filter_map(|x| match x.1 {
                ConfigType::Preset(_) => None,
                ConfigType::Specific(c) => Some((
                    x.0 .1.channel_name.clone(),
                    StreamerConfigRefWrapper::new(StreamerConfigRef {
                        _type: ConfigTypeRef::Specific,
                        config: c.clone(),
                    }),
                )),
            })
            .collect::<HashMap<_, _>>();

        let presets_iter = presets.into_iter().map(|(name, c)| {
            (
                name.clone(),
                StreamerConfigRefWrapper::new(StreamerConfigRef {
                    _type: ConfigTypeRef::Preset(name),
                    config: c,
                }),
            )
        });
        configs.extend(presets_iter);

        let streamers = channels
            .into_iter()
            .zip(points)
            .zip(active_predictions)
            .map(|((((channel_id, info), config), (p, _)), ap)| {
                (
                    channel_id,
                    StreamerState {
                        config: configs[match config {
                            ConfigType::Preset(p) => p,
                            ConfigType::Specific(_) => &info.channel_name,
                        }]
                        .clone(),
                        info,
                        predictions: ap
                            .into_iter()
                            .map(|x| (x.0.id.to_string(), x))
                            .collect::<HashMap<_, _>>(),
                        points: p,
                        last_points_refresh: Instant::now(),
                    },
                )
            })
            .collect();
        Ok(PubSub {
            config,
            config_path,
            streamers,
            simulate,
            token,
            spade_url: None,
            user_id: user_info.0,
            user_name: user_info.1,
            configs,
            live_runner: Some(live_runner),
            events_tx,
            analytics,
            gql,
            base_url: base_url.to_string(),
        })
    }

    #[cfg(test)]
    pub fn empty(events_tx: Sender<Events>) -> Self {
        Self {
            events_tx,
            analytics: Arc::new(crate::analytics::AnalyticsWrapper::empty()),
            config: Default::default(),
            config_path: Default::default(),
            streamers: Default::default(),
            simulate: Default::default(),
            token: Default::default(),
            spade_url: Default::default(),
            user_id: Default::default(),
            user_name: Default::default(),
            configs: Default::default(),
            gql: Default::default(),
            base_url: Default::default(),
            live_runner: Default::default(),
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<&StreamerState> {
        self.streamers
            .values()
            .find(|s| s.info.channel_name == name)
    }

    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut StreamerState> {
        self.streamers
            .values_mut()
            .find(|s| s.info.channel_name == name)
    }

    pub fn get_id_by_name(&self, name: &str) -> Option<&str> {
        for (k, v) in self.streamers.iter() {
            if v.info.channel_name == name {
                return Some(k.as_str());
            }
        }
        None
    }

    pub async fn run(
        token: Token,
        events_rx: Receiver<Events>,
        pubsub: Arc<RwLock<PubSub>>,
        gql: gql::Client,
    ) -> Result<()> {
        let (write, mut read) = ws::connect_twitch_ws(&token.access_token)
            .await
            .context("Could not connect to pub sub")?;
        let (tx, rx) = unbounded::<String>();

        spawn(ws::writer(rx, write));
        spawn(ws::ping_loop(tx.clone()));
        spawn(event_listener(
            pubsub.clone(),
            events_rx.clone(),
            tx.clone(),
            token.access_token.clone(),
        ));
        spawn(watch_streams(pubsub.clone(), events_rx));
        spawn(update_and_claim_points(pubsub.clone(), gql.clone()));

        info!("Connected to pubsub");
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
                debug!("Got PredictionsChannelV1 {:#?}", topic);
                let event = reply.data.event;
                let streamer = UserId::from_str(&event.channel_id)?;

                let name = self
                    .streamers
                    .get(&streamer)
                    .context(format!("Streamer not found: {streamer}"))?
                    .info
                    .channel_name
                    .clone();
                self.handle_prediction_event(event, streamer)
                    .await
                    .context(format!("Handle prediction event: {name}"))?;
            }
            TopicData::CommunityPointsUserV1 { topic, reply } => {
                debug!("Got CommunityPointsUserV1 {:#?}", topic);
                let streamer = UserId::from_str(&reply.data.channel_id)?;

                if self.streamers.contains_key(&streamer) {
                    debug!("Channel points updated for {}", streamer);
                    let s = self.streamers.get_mut(&streamer).unwrap();
                    s.points = reply.data.balance.balance as u32;
                    s.last_points_refresh = Instant::now();
                }
            }
            TopicData::Raid { topic, reply } => {
                debug!("Got Raid {:#?}", topic);

                if let RaidReply::RaidUpdateV2(raid) = *reply {
                    if let Some(s) = self.streamers.get(&raid.source_id) {
                        if s.config.0.read().unwrap().config.follow_raid {
                            info!(
                                "Joining raid for {} to {}",
                                s.info.channel_name, raid.target_login
                            );
                            self.gql.join_raid(&raid.id).await.context("Raiding user")?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn upsert_prediction(&mut self, streamer: &UserId, event: &Event) -> Result<()> {
        let channel_id = streamer.as_str().parse()?;
        let created_at = chrono::DateTime::<chrono::offset::FixedOffset>::parse_from_rfc3339(
            event.created_at.as_str(),
        )?
        .naive_local();
        let closed_at = if let Some(x) = &event.ended_at {
            Some(
                chrono::DateTime::<chrono::offset::FixedOffset>::parse_from_rfc3339(x.as_str())?
                    .naive_local(),
            )
        } else {
            None
        };

        self.analytics
            .execute(|analytics| {
                analytics.upsert_prediction(&Prediction {
                    channel_id,
                    prediction_id: event.id.clone(),
                    title: event.title.clone(),
                    prediction_window: event.prediction_window_seconds,
                    outcomes: event.outcomes.clone().into(),
                    winning_outcome_id: None,
                    placed_bet: crate::analytics::model::PredictionBetWrapper::None,
                    created_at,
                    closed_at,
                })
            })
            .await?;
        Ok(())
    }

    async fn handle_prediction_event(&mut self, event: Event, streamer: UserId) -> Result<()> {
        if event.locked_at.is_some() && event.ended_at.is_none() {
            debug!("Event {} locked, but not yet ended", event.id);
            return Ok(());
        }

        if event.ended_at.is_none()
            && self.streamers.contains_key(&streamer)
            && !self.streamers[&streamer]
                .predictions
                .contains_key(event.id.as_str())
        {
            let s = self.streamers.get_mut(&streamer).unwrap();
            info!("Prediction {} started", event.id);
            let event_id = event.id.clone();
            s.predictions
                .insert(event.id.clone(), (event.clone(), false));

            self.upsert_prediction(&streamer, &event).await?;

            self.try_prediction(&streamer, &event_id).await?;
        } else if event.ended_at.is_some() {
            info!("Prediction {} ended", event.id);
            if !self
                .streamers
                .get_mut(&streamer)
                .unwrap()
                .predictions
                .contains_key(event.id.as_str())
            {
                return Ok(());
            }

            self.upsert_prediction(&streamer, &event).await?;

            let channel_id = event.channel_id.parse()?;
            let points_value = self
                .gql
                .get_channel_points(&[&self.streamers.get(&streamer).unwrap().info.channel_name])
                .await?[0]
                .0;
            let closed_at = chrono::DateTime::<chrono::offset::FixedOffset>::parse_from_rfc3339(
                event.ended_at.unwrap().as_str(),
            )?
            .naive_local();

            self.analytics
                .execute(|analytics| {
                    let entry_id = analytics.last_prediction_id(channel_id, &event.id)?;
                    analytics.insert_points(
                        channel_id,
                        points_value as i32,
                        PointsInfo::Prediction(event.id.clone(), entry_id),
                    )?;
                    analytics.end_prediction(
                        &event.id,
                        channel_id,
                        event.winning_outcome_id,
                        event.outcomes.into(),
                        closed_at,
                    )
                })
                .await?;

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
            let event_id = event.id.clone();
            info!("Prediction {} updated", event.id);

            self.upsert_prediction(&streamer, &event).await?;
            if let Some((e, _)) = self
                .streamers
                .get_mut(&streamer)
                .unwrap()
                .predictions
                .get_mut(&event_id)
            {
                *e = event;
            }
            self.try_prediction(&streamer, &event_id).await?;
        }
        Ok(())
    }

    async fn try_prediction(&mut self, streamer: &UserId, event_id: &str) -> Result<()> {
        let s = self.streamers.get(streamer).unwrap().clone();

        if s.predictions[event_id].1 {
            return Ok(());
        }
        if s.last_points_refresh.elapsed() > Duration::from_secs(30) {
            let points = self
                .gql
                .get_channel_points(&[&s.info.channel_name])
                .await
                .context("Get channel points")?;
            let s = self.streamers.get_mut(streamer).unwrap();
            s.points = points[0].0;
            s.last_points_refresh = Instant::now();
        }

        if let Some((outcome_id, points_to_bet)) = prediction_logic(&s, event_id)
            .await
            .context("Prediction logic")?
        {
            info!(
                "{}: predicting {}, with points {}",
                s.info.channel_name, event_id, points_to_bet
            );
            self.gql
                .make_prediction(points_to_bet, event_id, &outcome_id, self.simulate)
                .await
                .context("Make prediction")?;
            let s = self.streamers.get_mut(streamer).unwrap();
            s.predictions.get_mut(event_id).unwrap().1 = true;

            let channel_id = streamer.as_str().parse::<i32>()?;
            let points = self
                .gql
                .get_channel_points(&[s.info.channel_name.as_str()])
                .await?;
            self.analytics
                .execute(|analytics| {
                    let entry_id = analytics.last_prediction_id(channel_id, event_id)?;
                    analytics.insert_points(
                        channel_id,
                        points[0].0 as i32,
                        PointsInfo::Prediction(event_id.to_owned(), entry_id),
                    )?;

                    analytics.place_bet(event_id, channel_id, &outcome_id, points_to_bet)
                })
                .await?;
        }
        Ok(())
    }

    pub fn restart_live_watcher(&mut self) {
        if let Some(s) = self.live_runner.take() {
            s.abort();
        }

        let channels = self
            .streamers
            .iter()
            .map(|x| (x.0.clone(), x.1.info.clone()))
            .collect();
        let live_runner = spawn(crate::live::run(
            self.events_tx.clone(),
            channels,
            self.gql.clone(),
            self.base_url.clone(),
        ));

        self.live_runner.replace(live_runner);
    }
}

async fn event_listener(
    pubsub: Arc<RwLock<PubSub>>,
    events_rx: Receiver<Events>,
    tx: Sender<String>,
    access_token: String,
) {
    async fn inner(
        pubsub: &Arc<RwLock<PubSub>>,
        events_rx: &Receiver<Events>,
        tx: &Sender<String>,
        access_token: &str,
    ) -> Result<()> {
        while let Ok(events) = events_rx.recv_async().await {
            let mut writer = pubsub.write().await;
            match events {
                Events::Live {
                    channel_id,
                    broadcast_id,
                } => {
                    if let Some(s) = writer.streamers.get_mut(&channel_id) {
                        info!(
                            "Live status of {} is {}",
                            s.info.channel_name,
                            broadcast_id.is_some()
                        );
                        s.info.live = broadcast_id.is_some();
                        s.info.broadcast_id = broadcast_id;

                        let channel_id = channel_id.as_str().parse()?;
                        let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 30);
                        let topics = [
                            Topics::PredictionsChannelV1(PredictionsChannelV1 { channel_id }),
                            Topics::CommunityPointsUserV1(CommunityPointsUserV1 { channel_id }),
                            Topics::Raid(Raid { channel_id }),
                        ];

                        let cmds = if s.info.live {
                            topics
                                .into_iter()
                                .map(|x| {
                                    listen_command(&[x], access_token, nonce.as_str())
                                        .context("Generate listen command")
                                })
                                .collect::<Result<Vec<_>, _>>()
                        } else {
                            topics
                                .into_iter()
                                .map(|x| {
                                    unlisten_command(&[x], nonce.as_str())
                                        .context("Generate unlisten command")
                                })
                                .collect::<Result<Vec<_>, _>>()
                        }?;

                        for item in cmds {
                            tx.send_async(item).await?;
                        }
                    }
                }
                Events::SpadeUpdate(s) => writer.spade_url = Some(s),
            }
        }
        Ok(())
    }

    loop {
        if let Err(err) = inner(&pubsub, &events_rx, &tx, &access_token).await {
            error!("{err:#?}");
        }
    }
}

pub async fn prediction_logic(
    streamer: &StreamerState,
    event_id: &str,
) -> Result<Option<(String, u32)>> {
    let prediction = streamer.predictions.get(event_id);
    if prediction.is_none() {
        return Ok(None);
    }

    let c = streamer
        .config
        .0
        .read()
        .map_err(|_| eyre!("Streamer config poison error"))?;

    let prediction = prediction.unwrap();
    for filter in &c.config.prediction.filters {
        if !filter_matches(&prediction.0, filter, streamer).context("Checking filter")? {
            info!("Filter matches {:#?}", filter);
            return Ok(None);
        }
    }

    match &c.config.prediction.strategy {
        config::strategy::Strategy::Detailed(s) => {
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
                odds_percentage.push(if odds == 0.0 { 0.0 } else { 1.0 / odds });
            }

            let mut rng = rand::thread_rng();
            for (idx, p) in odds_percentage.into_iter().enumerate() {
                debug!("Odds for {}: {}", prediction.0.outcomes[idx].id, p);

                let empty_vec = Vec::new();
                let points = s.detailed.as_ref().unwrap_or(&empty_vec).iter().find(|x| {
                    debug!("Checking config {x:#?}");
                    let does_match = match x._type {
                        strategy::OddsComparisonType::Le => p <= x.threshold,
                        strategy::OddsComparisonType::Ge => p >= x.threshold,
                    };
                    if does_match && rng.gen_bool(x.attempt_rate) {
                        return true;
                    }
                    false
                });

                match points {
                    Some(s) => {
                        debug!("Using high odds config {s:#?}");
                        return Ok(Some((
                            prediction.0.outcomes[idx].id.clone(),
                            s.points.value(streamer.points),
                        )));
                    }
                    None => {
                        if p >= s.default.min_percentage && p <= s.default.max_percentage {
                            debug!("Using default odds config {:#?} {}", s.default, p);
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

async fn watch_streams(pubsub: Arc<RwLock<PubSub>>, events_rx: Receiver<Events>) {
    sleep(Duration::from_secs(5)).await;

    let use_watch_streak = {
        let reader = pubsub.read().await;
        reader.config.watch_streak.unwrap_or(true)
    };

    let mut watch_streak = Vec::new();
    async fn inner(
        pubsub: &Arc<RwLock<PubSub>>,
        watch_streak: &mut Vec<(UserId, i32)>,
    ) -> Result<()> {
        let (streamers, user_id, user_name, spade_url, access_token, watch_priority) = {
            let reader = pubsub.read().await;
            let streamers = reader
                .streamers
                .iter()
                .filter(|x| x.1.info.live)
                .map(|x| (x.0.clone(), x.1.clone()))
                .collect::<Vec<_>>();

            (
                streamers,
                reader.user_id.parse()?,
                reader.user_name.clone(),
                reader.spade_url.clone().ok_or(eyre!("Spade URL not set"))?,
                reader.token.access_token.clone(),
                reader.config.watch_priority.clone().unwrap_or_default(),
            )
        };

        if streamers.is_empty() {
            debug!("No streamer found");
            sleep(Duration::from_secs(60)).await;
            return Ok(());
        }

        let mut watch_items = Vec::new();
        for item in &watch_priority {
            if let Some(s) = streamers.iter().find(|x| x.1.info.channel_name.eq(item)) {
                watch_items.push(s);
            }
        }

        // streamers not given in a priority order
        for item in &streamers {
            if !watch_priority.contains(&item.1.info.channel_name) {
                watch_items.push(item);
            }
        }

        // Just to allow the reference to live
        #[allow(unused_assignments)]
        let mut streak_entry = None;
        let mut entry = watch_streak.iter_mut().take(1);
        if let Some(entry) = entry.next() {
            entry.1 += 1;
            let s = pubsub.read().await.streamers.get(&entry.0).unwrap().clone();
            streak_entry = Some((entry.0.clone(), s));
            watch_items.insert(0, streak_entry.as_ref().unwrap());
        }

        for (id, streamer) in watch_items.into_iter().take(2) {
            debug!("Watching {}", streamer.info.channel_name);
            api::set_viewership(
                user_name.clone(),
                user_id,
                id.clone(),
                streamer.info.clone(),
                &spade_url,
                &access_token,
            )
            .await?;
        }

        *watch_streak = watch_streak.drain(..).filter(|x| x.1 < 60).collect();
        Ok(())
    }

    loop {
        if let Err(err) = inner(&pubsub, &mut watch_streak).await {
            if err.to_string() != "Spade URL not set" {
                error!("{err}");
            }
        }

        if use_watch_streak {
            let live = events_rx.drain().filter_map(|x| {
                if let Events::Live {
                    channel_id,
                    broadcast_id: _,
                } = x
                {
                    Some((channel_id, 0))
                } else {
                    None
                }
            });
            watch_streak.extend(live);
        }
        sleep(Duration::from_secs(10)).await;
    }
}

async fn update_and_claim_points(pubsub: Arc<RwLock<PubSub>>, gql: gql::Client) -> Result<()> {
    sleep(Duration::from_secs(5)).await;

    async fn inner(pubsub: &Arc<RwLock<PubSub>>, gql: &gql::Client) -> Result<()> {
        let streamer = {
            let reader = pubsub.read().await;
            reader
                .streamers
                .iter()
                .filter(|x| x.1.info.live)
                .map(|x| (x.0.clone(), x.1.clone()))
                .collect::<Vec<_>>()
        };

        let channel_names = streamer
            .iter()
            .map(|x| x.1.info.channel_name.as_str())
            .collect::<Vec<_>>();

        if channel_names.is_empty() {
            sleep(Duration::from_secs(60)).await;
            return Ok(());
        }

        let points = gql.get_channel_points(&channel_names).await?;
        {
            let mut points_value;
            let mut points_info = PointsInfo::Watching;

            let mut writer = pubsub.write().await;
            for (idx, (id, _)) in streamer.iter().enumerate() {
                points_value = points[idx].0;
                if let Some(s) = writer.streamers.get_mut(id) {
                    s.points = points[idx].0;
                    s.last_points_refresh = Instant::now();
                    if let Some(claim_id) = &points[idx].1 {
                        info!("Claiming community points bonus {}", s.info.channel_name);
                        let claimed_points = gql.claim_points(id.as_str(), claim_id).await?;
                        points_value = s.points;
                        points_info = PointsInfo::CommunityPointsClaimed;
                        s.points = claimed_points;
                        s.last_points_refresh = Instant::now();
                    }
                }

                let channel_id = id.as_str().parse::<i32>()?;
                writer
                    .analytics
                    .execute(|analytics| {
                        analytics.insert_points_if_updated(
                            channel_id,
                            points_value as i32,
                            points_info.clone(),
                        )
                    })
                    .await?;
            }
        }
        Ok(())
    }

    loop {
        if let Err(err) = inner(&pubsub, &gql).await {
            error!("{err}");
        }

        sleep(Duration::from_secs(60)).await
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        sync::Arc,
        time::{Duration, Instant},
    };

    use chrono::Local;
    use color_eyre::Result;
    use flume::unbounded;
    use futures::StreamExt;
    use rstest::rstest;
    use serde_json::json;
    use tokio::{spawn, sync::RwLock, time::timeout};
    use twitch_api::{
        pubsub::predictions::{Event, Outcome},
        types::{Timestamp, UserId},
    };

    use common::{
        config::{
            strategy::{DefaultPrediction, DetailedOdds, Strategy},
            PredictionConfig, StreamerConfig,
        },
        twitch::{
            gql::{Client, Stream, User},
            traverse_json,
        },
        types::*,
    };

    use crate::{
        live,
        pubsub::prediction_logic,
        test::{container, TestContainer},
    };

    use super::PubSub;

    fn outcome_from(id: u32, points: i64, users: i64) -> Outcome {
        Outcome {
            id: id.to_string(),
            color: "".to_owned(),
            title: id.to_string(),
            total_points: points,
            total_users: users,
            top_predictors: Vec::new(),
        }
    }

    fn get_prediction() -> StreamerState {
        StreamerState {
            info: StreamerInfo {
                broadcast_id: None,
                live: true,
                channel_name: "a".to_owned(),
                game: None,
            },
            predictions: HashMap::from([(
                "pred-key-1".to_owned(),
                (
                    Event {
                        id: "pred-key-1".to_owned(),
                        channel_id: "channel-id-1".to_owned(),
                        created_at: Timestamp::new(Local::now().to_rfc3339()).unwrap(),
                        ended_at: None,
                        locked_at: None,
                        outcomes: Vec::new(),
                        prediction_window_seconds: 1500,
                        status: "".to_owned(),
                        title: "".to_owned(),
                        winning_outcome_id: None,
                    },
                    false,
                ),
            )]),
            config: StreamerConfigRefWrapper::new(StreamerConfigRef {
                _type: ConfigTypeRef::Specific,
                config: StreamerConfig {
                    follow_raid: true,
                    prediction: PredictionConfig {
                        strategy: Strategy::default(),
                        filters: vec![],
                    },
                },
            }),
            points: 0,
            last_points_refresh: Instant::now(),
        }
    }

    #[tokio::test]
    async fn detailed_strategy_default() -> Result<()> {
        use common::config::strategy as s;
        let mut streamer = get_prediction();
        {
            let pred = streamer.predictions.get_mut("pred-key-1").unwrap();
            streamer.points = 50000;
            pred.0.outcomes = vec![
                outcome_from(1, 5_000, 2),
                outcome_from(2, 30_000, 14),
                outcome_from(3, 45_000, 10),
                outcome_from(4, 1_000, 1),
            ];
        }

        let default_points_percentage = 0.15;
        let default_max_points = 40000;

        let mut config_ref = streamer.config.0.write().unwrap();
        #[allow(irrefutable_let_patterns)]
        if let Strategy::Detailed(d) = &mut config_ref.config.prediction.strategy {
            d.default = DefaultPrediction {
                max_percentage: 0.55,
                min_percentage: 0.45,
                points: s::Points {
                    max_value: default_max_points,
                    percent: default_points_percentage,
                },
            };

            d.detailed = Some(vec![
                DetailedOdds {
                    _type: s::OddsComparisonType::Le,
                    threshold: 0.10,
                    attempt_rate: 0.00,
                    points: s::Points {
                        max_value: 1000,
                        percent: 0.001,
                    },
                },
                DetailedOdds {
                    _type: s::OddsComparisonType::Le,
                    threshold: 0.30,
                    attempt_rate: 0.00,
                    points: s::Points {
                        max_value: 5000,
                        percent: 0.01,
                    },
                },
            ]);
        }

        drop(config_ref);
        let res = prediction_logic(&streamer, "pred-key-1").await?;
        assert_eq!(res, None);

        {
            let pred = streamer.predictions.get_mut("pred-key-1").unwrap();
            pred.0.outcomes[2] = outcome_from(3, 45_000, 10);
        }
        let res = prediction_logic(&streamer, "pred-key-1").await?;
        assert_eq!(res, None);

        {
            let pred = streamer.predictions.get_mut("pred-key-1").unwrap();
            pred.0.outcomes[2] = outcome_from(3, 40_000, 10);
        }
        let res = prediction_logic(&streamer, "pred-key-1").await?;
        assert_eq!(
            res,
            Some((
                "3".to_owned(),
                (streamer.points as f64 * default_points_percentage) as u32
            ))
        );

        streamer.points = 500000;
        let res = prediction_logic(&streamer, "pred-key-1").await?;
        assert_eq!(res, Some(("3".to_owned(), default_max_points)));

        Ok(())
    }

    #[tokio::test]
    async fn detailed_strategy_high_odds() -> Result<()> {
        use common::config::strategy as s;
        let mut streamer = get_prediction();
        {
            let pred = streamer.predictions.get_mut("pred-key-1").unwrap();
            streamer.points = 50000;
            pred.0.outcomes = vec![
                outcome_from(1, 5_000, 2),
                outcome_from(2, 30_000, 14),
                outcome_from(3, 45_000, 10),
                outcome_from(4, 1_000, 1),
            ];
        }

        let high_odds_percentage = 0.001;

        let mut config_ref = streamer.config.0.write().unwrap();
        #[allow(irrefutable_let_patterns)]
        if let Strategy::Detailed(d) = &mut config_ref.config.prediction.strategy {
            d.default = DefaultPrediction {
                max_percentage: 0.55,
                min_percentage: 0.45,
                points: s::Points {
                    max_value: 40000,
                    percent: 0.15,
                },
            };

            d.detailed = Some(vec![
                DetailedOdds {
                    _type: s::OddsComparisonType::Le,
                    threshold: 0.10,
                    attempt_rate: 1.0,
                    points: s::Points {
                        max_value: 1000,
                        percent: high_odds_percentage,
                    },
                },
                DetailedOdds {
                    _type: s::OddsComparisonType::Le,
                    threshold: 0.30,
                    attempt_rate: 0.00,
                    points: s::Points {
                        max_value: 5000,
                        percent: 0.01,
                    },
                },
            ]);
        }

        drop(config_ref);
        let res = prediction_logic(&streamer, "pred-key-1").await?;
        assert_eq!(
            res,
            Some((
                "1".to_owned(),
                (streamer.points as f64 * high_odds_percentage) as u32
            ))
        );

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[rustfmt::skip]
    async fn event_listener(container: TestContainer<'_>) -> Result<()> {
        let gql_test = Client::new(
            "".to_owned(),
            format!("http://localhost:{}/gql", container.get_host_port_ipv4(3000)),
        );
        let metadata_uri = format!("http://localhost:{}/streamer_metadata", container.get_host_port_ipv4(3000));
        let spade_uri = format!("http://localhost:{}/base", container.get_host_port_ipv4(3000));

        let (events_tx, events_rx) = unbounded();
        let (ws_tx, ws_rx) = unbounded();
        let mut pubsub = PubSub::empty(events_tx.clone());
        pubsub.streamers = HashMap::from([
            (UserId::from_static("1"), StreamerState::default()),
        ]);

        let user_a = User {
            id: UserId::from_static("1"),
            stream: Some(Stream {
                id: UserId::from_static("2"),
                game: None,
            }),
        };

        let mock = reqwest::Client::new();
        mock.post(&metadata_uri)
            .json(&json!({ "1": ["a", user_a] })).send().await.unwrap();

        let pubsub = Arc::new(RwLock::new(pubsub));
        let live = spawn(live::run(events_tx, vec![(UserId::from_static("1"), StreamerInfo::with_channel_name("a"))], gql_test, spade_uri));
        let listener = spawn(super::event_listener(pubsub.clone(), events_rx, ws_tx, "".to_owned()));

        let res = timeout(Duration::from_secs(1), ws_rx.stream().take(3).collect::<Vec<_>>()).await;

        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.len(), 3);
        for item in res {
            let mut v = serde_json::from_str::<serde_json::Value>(&item).expect("Deserialize JSON");
            let item = traverse_json(&mut v, ".type");
            assert!(item.is_some());
            let item = item.unwrap();
            assert!(item.is_string());
            assert_eq!(item.as_str().unwrap(), "LISTEN");
        }

        let user_a = User {
            id: UserId::from_static("1"),
            stream: None,
        };
        mock.post(&metadata_uri)
            .json(&json!({ "1": ["a", user_a] })).send().await.unwrap();

        let res = timeout(Duration::from_secs(1), ws_rx.stream().take(3).collect::<Vec<_>>()).await;

        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.len(), 3);
        for item in res {
            let mut v = serde_json::from_str::<serde_json::Value>(&item).expect("Deserialize JSON");
            let item = traverse_json(&mut v, ".type");
            assert!(item.is_some());
            let item = item.unwrap();
            assert!(item.is_string());
            assert_eq!(item.as_str().unwrap(), "UNLISTEN");
        }

        live.abort();
        listener.abort();
        Ok(())
    }
}
