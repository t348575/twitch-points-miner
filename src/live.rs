use std::{collections::HashMap, sync::Arc, time::Duration};

use color_eyre::{eyre::Context, Result};
use flume::Sender;
use tokio::time::{interval, sleep};
use tracing::{debug, error};
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
    token: Arc<Token>,
    events_tx: Sender<Events>,
    channels: Vec<(UserId, StreamerInfo)>,
) -> Result<()> {
    debug!("{channels:#?}");

    async fn inner(
        access_token: &str,
        events_tx: &Sender<Events>,
        channels: &[(UserId, StreamerInfo)],
    ) -> Result<()> {
        let mut channels_status: HashMap<UserId, bool> =
            channels.into_iter().map(|x| (x.0.clone(), false)).collect();

        let channel_names = channels
            .iter()
            .map(|x| x.1.channel_name.as_str())
            .collect::<Vec<_>>();

        let mut interval = interval(Duration::from_secs(60));
        let mut count = 10;
        loop {
            let live_channels = gql::streamer_metadata(&channel_names, access_token)
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
                        .send_async(Events::Live {
                            channel_id: user_id.clone(),
                            broadcast_id: info.broadcast_id,
                        })
                        .await
                        .context(format!("Send live event for {user_id}"))?;
                }
            }

            if get_spade_using.len() > 0 && count == 10 {
                events_tx
                    .send_async(Events::SpadeUpdate(
                        api::get_spade_url(&get_spade_using).await?,
                    ))
                    .await?;
                count = 0;
            }

            count += 1;
            interval.tick().await;
        }
    }

    loop {
        if let Err(err) = inner(&token.access_token, &events_tx, &channels).await {
            error!("{err}");
        }

        sleep(Duration::from_secs(60)).await
    }
}
