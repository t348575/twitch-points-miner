use std::{collections::HashMap, time::Duration};

use color_eyre::{eyre::Context, Result};
use common::{
    twitch::{api, gql},
    types::StreamerInfo,
};
use flume::Sender;
use tokio::time::{interval, sleep};
use tracing::{debug, error};
use twitch_api::types::UserId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Events {
    Live {
        channel_id: UserId,
        broadcast_id: Option<UserId>,
    },
    SpadeUpdate(String),
}

pub async fn run(
    events_tx: Sender<Events>,
    channels: Vec<(UserId, StreamerInfo)>,
    gql: gql::Client,
    base_url: String,
) -> Result<()> {
    debug!("{channels:#?}");

    loop {
        if let Err(err) = inner(&events_tx, &channels, &gql, &base_url, 60 * 1000).await {
            error!("{err}");
        }

        sleep(Duration::from_secs(60)).await
    }
}

async fn inner(
    events_tx: &Sender<Events>,
    channels: &[(UserId, StreamerInfo)],
    gql: &gql::Client,
    base_url: &str,
    wait: u64,
) -> Result<()> {
    let mut channels_status: HashMap<UserId, bool> =
        channels.iter().map(|x| (x.0.clone(), false)).collect();

    let channel_names = channels
        .iter()
        .map(|x| x.1.channel_name.as_str())
        .collect::<Vec<_>>();

    let mut interval = interval(Duration::from_millis(wait));
    let mut count = 10;
    loop {
        let live_channels = gql
            .streamer_metadata(&channel_names)
            .await
            .context("Live channels")?
            .drain(..)
            .flatten()
            .collect::<Vec<_>>();

        let mut get_spade_using = String::new();
        for (idx, (user_id, info)) in live_channels.into_iter().enumerate() {
            if info.broadcast_id.is_some() {
                get_spade_using.clone_from(&channels[idx].1.channel_name);
            }

            if channels_status[&user_id] != info.live {
                *channels_status.get_mut(&user_id).unwrap() = info.live;
                events_tx
                    .send_async(Events::Live {
                        channel_id: user_id.clone(),
                        broadcast_id: info.broadcast_id,
                    })
                    .await
                    .context(format!("Send live event for {user_id}"))?;
            }
        }

        if !get_spade_using.is_empty() && count == 10 {
            events_tx
                .send_async(Events::SpadeUpdate(
                    api::get_spade_url(&get_spade_using, base_url).await?,
                ))
                .await?;
            count = 0;
        }

        count += 1;
        interval.tick().await;
    }
}

#[cfg(test)]
mod test {
    use common::twitch::gql::Client;
    use flume::unbounded;
    use rstest::rstest;
    use serde_json::json;
    use test::gql::{Stream, User};
    use testcontainers::{Container, GenericImage};
    use tokio::{spawn, time::timeout};

    use crate::test::container;

    use super::*;

    #[rstest]
    #[tokio::test]
    #[rustfmt::skip]
    async fn live_sequence(container: Container<'_, GenericImage>) {
        env_logger::init();
        let gql_test = Client::new(
            "".to_owned(),
            format!("http://localhost:{}/gql", container.get_host_port_ipv4(3000)),
        );
        let metadata_uri = format!("http://localhost:{}/streamer_metadata", container.get_host_port_ipv4(3000));
        let spade_uri = format!("http://localhost:{}/base", container.get_host_port_ipv4(3000));

        let mut user_a = User {
            id: UserId::from_static("1"),
            stream: Some(Stream {
                id: UserId::from_static("2"),
                game: None,
            }),
        };

        let mut user_b = User {
            id: UserId::from_static("3"),
            stream: None,
        };

        let mock = reqwest::Client::new();
        mock.post(&metadata_uri)
            .json(&json!({
                "1": ["a", user_a],
                "3": ["b", user_b]
            })).send().await.unwrap();

        let events_expected = [
            Events::Live { channel_id: UserId::from_static("1"), broadcast_id: Some(UserId::from_static("2")) },
            Events::Live { channel_id: UserId::from_static("3"), broadcast_id: Some(UserId::from_static("4")) },
            Events::Live { channel_id: UserId::from_static("1"), broadcast_id: None },
            Events::Live { channel_id: UserId::from_static("3"), broadcast_id: None },
        ];

        let (events_tx, events_rx) = unbounded::<Events>();
        let channels = vec![
            (UserId::from_static("1"), StreamerInfo::with_channel_name("a")),
            (UserId::from_static("3"), StreamerInfo::with_channel_name("b")),
        ];
        let live = spawn(async move { inner(&events_tx, &channels, &gql_test, &spade_uri, 50).await });

        let res = timeout(Duration::from_secs(1), events_rx.recv_async()).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().unwrap(), events_expected[0]);

        user_b.stream = Some(Stream {
            id: UserId::from_static("4"),
            game: None,
        });
        mock.post(&metadata_uri)
            .json(&json!({
                "1": ["a", user_a],
                "3": ["b", user_b]
            })).send().await.unwrap();

        // ignore spade url
        _ = timeout(Duration::from_secs(1), events_rx.recv_async()).await;
        let res = timeout(Duration::from_secs(1), events_rx.recv_async()).await;
        assert_eq!(res.unwrap().unwrap(), events_expected[1]);

        user_a.stream = None;
        mock.post(&metadata_uri)
            .json(&json!({
                "1": ["a", user_a],
                "3": ["b", user_b]
            })).send().await.unwrap();

        let res = timeout(Duration::from_secs(1), events_rx.recv_async()).await;
        assert_eq!(res.unwrap().unwrap(), events_expected[2]);

        user_b.stream = None;
        mock.post(&metadata_uri)
            .json(&json!({
                "1": ["a", user_a],
                "3": ["b", user_b]
            })).send().await.unwrap();

        let res = timeout(Duration::from_secs(1), events_rx.recv_async()).await;
        assert_eq!(res.unwrap().unwrap(), events_expected[3]);

        live.abort();
    }
}
