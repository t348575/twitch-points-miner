use std::{collections::HashMap, time::Duration};

use color_eyre::{eyre::Context, Result};
use tokio::{sync::mpsc::Sender, time::interval};
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
    let mut channels_status: HashMap<UserId, bool> = channels
        .clone()
        .into_iter()
        .map(|x| (x.0.clone(), false))
        .collect();

    let channel_ids = channels
        .iter()
        .map(|x| x.0.clone())
        .collect::<Vec<UserId>>();

    let mut interval = interval(Duration::from_secs(2 * 60));
    loop {
        let live_channels = gql::live_channels(&channel_ids, &token.access_token)
            .await
            .context("Live channels")?;

        let mut get_spade_using = String::new();
        for (idx, (user_id, broadcast_id)) in live_channels.into_iter().enumerate() {
            if broadcast_id.is_some() {
                get_spade_using = channels[idx].1.channel_name.clone();
            }

            if channels_status[&user_id] != broadcast_id.is_some() {
                *channels_status.get_mut(&user_id).unwrap() = broadcast_id.is_some();
                events_tx
                    .send(Events::Live {
                        channel_id: user_id,
                        broadcast_id: broadcast_id,
                    })
                    .await
                    .context("Send event")?;
            }
        }

        events_tx
            .send(Events::SpadeUpdate(
                api::get_spade_url(&get_spade_using).await?,
            ))
            .await?;

        interval.tick().await;
    }
}
