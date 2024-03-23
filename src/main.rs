use std::path::Path;

use clap::Parser;
use color_eyre::eyre::{eyre, Context, Result};
use tokio::{fs, join};
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

mod auth;
mod common;
mod config;
mod live;
mod pubsub;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Config file
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    config: String,
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
        auth::login().await?;
    }

    let c: config::Config = toml::from_str(
        &fs::read_to_string(&args.config)
            .await
            .context("Reading config file")?,
    )
    .context("Parsing config file")?;
    let token: auth::Token = serde_json::from_str(
        &fs::read_to_string("tokens.json")
            .await
            .context("Reading tokens file")?,
    )
    .context("Parsing tokens file")?;
    let channels = common::get_channel_ids(
        &c.streamers.keys().map(|s| s.as_str()).collect::<Vec<_>>(),
        &token.clone().into(),
    )
    .await
    .wrap_err_with(|| "Preparing streamer list")?;
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
        .zip(c.streamers.keys())
        .map(|x| (x.0, x.1.to_owned()))
        .collect::<Vec<_>>();
    let (events_tx, events_rx) = flume::unbounded();

    let (pubsub, live) = join!(
        pubsub::run(token.clone(), c, events_rx, channels.clone()),
        live::run(token, events_tx, channels)
    );

    pubsub.context("Pubsub")?;
    live.context("Live check")?;

    Ok(())
}
