use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::{eyre, Context, Result};
use tokio::sync::RwLock;
use tokio::{fs, spawn};
use tracing::info;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use validator::Validate;

#[cfg(feature = "api")]
mod api;
mod auth;
mod common;
mod config;
mod live;
mod pubsub;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Config file
    #[arg(short, long, default_value_t = String::from("config.yaml"))]
    config: String,
    /// API address to bind
    #[cfg(feature = "api")]
    #[arg(short, long, default_value_t = String::from("0.0.0.0:3000"))]
    address: String,
    /// Simulate
    #[arg(short, long, default_value_t = false)]
    simulate: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("LOG"))
        .init();

    let args = Args::parse();

    if !Path::new("tokens.json").exists() {
        info!("Starting login sequence");
        auth::login().await?;
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

    for s in c.streamers.values_mut() {
        s.validate()?;
        s.strategy.normalize();
    }

    let token: auth::Token = serde_json::from_str(
        &fs::read_to_string("tokens.json")
            .await
            .context("Reading tokens file")?,
    )
    .context("Parsing tokens file")?;
    info!("Parsed tokens file");

    let channels = common::get_channel_ids(
        &c.streamers.keys().map(|s| s.as_str()).collect::<Vec<_>>(),
        &token.clone().into(),
    )
    .await
    .wrap_err_with(|| "Could not get streamer list. Is your token valid?")?;
    info!("Got streamer list");

    for (idx, id) in channels.iter().enumerate() {
        if id.is_none() {
            return Err(eyre!(format!(
                "Channel not found {}",
                c.streamers.keys().skip(idx).take(1).collect::<Vec<_>>()[0]
            )));
        }
    }

    let channels = channels
        .into_iter()
        .map(|x| x.unwrap())
        .zip(c.streamers.clone())
        .map(|x| (x.0, x.1 .0, x.1 .1))
        .collect::<Vec<_>>();
    let (events_tx, events_rx) = flume::unbounded();

    println!("Everything ok, starting twitch pubsub");

    let pubsub_data = Arc::new(RwLock::new(pubsub::PubSub::new(
        channels.clone(),
        args.simulate,
    )));
    let pubsub = spawn(pubsub::run(
        token.clone(),
        c,
        events_rx,
        pubsub_data.clone(),
    ));
    let live = spawn(live::run(token.clone(), events_tx, channels));

    #[cfg(feature = "api")]
    let axum_server = api::get_api_server(args.address, pubsub_data, Arc::new(token)).await;

    #[cfg(feature = "api")]
    axum_server.await?;
    pubsub.await??;
    live.await??;

    Ok(())
}
