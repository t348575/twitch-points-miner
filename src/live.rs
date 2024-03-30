use std::{collections::HashMap, time::Duration};

use color_eyre::{eyre::Context, Result};
use tokio::{sync::mpsc::Sender, time::interval};
use tracing::debug;
use twitch_api::types::UserId;

use crate::{
    twitch::{api, auth::Token, gql},
    types::StreamerInfo,
};

#[derive(Debug, Clone)]
pub enum Events {
    Live {
        channel_id: UserId,
        broadcast_id: Option<UserId>,
    },
    SpadeUpdate(String),
}

pub async fn run(
    token: Token,
    events_tx: Sender<Events>,
    channels: Vec<(UserId, StreamerInfo)>,
) -> Result<()> {
    debug!("{channels:#?}");

    let mut channels_status: HashMap<UserId, bool> = channels
        .clone()
        .into_iter()
        .map(|x| (x.0.clone(), false))
        .collect();

    let channel_names = channels
        .iter()
        .map(|x| x.1.channel_name.as_str())
        .collect::<Vec<_>>();

    let mut interval = interval(Duration::from_secs(2 * 60));
    loop {
        let live_channels = gql::streamer_metadata(&channel_names, &token.access_token)
            .await
            .context("Live channels")?
            .drain(..)
            .filter(|x| x.is_some())
            .map(|x| x.unwrap())
            .collect::<Vec<_>>();

        let mut get_spade_using = String::new();
        for (idx, (user_id, info)) in live_channels.into_iter().enumerate() {
            if info.broadcast_id.is_some() {
                get_spade_using = channels[idx].1.channel_name.clone();
            }

            if channels_status[&user_id] != info.broadcast_id.is_some() {
                *channels_status.get_mut(&user_id).unwrap() = info.broadcast_id.is_some();
                events_tx
                    .send(Events::Live {
                        channel_id: user_id,
                        broadcast_id: info.broadcast_id,
                    })
                    .await
                    .context("Send event")?;
            }
        }

        if get_spade_using.len() > 0 {
            events_tx
                .send(Events::SpadeUpdate(
                    api::get_spade_url(&get_spade_using).await?,
                ))
                .await?;
        }

        interval.tick().await;
    }
}
