use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, RwLock},
};

use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use flume::{Receiver, Sender};
use futures::StreamExt;
use rand::distributions::{Alphanumeric, DistString};
use tokio::spawn;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};
use twitch_api::{
    pubsub::{
        listen_command,
        predictions::{Event as PredictionEvent, PredictionsChannelV1},
        Response, TopicData, Topics,
    },
    types::UserId,
};

use crate::{
    auth::Token,
    common::{connect_twitch_ws, ping_loop, writer},
    config::Config,
    live::Events,
};

struct PubSub {
    streamers: HashMap<UserId, Streamer>,
}

#[derive(Debug)]
pub struct Streamer {
    pub name: String,
    pub live: bool,
    pub predictions: HashMap<String, PredictionEvent>,
}

pub async fn run(
    token: Token,
    config: Config,
    events_rx: Receiver<Events>,
    channels: Vec<(UserId, String)>,
) -> Result<()> {
    let (write, mut read) =
        connect_twitch_ws("wss://pubsub-edge.twitch.tv", &token.access_token).await?;
    info!("Connected to pubsub");

    let pubsub = Arc::new(RwLock::new(PubSub::new(channels.clone())));
    let (tx, rx) = flume::unbounded();
    spawn(writer(rx, write));
    spawn(ping_loop(tx.clone()));
    spawn(event_listener(pubsub.clone(), events_rx, tx.clone()));

    let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let cmd = listen_command(
        &[Topics::PredictionsChannelV1(PredictionsChannelV1 {
            channel_id: channels[0].0.as_str().parse()?,
        })],
        token.access_token.as_str(),
        nonce.as_str(),
    )?;
    tx.send_async(cmd).await?;

    while let Some(Ok(msg)) = read.next().await {
        if let Message::Text(m) = msg {
            match Response::parse(&m) {
                Ok(r) => {
                    if let Response::Message { data } = r {
                        handle_response(data, &pubsub)
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
) -> Result<()> {
    while let Ok(events) = events_rx.recv_async().await {
        let mut pubsub = pubsub
            .write()
            .map_err(|_| eyre!("Failed to get pubsub lock"))?;
        match events {
            Events::Live(id, status) => {
                if let Some(s) = pubsub.streamers.get_mut(&id) {
                    s.live = status;
                }
            }
        }
    }
    Ok(())
}

impl PubSub {
    pub fn new(channels: Vec<(UserId, String)>) -> PubSub {
        let streamers = channels
            .into_iter()
            .map(|(id, name)| {
                (
                    id,
                    Streamer {
                        name,
                        live: false,
                        predictions: HashMap::new(),
                    },
                )
            })
            .collect();
        PubSub { streamers }
    }
}

async fn handle_response(data: TopicData, pubsub: &Arc<RwLock<PubSub>>) -> Result<()> {
    match data {
        TopicData::PredictionsChannelV1 { topic, reply } => {
            info!("Got reply {:#?}", topic);
            let event = reply.data.event;

            let streamer = UserId::from_str(&event.channel_id)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(event.created_at.as_str())
                .context("Parse timestamp")?;
            let now = chrono::Local::now();
            let one_minute = std::time::Duration::from_secs(60);

            if event.ended_at.is_none()
                && created_at < (now + one_minute)
                && created_at > (now - one_minute)
            {
                let reader = pubsub
                    .read()
                    .map_err(|_| eyre!("Failed to get pubsub lock"))?;
                if reader.streamers.contains_key(&streamer)
                    && reader.streamers[&streamer]
                        .predictions
                        .contains_key(event.id.as_str())
                {
                    drop(reader);
                    let mut writer = pubsub
                        .write()
                        .map_err(|_| eyre!("Failed to get pubsub lock"))?;
                    writer
                        .streamers
                        .get_mut(&streamer)
                        .unwrap()
                        .predictions
                        .insert(event.id.clone(), event);
                    // TODO: perform selection logic
                }
            } else if event.ended_at.is_some() {
                let mut writer = pubsub
                    .write()
                    .map_err(|_| eyre!("Failed to get pubsub lock"))?;
                writer
                    .streamers
                    .get_mut(&streamer)
                    .unwrap()
                    .predictions
                    .remove(event.id.as_str());
                // TODO: write results to db
            }
        }
        _ => {}
    }
    Ok(())
}
