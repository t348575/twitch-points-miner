use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::{eyre, Context, Result};
use tokio::sync::{mpsc, RwLock};
use tokio::{fs, spawn};
use tracing::info;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use validator::Validate;

use crate::config::Normalize;

#[cfg(feature = "web_api")]
mod web_api;
// mod auth;
// mod common;
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
    /// Simulate
    #[arg(short, long, default_value_t = false)]
    simulate: bool,
    /// Token file
    #[arg(short, long, default_value_t = String::from("tokens.json"))]
    token: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("LOG"))
        .init();

    let args = Args::parse();

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

    for s in c.streamers.values_mut() {
        s.validate()?;
        s.strategy.normalize();
    }

    let token: twitch::auth::Token = serde_json::from_str(
        &fs::read_to_string(args.token)
            .await
            .context("Reading tokens file")?,
    )
    .context("Parsing tokens file")?;
    info!("Parsed tokens file");

    let user_id = twitch::gql::get_user_id(&token.access_token).await?;

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

    println!("Everything ok, starting twitch pubsub");

    let (events_tx, events_rx) = mpsc::channel(128);
    let channels = channels.into_iter().map(|x| x.unwrap());
    let pubsub_data = Arc::new(RwLock::new(pubsub::PubSub::new(
        channels.clone().zip(c.streamers.values()).collect(),
        args.simulate,
        token.clone(),
        user_id,
    )?));
    let pubsub = spawn(pubsub::PubSub::run(
        token.clone(),
        c,
        events_rx,
        pubsub_data.clone(),
    ));
    let live = spawn(live::run(token.clone(), events_tx, channels.collect()));

    #[cfg(feature = "web_api")]
    let axum_server = web_api::get_api_server(args.address, pubsub_data, Arc::new(token)).await;

    #[cfg(feature = "web_api")]
    axum_server.await?;
    pubsub.await??;
    live.await??;

    Ok(())
}
