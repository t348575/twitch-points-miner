use std::{
    collections::HashMap,
    ops::Deref,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use common::{
    config::{self, filters::filter_matches, strategy, Config, ConfigType, StreamerConfig},
    remove_duplicates_in_place,
    twitch::{api, auth::Token, gql, ws::Request},
    types::{
        ConfigTypeRef, StreamerConfigRef, StreamerConfigRefWrapper, StreamerInfo, StreamerState,
    },
};
use eyre::{eyre, Context, ContextCompat, Result};
use flume::{unbounded, Receiver, Sender};
use indexmap::IndexMap;
use rand::Rng;
use serde::Serialize;
use tokio::{spawn, sync::RwLock, time::sleep};
use tracing::{debug, error, info, trace, warn};
use twitch_api::{
    pubsub::{
        community_points::CommunityPointsUserV1Reply,
        predictions::{Event, PredictionsChannelV1},
        raid::{Raid, RaidReply},
        video_playback::VideoPlaybackReply,
        TopicData, Topics,
    },
    types::UserId,
};

use crate::analytics::model::{PointsInfo, Prediction};

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
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
    pub ws_tx: Sender<Request>,
    #[serde(skip)]
    pub analytics: Arc<crate::analytics::AnalyticsWrapper>,
    #[serde(skip)]
    pub log_path: Option<String>,
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
        ws_tx: Sender<Request>,
        analytics: Arc<crate::analytics::AnalyticsWrapper>,
        log_path: Option<String>,
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
            ws_tx,
            analytics,
            gql,
            base_url: base_url.to_string(),
            log_path,
        })
    }

    #[cfg(test)]
    pub fn empty(ws_tx: Sender<Request>) -> Self {
        Self {
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
            ws_tx,
            log_path: Default::default(),
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
        ws_rx: Receiver<TopicData>,
        pubsub: Arc<RwLock<PubSub>>,
        gql: gql::Client,
    ) -> Result<()> {
        let (tx_watch_streams, rx_watch_streams) = unbounded();

        spawn(watch_streams(pubsub.clone(), rx_watch_streams));
        spawn(update_and_claim_points(pubsub.clone(), gql.clone()));
        spawn(update_spade_url(pubsub.clone()));

        while let Ok(data) = ws_rx.recv_async().await {
            if let TopicData::VideoPlaybackById { topic, reply } = &data {
                if let VideoPlaybackReply::StreamUp {
                    server_time: _,
                    play_delay: _,
                } = reply.deref()
                {
                    _ = tx_watch_streams
                        .send_async(UserId::from_str(&topic.channel_id.to_string()).unwrap())
                        .await;
                }
            }
            let res = pubsub.write().await.handle_response(data).await;

            if let Err(e) = res {
                warn!("Error handling response: {}", e);
            }
        }
        Ok(())
    }

    async fn handle_response(&mut self, data: TopicData) -> Result<()> {
        match data {
            TopicData::VideoPlaybackById { topic, reply } => {
                debug!("Got VideoPlaybackById {:#?}", topic);
                let channel_id = topic.channel_id;
                let topics = [
                    Topics::PredictionsChannelV1(PredictionsChannelV1 { channel_id }),
                    Topics::Raid(Raid { channel_id }),
                ];

                let streamer = self
                    .streamers
                    .get_mut(&UserId::from_str(&channel_id.to_string()).unwrap())
                    .context("Streamer does not exist")?;
                match *reply {
                    VideoPlaybackReply::StreamUp {
                        server_time: _,
                        play_delay: _,
                    } => {
                        info!("{} is live", streamer.info.channel_name);
                        for item in topics.into_iter().map(Request::Listen) {
                            self.ws_tx
                                .send_async(item)
                                .await
                                .context("Send ws command")?;
                        }
                    }
                    VideoPlaybackReply::StreamDown { server_time: _ } => {
                        streamer.info.live = false;
                        info!("{} is not live", streamer.info.channel_name);
                        for item in topics.into_iter().map(Request::UnListen) {
                            self.ws_tx
                                .send_async(item)
                                .await
                                .context("Send ws command")?;
                        }
                    }
                    _ => {}
                }
            }
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

                if let CommunityPointsUserV1Reply::ClaimClaimed {
                    timestamp: _,
                    claim,
                } = *reply
                {
                    if claim.user_id.as_str().ne(&self.user_id) {
                        return Ok(());
                    };

                    if self.streamers.contains_key(&claim.channel_id) {
                        debug!("Channel points updated for {}", claim.channel_id);
                        let s = self.streamers.get_mut(&claim.channel_id).unwrap();
                        s.points = claim.point_gain.total_points as u32;
                        s.last_points_refresh = Instant::now();
                    }
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

        if let Some((outcome_id, points_to_bet)) =
            prediction_logic(&s, event_id).context("Prediction logic")?
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
}

pub fn prediction_logic(streamer: &StreamerState, event_id: &str) -> Result<Option<(String, u32)>> {
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
            debug!("Filter matches {:#?}", filter);
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

mod watch_stream {
    use super::*;

    pub async fn inner(
        pubsub: &Arc<RwLock<PubSub>>,
        watch_streak: &mut Vec<(UserId, i32)>,
        use_watch_streak: bool,
        live_event: &Receiver<UserId>,
    ) -> Result<()> {
        if use_watch_streak {
            let live = live_event
                .drain()
                .map(|x| (x, 0))
                .filter(|x| !watch_streak.iter().any(|y| y.0 == x.0))
                .collect::<Vec<_>>();

            watch_streak.extend(live);
        }

        let (streamers, user_id, user_name, spade_url, access_token, config) = {
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
                reader.config.clone(),
            )
        };

        if streamers.is_empty() {
            trace!("No streamer found");
            return Ok(());
        }

        let watch_priority = config.watch_priority.unwrap_or_default();
        let mut watch_items = Vec::new();
        for item in &watch_priority {
            if let Some(s) = streamers.iter().find(|x| x.1.info.channel_name.eq(item)) {
                watch_items.push(s);
            }
        }

        // streamers not given in a priority order
        for item in config
            .streamers
            .iter()
            .filter(|x| streamers.iter().any(|y| y.1.info.channel_name.eq(x.0)))
        {
            if !watch_priority.contains(&item.0) {
                watch_items.push(
                    streamers
                        .iter()
                        .find(|x| x.1.info.channel_name.eq(item.0))
                        .unwrap(),
                );
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

        watch_items = remove_duplicates_in_place(watch_items, |a, b| a.0.eq(&b.0));
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
            .await
            .context(format!(
                "Could not set viewership {}",
                streamer.info.channel_name
            ))?;
        }

        *watch_streak = watch_streak.drain(..).filter(|x| x.1 < 31).collect();
        Ok(())
    }
}

async fn watch_streams(pubsub: Arc<RwLock<PubSub>>, live_event: Receiver<UserId>) {
    let use_watch_streak = {
        let reader = pubsub.read().await;
        reader.config.watch_streak.unwrap_or(true)
    };

    let mut watch_streak = Vec::new();

    loop {
        if let Err(err) =
            watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &live_event).await
        {
            if err.to_string() != "Spade URL not set" {
                error!("watch_streams {err}");
            }
        }

        #[cfg(test)]
        let time = 1;
        #[cfg(not(test))]
        let time = 10 * 1000;
        sleep(Duration::from_millis(time)).await;
    }
}

async fn update_and_claim_points(pubsub: Arc<RwLock<PubSub>>, gql: gql::Client) {
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

        let points = gql
            .get_channel_points(&channel_names)
            .await
            .context("Get channel points")?;
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
            error!("update_and_claim_points {err}");
        }

        sleep(Duration::from_secs(60)).await
    }
}

async fn update_spade_url(pubsub: Arc<RwLock<PubSub>>) {
    let base_url = { pubsub.read().await.base_url.clone() };
    async fn inner(pubsub: &Arc<RwLock<PubSub>>, base_url: &str) -> Result<()> {
        let a_live_stream = {
            let reader = pubsub.read().await;
            reader
                .streamers
                .iter()
                .find(|x| x.1.info.live)
                .map(|x| (x.0.clone(), x.1.clone()))
        };

        if let Some((_, streamer)) = a_live_stream {
            let spade_url = api::get_spade_url(&streamer.info.channel_name, base_url).await?;
            pubsub.write().await.spade_url = Some(spade_url);
            debug!("Updated spade url");
        }
        Ok(())
    }

    loop {
        if let Err(err) = inner(&pubsub, &base_url).await {
            error!("update_and_claim_points {err}");
        }

        sleep(Duration::from_secs(120)).await
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        str::FromStr,
        sync::Arc,
        time::{Duration, Instant},
    };

    use chrono::Local;
    use eyre::Result;
    use flume::unbounded;
    use rstest::rstest;
    use tokio::sync::RwLock;
    use twitch_api::{
        pubsub::predictions::{Event, Outcome},
        types::{Timestamp, UserId},
    };

    use common::{
        config::{strategy::*, ConfigType, PredictionConfig, StreamerConfig},
        testing::{container, TestContainer},
        types::*,
    };

    use crate::pubsub::prediction_logic;

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

    #[test]
    fn detailed_strategy_default() -> Result<()> {
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
        let res = prediction_logic(&streamer, "pred-key-1")?;
        assert_eq!(res, None);

        {
            let pred = streamer.predictions.get_mut("pred-key-1").unwrap();
            pred.0.outcomes[2] = outcome_from(3, 45_000, 10);
        }
        let res = prediction_logic(&streamer, "pred-key-1")?;
        assert_eq!(res, None);

        {
            let pred = streamer.predictions.get_mut("pred-key-1").unwrap();
            pred.0.outcomes[2] = outcome_from(3, 40_000, 10);
        }
        let res = prediction_logic(&streamer, "pred-key-1")?;
        assert_eq!(
            res,
            Some((
                "3".to_owned(),
                (streamer.points as f64 * default_points_percentage) as u32
            ))
        );

        streamer.points = 500000;
        let res = prediction_logic(&streamer, "pred-key-1")?;
        assert_eq!(res, Some(("3".to_owned(), default_max_points)));

        Ok(())
    }

    #[test]
    fn detailed_strategy_high_odds() -> Result<()> {
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
        let res = prediction_logic(&streamer, "pred-key-1")?;
        assert_eq!(
            res,
            Some((
                "1".to_owned(),
                (streamer.points as f64 * high_odds_percentage) as u32
            ))
        );

        Ok(())
    }

    macro_rules! watch_stream_eq {
        ($watching_uri:expr,$eq:expr) => {
            let res: Vec<UserId> = ureq::get(&$watching_uri).call()?.into_json()?;
            assert_eq!(res, $eq)
        };
        ($watching_uri:expr,$eq:expr,$user_ids:tt) => {
            let res: Vec<UserId> = ureq::get(&$watching_uri).call()?.into_json()?;
            assert_eq!(res.len(), $eq.len());
            for item in res {
                assert!($user_ids.contains(&item));
            }
        };
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test(flavor = "multi_thread")]
    async fn watch_stream_on_live(#[future] container: TestContainer) -> Result<()> {
        let container = container.await;

        let (ws_tx, _) = unbounded();
        let (_, rx) = unbounded();
        let mut pubsub = PubSub::empty(ws_tx);
        pubsub.spade_url = Some(format!("http://localhost:{}/spade", container.port));
        pubsub.user_id = "1".to_string();

        let user_ids = vec![UserId::from_static("1"), UserId::from_static("2")];
        pubsub.streamers = HashMap::from([
            (
                user_ids[0].clone(),
                StreamerState::new(true, user_ids[0].as_str().to_owned()),
            ),
            (
                user_ids[1].clone(),
                StreamerState::new(true, user_ids[1].as_str().to_owned()),
            ),
        ]);
        pubsub.config.streamers = user_ids
            .iter()
            .map(|x| {
                (
                    x.to_string(),
                    ConfigType::Specific(StreamerConfig::default()),
                )
            })
            .collect();

        let pubsub = Arc::new(RwLock::new(pubsub.clone()));
        let watching_uri = format!("http://localhost:{}/watching", container.port);
        let mut watch_streak = Vec::new();

        super::watch_stream::inner(&pubsub, &mut watch_streak, true, &rx).await?;
        watch_stream_eq!(watching_uri, user_ids, user_ids);

        Ok(())
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test(flavor = "multi_thread")]
    #[rustfmt::skip]
    async fn watch_stream_with_watch_streak(#[future] container: TestContainer) -> Result<()> {
        let container = container.await;

        let (ws_tx, _) = unbounded();
        let (tx, rx) = unbounded();
        let mut pubsub = PubSub::empty(ws_tx);
        pubsub.spade_url = Some(format!("http://localhost:{}/spade", container.port));
        pubsub.user_id = "1".to_string();
        pubsub.config.watch_streak = Some(true);

        let user_ids: Vec<UserId> = (1..4).map(|x| UserId::from_str(&x.to_string()).unwrap()).collect();
        pubsub.streamers = user_ids.iter().enumerate().map(|(idx, x)| (x.clone(), StreamerState::new(idx == 0, x.to_string()))).collect();
        pubsub.config.streamers = user_ids.iter().map(|x| { (x.to_string(), ConfigType::Specific(StreamerConfig::default()) )}).collect();

        let pubsub = Arc::new(RwLock::new(pubsub.clone()));
        let watching_uri = format!("http://localhost:{}/watching", container.port);

        let mut watch_streak = Vec::new();
        let use_watch_streak = true;

        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        watch_stream_eq!(watching_uri, [user_ids[0].clone()]);

        pubsub.write().await.streamers.get_mut(&user_ids[1]).unwrap().info.live = true;
        tx.send_async(user_ids[1].clone()).await?;
        ureq::delete(&watching_uri).call()?;
        for _ in 0..30 {
            super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
            watch_stream_eq!(watching_uri, user_ids[0..2], user_ids);
        }

        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        watch_stream_eq!(watching_uri, user_ids[0..2], user_ids);

        pubsub.write().await.streamers.get_mut(&user_ids[2]).unwrap().info.live = true;
        tx.send_async(user_ids[2].clone()).await?;
        ureq::delete(&watching_uri).call()?;
        for _ in 0..30 {
            super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
            watch_stream_eq!(watching_uri, [user_ids[0].clone(), user_ids[2].clone()], user_ids);
        }

        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        watch_stream_eq!(watching_uri, [user_ids[0].clone(), user_ids[2].clone()], user_ids);

        pubsub.write().await.config.watch_priority = Some(vec![user_ids[2].as_str().to_owned()]);
        ureq::delete(&watching_uri).call()?;
        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        watch_stream_eq!(watching_uri, [user_ids[0].clone(), user_ids[2].clone()], user_ids);

        pubsub.write().await.streamers.get_mut(&user_ids[2]).unwrap().info.live = false;
        ureq::delete(&watching_uri).call()?;
        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        watch_stream_eq!(watching_uri, user_ids[0..2], user_ids);

        pubsub.write().await.streamers.get_mut(&user_ids[0]).unwrap().info.live = false;
        ureq::delete(&watching_uri).call()?;
        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        watch_stream_eq!(watching_uri, user_ids[1..2], user_ids);

        pubsub.write().await.streamers.get_mut(&user_ids[1]).unwrap().info.live = false;
        ureq::delete(&watching_uri).call()?;
        super::watch_stream::inner(&pubsub, &mut watch_streak, use_watch_streak, &rx).await?;
        watch_stream_eq!(watching_uri, Vec::<UserId>::new(), user_ids);

        Ok(())
    }
}
