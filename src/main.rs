use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::{eyre, Context, Result};
use flume::unbounded;
use tokio::sync::RwLock;
use tokio::{fs, spawn};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg(feature = "web_api")]
mod web_api;

#[cfg(feature = "analytics")]
mod analytics;

mod config;
mod live;
mod pubsub;
mod twitch;
mod types;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Config file
    #[arg(short, long, default_value_t = String::from("config.yaml"))]
    config: String,
    /// API address to bind
    #[cfg(feature = "web_api")]
    #[arg(short, long, default_value_t = String::from("0.0.0.0:3000"))]
    address: String,
    /// Simulate predictions, don't actually make them
    #[arg(short, long, default_value_t = false)]
    simulate: bool,
    /// Token file
    #[arg(short, long, default_value_t = String::from("tokens.json"))]
    token: String,
    /// Log to file
    #[arg(short, long, default_value_t = false)]
    log_to_file: bool,
    /// Enable analytics, enabled by default
    #[cfg(feature = "analytics")]
    #[arg(long, default_value_t = true)]
    analytics: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let log_level = std::env::var("LOG").unwrap_or("warn".to_owned());
    let tracing_opts = tracing_subscriber::registry()
        .with(
            EnvFilter::new(&format!("twitch_points_miner={log_level}"))
                .add_directive(format!("tower_http::trace={log_level}").parse()?),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .compact(),
        );

    let file_appender = tracing_appender::rolling::never(".", "twitch-points-miner.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    if args.log_to_file {
        tracing_opts
            .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
            .init();
    } else {
        tracing_opts.init();
    }

    if !Path::new(&args.token).exists() {
        info!("Starting login sequence");
        twitch::auth::login(&args.token).await?;
    }

    let mut c: config::Config = serde_yaml::from_str(
        &fs::read_to_string(&args.config)
            .await
            .context("Reading config file")?,
    )
    .context("Parsing config file")?;
    info!("Parsed config file");

    if c.streamers.len() == 0 {
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

    let token: twitch::auth::Token = serde_json::from_str(
        &fs::read_to_string(args.token)
            .await
            .context("Reading tokens file")?,
    )
    .context("Parsing tokens file")?;
    info!("Parsed tokens file");

    let user_info = twitch::gql::get_user_id(&token.access_token).await?;
    let streamer_names = c.streamers.keys().map(|s| s.as_str()).collect::<Vec<_>>();
    let channels = twitch::gql::streamer_metadata(&streamer_names, &token.access_token)
        .await
        .wrap_err_with(|| "Could not get streamer list. Is your token valid?")?;
    info!("Got streamer list");

    for (idx, id) in channels.iter().enumerate() {
        if id.is_none() {
            return Err(eyre!(format!("Channel not found {}", streamer_names[idx])));
        }
    }

    #[cfg(feature = "analytics")]
    let analytics = if args.analytics {
        Arc::new(analytics::AnalyticsWrapper::new(
            &c.analytics_db.unwrap_or("analytics.db".to_owned()),
        )?)
    } else {
        Arc::new(analytics::AnalyticsWrapper(tokio::sync::Mutex::new(None)))
    };

    let channels = channels.into_iter().filter_map(|x| x).collect::<Vec<_>>();
    let points = twitch::gql::get_channel_points(
        &channels
            .iter()
            .map(|x| x.1.channel_name.as_str())
            .collect::<Vec<_>>(),
        &token.access_token,
    )
    .await?;

    #[cfg(feature = "analytics")]
    for (c, p) in channels.iter().zip(&points) {
        let id = c.0.as_str().parse::<i32>()?;
        let inserted = analytics
            .execute(|analytics| analytics.insert_streamer(id, c.1.channel_name.clone()))
            .await?;
        if inserted {
            analytics
                .execute(|analytics| {
                    analytics.insert_points(
                        id,
                        p.0 as i32,
                        analytics::model::PointsInfo::FirstEntry,
                    )
                })
                .await?;
        }
    }

    let active_predictions = twitch::gql::channel_points_context(
        &channels
            .iter()
            .map(|x| x.1.channel_name.as_str())
            .collect::<Vec<_>>(),
        &token.access_token,
    )
    .await?;

    println!("Everything ok, starting twitch pubsub");
    let (events_tx, events_rx) = unbounded::<live::Events>();
    let live = spawn(live::run(
        Arc::new(token.clone()),
        events_tx.clone(),
        channels.clone(),
    ));
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
        #[cfg(feature = "web_api")]
        live,
        #[cfg(feature = "web_api")]
        events_tx,
        #[cfg(feature = "analytics")]
        analytics,
    )?));

    let pubsub = spawn(pubsub::PubSub::run(
        token.clone(),
        events_rx,
        pubsub_data.clone(),
    ));

    #[cfg(feature = "web_api")]
    println!("Starting web api!");

    #[cfg(feature = "web_api")]
    let axum_server = web_api::get_api_server(args.address, pubsub_data, Arc::new(token)).await;

    #[cfg(feature = "web_api")]
    axum_server.await?;
    pubsub.await??;
    #[cfg(not(feature = "web_api"))]
    live.await??;

    Ok(())
}
