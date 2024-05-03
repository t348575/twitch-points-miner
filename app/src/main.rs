use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use common::twitch::ws::{Request, WsPool};
use eyre::{eyre, Context, Result};
use tokio::sync::RwLock;
use tokio::{fs, spawn};
use tracing::info;
use tracing_subscriber::fmt::format::{Compact, DefaultFields};
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use twitch_api::pubsub::community_points::CommunityPointsUserV1;
use twitch_api::pubsub::video_playback::{VideoPlaybackById, VideoPlaybackReply};
use twitch_api::pubsub::{TopicData, Topics};

use crate::analytics::{Analytics, AnalyticsWrapper};

mod analytics;
// mod live;
mod pubsub;
mod web_api;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Config file
    #[arg(short, long, default_value_t = String::from("config.yaml"))]
    config: String,
    /// API address to bind
    #[arg(short, long, default_value_t = String::from("0.0.0.0:3000"))]
    address: String,
    /// Simulate predictions, don't actually make them
    #[arg(short, long, default_value_t = false)]
    simulate: bool,
    /// Token file
    #[arg(short, long, default_value_t = String::from("tokens.json"))]
    token: String,
    /// Log to file
    #[arg(short, long)]
    log_file: Option<String>,
    /// Analytics database path
    #[arg(long, default_value_t = String::from("analytics.db"))]
    analytics_db: String,
}

const BASE_URL: &str = "https://twitch.tv";

fn get_layer<S>(
    layer: tracing_subscriber::fmt::Layer<S>,
) -> tracing_subscriber::fmt::Layer<
    S,
    DefaultFields,
    tracing_subscriber::fmt::format::Format<Compact, ChronoLocal>,
> {
    layer
        .with_timer(ChronoLocal::new("%v %k:%M:%S %z".to_owned()))
        .compact()
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = std::env::var("LOG").unwrap_or("warn".to_owned());
    let tracing_opts = tracing_subscriber::registry()
        .with(
            EnvFilter::new(format!("twitch_points_miner={log_level}"))
                .add_directive(format!("common={log_level}").parse()?)
                .add_directive(format!("tower_http::trace={log_level}").parse()?),
        )
        .with(get_layer(tracing_subscriber::fmt::layer()));

    let file_appender = tracing_appender::rolling::never(
        ".",
        args.log_file.clone().unwrap_or("log.log".to_owned()),
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    if args.log_file.is_some() {
        tracing_opts
            .with(get_layer(tracing_subscriber::fmt::layer()).with_writer(non_blocking))
            .init();
    } else {
        tracing_opts.init();
    }

    if !Path::new(&args.token).exists() {
        info!("Starting login sequence");
        common::twitch::auth::login(&args.token).await?;
    }

    let mut c: common::config::Config = serde_yaml::from_str(
        &fs::read_to_string(&args.config)
            .await
            .context("Reading config file")?,
    )
    .context("Parsing config file")?;
    info!("Parsed config file");

    if c.streamers.is_empty() {
        return Err(eyre!("No streamers in config file"));
    }

    let c_original = c.clone();
    c.parse_and_validate()?;

    for item in c.watch_priority.clone().unwrap_or_default() {
        if !c.streamers.contains_key(&item) {
            return Err(eyre!(format!(
                "Channel in watch_priority not found in streamers list {item}"
            )));
        }
    }

    let token: common::twitch::auth::Token = serde_json::from_str(
        &fs::read_to_string(args.token)
            .await
            .context("Reading tokens file")?,
    )
    .context("Parsing tokens file")?;
    info!("Parsed tokens file");

    let gql = common::twitch::gql::Client::new(
        token.access_token.clone(),
        "https://gql.twitch.tv/gql".to_owned(),
    );
    let user_info = gql.get_user_id().await?;
    let streamer_names = c.streamers.keys().map(|s| s.as_str()).collect::<Vec<_>>();
    let channels = gql
        .streamer_metadata(&streamer_names)
        .await
        .wrap_err_with(|| "Could not get streamer list. Is your token valid?")?;
    info!("Got streamer list");

    for (idx, id) in channels.iter().enumerate() {
        if id.is_none() {
            return Err(eyre!(format!("Channel not found {}", streamer_names[idx])));
        }
    }

    let (mut analytics, analytics_tx) = Analytics::new(&args.analytics_db)?;

    let channels = channels.into_iter().flatten().collect::<Vec<_>>();
    let points = gql
        .get_channel_points(
            &channels
                .iter()
                .map(|x| x.1.channel_name.as_str())
                .collect::<Vec<_>>(),
        )
        .await?;

    for (c, p) in channels.iter().zip(&points) {
        let id = c.0.as_str().parse::<i32>()?;
        let inserted = analytics.insert_streamer(id, c.1.channel_name.clone())?;
        if inserted {
            analytics.insert_points(id, p.0 as i32, analytics::model::PointsInfo::FirstEntry)?;
        }
    }

    let active_predictions = gql
        .channel_points_context(
            &channels
                .iter()
                .map(|x| x.1.channel_name.as_str())
                .collect::<Vec<_>>(),
        )
        .await?;

    info!("Config OK!");
    let (ws_pool, ws_tx, (ws_data_tx, ws_rx)) = WsPool::start(
        &token.access_token,
        #[cfg(test)]
        String::new(),
    )
    .await;

    channels.iter().for_each(|x| {
        let channel_id = x.0.as_str().parse().unwrap();

        if x.1.live {
            // send initial live messages
            _ = ws_data_tx.send(TopicData::VideoPlaybackById {
                topic: VideoPlaybackById { channel_id },
                reply: Box::new(VideoPlaybackReply::StreamUp {
                    server_time: 0.0,
                    play_delay: 0,
                }),
            });
        }

        ws_tx
            .send(Request::Listen(Topics::VideoPlaybackById(
                VideoPlaybackById { channel_id },
            )))
            .expect("Could not add streamer to pubsub");
    });
    ws_tx
        .send_async(Request::Listen(Topics::CommunityPointsUserV1(
            CommunityPointsUserV1 {
                channel_id: user_info.0.parse().unwrap(),
            },
        )))
        .await
        .context("Could not add user to pubsub")?;
    // we definitely do not want to keep this in scope
    drop(ws_data_tx);

    let pubsub_data = Arc::new(RwLock::new(pubsub::PubSub::new(
        c_original,
        args.config,
        channels
            .clone()
            .into_iter()
            .zip(c.streamers.values())
            .collect(),
        points,
        active_predictions,
        c.presets.unwrap_or_default(),
        args.simulate,
        token.clone(),
        user_info,
        gql.clone(),
        BASE_URL,
        ws_tx,
        Arc::new(AnalyticsWrapper::new(analytics)),
        analytics_tx.clone(),
    )?));

    let pubsub = spawn(pubsub::PubSub::run(
        ws_rx,
        pubsub_data.clone(),
        gql,
        analytics_tx,
    ));

    info!("Starting web api!");

    let axum_server = web_api::get_api_server(
        args.address,
        pubsub_data,
        Arc::new(token),
        &args.analytics_db,
        args.log_file,
    )
    .await?;

    axum_server.await?;
    pubsub.await??;
    ws_pool.await?;

    Ok(())
}
