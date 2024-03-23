use std::{collections::HashMap, time::Duration};

use color_eyre::{eyre::Context, Result};
use flume::Sender;
use tokio::time::interval;
use twitch_api::types::UserId;

use crate::{auth::Token, common};

#[derive(Debug, Clone)]
pub enum Events {
    Live(UserId, bool),
}

pub async fn run(
    token: Token,
    events_tx: Sender<Events>,
    channels: Vec<(UserId, String)>,
) -> Result<()> {
    let twitch_api_token = token.into();
    let channels = channels.iter().map(|x| x.0.clone()).collect::<Vec<_>>();

    let mut channels_status: HashMap<UserId, bool> = channels
        .clone()
        .into_iter()
        .map(|x| (x.clone(), false))
        .collect();
    let mut interval = interval(Duration::from_secs(2 * 60));
    loop {
        let live_channels = common::live_channels(&channels, &twitch_api_token)
            .await
            .context("Live channels")?;

        for (user_id, live) in live_channels {
            if channels_status[&user_id] != live {
                *channels_status.get_mut(&user_id).unwrap() = live;
                events_tx
                    .send_async(Events::Live(user_id, live))
                    .await
                    .context("Send event")?;
            }
        }
        interval.tick().await;
    }
}
