use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use eyre::{Context, Report, Result};
use flume::{Receiver, Sender};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use rand::distributions::{Alphanumeric, DistString};
use serde_json::json;
use tokio::{
    net::TcpStream,
    spawn,
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, timeout},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, info, trace, warn};
use twitch_api::pubsub::{
    listen_command, unlisten_command,
    video_playback::{VideoPlaybackById, VideoPlaybackReply},
    Response, TopicData, Topics,
};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct WsPool {
    connections: Vec<WsConn>,
    rx: Receiver<Request>,
    tx: Sender<TopicData>,
    access_token: String,
    #[cfg(feature = "testing")]
    base_url: String,
}

#[derive(Debug, PartialEq)]
pub enum Request {
    Listen(Topics),
    UnListen(Topics),
}

struct WsConn {
    reader: JoinHandle<Result<()>>,
    writer: SplitSink<WsStream, Message>,
    topics: Vec<(Topics, String)>,
    state: Arc<Mutex<WsConnState>>,
    access_token: String,
}

#[derive(Debug, Clone)]
struct WsConnState {
    last_update: Instant,
    stream_state: WsStreamState,
    // string of command nonce's
    retry_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum WsStreamState {
    Open,
    Reconnect,
}

impl Drop for WsPool {
    fn drop(&mut self) {
        for item in &self.connections {
            item.reader.abort();
        }
    }
}

impl WsPool {
    pub async fn start(
        access_token: &str,
        #[cfg(feature = "testing")] base_url: String,
    ) -> (
        JoinHandle<()>,
        Sender<Request>,
        (Sender<TopicData>, Receiver<TopicData>),
    ) {
        let (req_tx, req_rx) = flume::unbounded();
        let (res_tx, res_rx) = flume::unbounded();

        let pool = spawn(WsPool::run(WsPool {
            connections: vec![],
            rx: req_rx,
            tx: res_tx.clone(),
            access_token: access_token.to_owned(),
            #[cfg(feature = "testing")]
            base_url,
        }));

        (pool, req_tx, (res_tx, res_rx))
    }

    async fn run(mut self) {
        loop {
            if self.connections.is_empty() {
                self.retry_add_connection().await;
            }

            let recv = timeout(
                Duration::from_millis(
                    #[cfg(feature = "testing")]
                    1,
                    #[cfg(not(feature = "testing"))]
                    250,
                ),
                self.rx.recv_async(),
            )
            .await;

            match recv {
                Ok(Ok(Request::Listen(topic))) => {
                    debug!("Got request to add topic {topic:#?}");
                    let topic_already_exists = self
                        .connections
                        .iter()
                        .flat_map(|x| x.topics.clone())
                        .find(|x| x.0.eq(&topic));
                    if topic_already_exists.is_none() {
                        self.listen_command(topic).await
                    } else {
                        debug!("Got request to add existing topic {topic:#?}");
                    }
                }
                Ok(Ok(Request::UnListen(topic))) => {
                    debug!("Got request to remove topic {topic:#?}");
                    let mut conn = None;
                    self.connections = self
                        .connections
                        .drain(..)
                        .filter_map(|x| {
                            if x.topics.iter().any(|x| x.0.eq(&topic)) && conn.is_none() {
                                conn = Some(x);
                                None
                            } else {
                                Some(x)
                            }
                        })
                        .collect();

                    if let Some(mut conn) = conn {
                        let res = conn.unlisten_topic(&topic).await;
                        conn.topics.retain(|x| x.0.ne(&topic));
                        if res.is_err() {
                            conn = self.reconnect(conn).await;
                        }
                        self.connections.push(conn);
                    }

                    // Send a not-live message back to other listeners, so they can destruct any events they have subscribed to
                    if let Topics::VideoPlaybackById(VideoPlaybackById { channel_id }) = topic {
                        info!("Unlisten on stream {channel_id}");
                        _ = self
                            .tx
                            .send_async(TopicData::VideoPlaybackById {
                                topic: VideoPlaybackById { channel_id },
                                reply: Box::new(VideoPlaybackReply::StreamDown {
                                    server_time: 0.0,
                                }),
                            })
                            .await;
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => {}
            }

            for mut conn in self.connections.drain(..).collect::<Vec<_>>() {
                let state = { conn.state.lock().await.clone() };
                if state.stream_state == WsStreamState::Reconnect {
                    conn = self.reconnect(conn).await;
                }

                if state.last_update.elapsed() > Duration::from_secs(60) {
                    if let Err(err) = conn
                        .writer
                        .send(Message::Text(json!({"type": "PING"}).to_string()))
                        .await
                    {
                        warn!("Connection closed {:#?}", err);
                        conn = self.reconnect(conn).await;
                    }

                    let ping = timeout(Duration::from_secs(10), async {
                        loop {
                            sleep(Duration::from_millis(1)).await;
                            let last_update = { conn.state.lock().await.last_update };
                            if last_update.elapsed() < Duration::from_secs(60) {
                                return;
                            }
                        }
                    })
                    .await;
                    if ping.is_err() {
                        warn!("Twitch pubsub did not respond to ping");
                        conn = self.reconnect(conn).await;
                    }
                }

                if !state.retry_commands.is_empty() {
                    for nonce in state.retry_commands {
                        let mut topic = None;
                        conn.topics = conn
                            .topics
                            .drain(..)
                            .filter_map(|x| {
                                if x.1.eq(&nonce) {
                                    topic = Some(x.0);
                                    None
                                } else {
                                    Some(x)
                                }
                            })
                            .collect();
                        if let Some(topic) = topic {
                            conn.state
                                .lock()
                                .await
                                .retry_commands
                                .retain(|x| *x != nonce);
                            debug!("Retrying topic {topic:#?}");
                            self.listen_command(topic).await;
                        }
                    }
                }

                self.connections = self
                    .connections
                    .drain(..)
                    .filter(|x| !x.topics.is_empty())
                    .collect();
                self.connections.push(conn);
            }
        }
    }

    async fn listen_command(&mut self, topic: Topics) {
        if self
            .connections
            .iter()
            .filter(|x| x.topics.len() < 50)
            .count()
            == 0
        {
            self.retry_add_connection().await;
        }

        let mut conn = None;
        self.connections = self
            .connections
            .drain(..)
            .filter_map(|x| {
                if x.topics.len() < 50 && conn.is_none() {
                    conn = Some(x);
                    None
                } else {
                    Some(x)
                }
            })
            .collect();

        let mut conn = conn.unwrap();
        loop {
            match conn.listen_topic(&topic).await {
                Ok(nonce) => {
                    conn.topics.push((topic, nonce));
                    self.connections.push(conn);
                    break;
                }
                Err(err) => {
                    warn!("Failed to listen to topic {:#?}", err);
                    conn = self.reconnect(conn).await;
                }
            }
        }
    }

    async fn retry_add_connection(&mut self) {
        debug!("Adding connection");
        loop {
            match self.add_connection().await {
                Ok(conn) => {
                    self.connections.push(conn);
                    break;
                }
                Err(err) => {
                    warn!("Failed to add connection {err:#?}");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn add_connection(&mut self) -> Result<WsConn> {
        let (mut writer, reader) = self
            .connect_twitch_ws()
            .await
            .context("Connecting to twitch pubsub")?;

        let state = Arc::new(Mutex::new(WsConnState {
            last_update: Instant::now(),
            stream_state: WsStreamState::Open,
            retry_commands: Vec::new(),
        }));

        writer
            .send(Message::Text(json!({"type": "PING"}).to_string()))
            .await?;

        let conn = WsConn {
            reader: spawn(ws_reader(state.clone(), self.tx.clone(), reader)),
            writer,
            topics: Vec::new(),
            state,
            access_token: self.access_token.clone(),
        };

        Ok(conn)
    }

    async fn reconnect(&mut self, mut conn: WsConn) -> WsConn {
        async fn reconnect_logic(
            pool: &mut WsPool,
            mut conn: WsConn,
        ) -> Result<WsConn, (WsConn, Report)> {
            debug!("Reconnecting ws with {} topics", conn.topics.len());
            if !conn.reader.is_finished() {
                _ = conn.writer.close().await;
                conn.reader.abort();
            }

            if let Err(err) = conn.writer.close().await {
                warn!("Error closing connection {:#?}", err);
            }

            let mut added_connection = 'outer: loop {
                match pool.add_connection().await {
                    Ok(c) => break 'outer c,
                    Err(err) => {
                        warn!("Failed to add connection {err:#?}");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            };

            added_connection.topics.clone_from(&conn.topics);
            for (t, _) in conn.topics {
                let res = added_connection
                    .listen_topic(&t)
                    .await
                    .context("Listening to topic");
                match res {
                    Ok(nonce) => {
                        if let Some((_, n)) = added_connection
                            .topics
                            .iter_mut()
                            .find(|(x, _)| (*x).eq(&t))
                        {
                            *n = nonce;
                        }
                    }
                    Err(err) => return Err((added_connection, err)),
                }
            }
            info!("Reconnected with {} topics", added_connection.topics.len());
            Ok(added_connection)
        }

        loop {
            match reconnect_logic(self, conn).await {
                Ok(c) => return c,
                Err((failed_conn, err)) => {
                    conn = failed_conn;
                    warn!("Failed to reconnect {err:#?}");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn connect_twitch_ws(
        &self,
    ) -> Result<(SplitSink<WsStream, Message>, SplitStream<WsStream>)> {
        let (socket, _) = connect_async(
            #[cfg(feature = "testing")]
            &format!("{}/pubsub", self.base_url),
            #[cfg(not(feature = "testing"))]
            "wss://pubsub-edge.twitch.tv/v1",
        )
        .await?;

        Ok(socket.split())
    }
}

pub async fn add_streamer(ws_tx: &Sender<Request>, channel_id: u32) -> Result<()> {
    ws_tx
        .send_async(Request::Listen(Topics::VideoPlaybackById(
            VideoPlaybackById { channel_id },
        )))
        .await
        .context("Add streamer to pubsub")?;
    Ok(())
}

pub async fn remove_streamer(ws_tx: &Sender<Request>, channel_id: u32) -> Result<()> {
    ws_tx
        .send_async(Request::UnListen(Topics::VideoPlaybackById(
            VideoPlaybackById { channel_id },
        )))
        .await
        .context("Remove streamer from pubsub")?;
    Ok(())
}

impl WsConn {
    /// Returns the nonce
    async fn listen_topic(&mut self, topic: &Topics) -> Result<String> {
        let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 30);
        let msg = listen_command(&[topic.clone()], self.access_token.as_str(), nonce.as_str())
            .context("Generate listen command")?;
        trace!("{msg}");
        self.writer
            .send(Message::Text(msg))
            .await
            .context("Send WS message")?;
        Ok(nonce)
    }

    /// Returns the nonce
    async fn unlisten_topic(&mut self, topic: &Topics) -> Result<()> {
        let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 30);
        let msg = unlisten_command(&[topic.clone()], nonce.as_str())
            .context("Generate listen command")?;
        trace!("{msg}");
        self.writer
            .send(Message::Text(msg))
            .await
            .context("Send WS message")?;
        Ok(())
    }
}

async fn ws_reader(
    state: Arc<Mutex<WsConnState>>,
    tx: Sender<TopicData>,
    mut stream: SplitStream<WsStream>,
) -> Result<()> {
    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Text(m) = msg {
            trace!("Got message {m}");
            match Response::parse(&m) {
                Ok(r) => match r {
                    Response::Response(data) => {
                        if let Some(error) = data.error {
                            if !error.is_empty() {
                                warn!(
                                    "Command error {} {error:#?}",
                                    data.nonce.clone().unwrap_or_default()
                                );
                                state
                                    .lock()
                                    .await
                                    .retry_commands
                                    .push(data.nonce.unwrap_or_default());
                            }
                        }
                    }
                    Response::Message { data } => {
                        if let TopicData::VideoPlaybackById { topic: _, reply } = &data {
                            match reply.as_ref() {
                                VideoPlaybackReply::StreamUp {
                                    server_time: _,
                                    play_delay: _,
                                } => {}
                                VideoPlaybackReply::StreamDown { server_time: _ } => {}
                                _ => continue,
                            }
                        }
                        tx.send_async(data)
                            .await
                            .context("Could not send topic data")?;
                    }
                    Response::Pong => state.lock().await.last_update = Instant::now(),
                    Response::Reconnect => {
                        state.lock().await.stream_state = WsStreamState::Reconnect;
                        warn!("Twitch requested reconnect");
                        break;
                    }
                    _ => warn!("Unknown response {:#?}", r),
                },
                Err(err) => warn!("Failed to parse ws message {:#?} \nmessage {m}", err),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use super::*;
    use crate::{
        testing::{container, TestContainer},
        twitch::traverse_json,
    };

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test(flavor = "multi_thread")]
    async fn listen(#[future] container: TestContainer) -> Result<()> {
        let container = container.await;
        let (pool, tx, (_, rx)) =
            WsPool::start("test", format!("ws://localhost:{}", container.port)).await;

        let topic = VideoPlaybackById { channel_id: 1 };
        _ = tx
            .send_async(Request::Listen(Topics::VideoPlaybackById(topic.clone())))
            .await;

        _ = tx
            .send_async(Request::UnListen(Topics::VideoPlaybackById(topic.clone())))
            .await;

        let res = rx.stream().take(2).collect::<Vec<_>>().await;

        assert_eq!(res.len(), 2);
        assert_eq!(
            res[1],
            TopicData::VideoPlaybackById {
                topic: topic.clone(),
                reply: Box::new(VideoPlaybackReply::StreamUp {
                    server_time: 0.0,
                    play_delay: 0
                })
            }
        );
        assert_eq!(
            res[0],
            TopicData::VideoPlaybackById {
                topic: topic.clone(),
                reply: Box::new(VideoPlaybackReply::StreamDown { server_time: 0.0 })
            }
        );

        pool.abort();
        Ok(())
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test(flavor = "multi_thread")]
    async fn reconnect(#[future] container: TestContainer) -> Result<()> {
        let container = container.await;
        let pubsub_uri = format!("http://localhost:{}/pubsub", container.port);

        let client = reqwest::Client::new();
        client
            .post(&format!("{pubsub_uri}/test_mode"))
            .json(&json!("Reconnect"))
            .send()
            .await?;

        let (pool, tx, (_, _)) =
            WsPool::start("test", format!("ws://localhost:{}", container.port)).await;

        let topic = VideoPlaybackById { channel_id: 1 };
        _ = tx
            .send_async(Request::Listen(Topics::VideoPlaybackById(topic.clone())))
            .await;

        loop {
            let mut mock: serde_json::Value = client
                .get(&format!("{pubsub_uri}/test_stats"))
                .send()
                .await?
                .json()
                .await?;
            let connect_count = traverse_json(&mut mock, ".Reconnect.count");
            if connect_count.is_none() {
                sleep(Duration::from_millis(1)).await;
                continue;
            }

            let connect_count = connect_count.unwrap();
            assert!(connect_count.is_i64());

            if connect_count.as_i64().unwrap() == 2 {
                break;
            } else {
                sleep(Duration::from_millis(1)).await;
            }
        }

        pool.abort();
        Ok(())
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test(flavor = "multi_thread")]
    async fn retry_command(#[future] container: TestContainer) -> Result<()> {
        let container = container.await;
        let pubsub_uri = format!("http://localhost:{}/pubsub", container.port);

        let client = reqwest::Client::new();
        client
            .post(&format!("{pubsub_uri}/test_mode"))
            .json(&json!("RetryCommand"))
            .send()
            .await?;

        let (pool, tx, (_, _)) =
            WsPool::start("test", format!("ws://localhost:{}", container.port)).await;

        let topic = VideoPlaybackById { channel_id: 1 };
        _ = tx
            .send_async(Request::Listen(Topics::VideoPlaybackById(topic.clone())))
            .await;

        loop {
            let mut mock: serde_json::Value = client
                .get(&format!("{pubsub_uri}/test_stats"))
                .send()
                .await?
                .json()
                .await?;

            let connect_count = traverse_json(&mut mock, ".RetryCommand.count");
            if connect_count.unwrap().as_i64().unwrap() == 2 {
                break;
            } else {
                sleep(Duration::from_millis(1)).await;
            }
        }

        pool.abort();
        Ok(())
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test(flavor = "multi_thread")]
    async fn scale_connections(#[future] container: TestContainer) -> Result<()> {
        let container = container.await;
        let pubsub_uri = format!("http://localhost:{}/pubsub", container.port);

        let client = reqwest::Client::new();
        client
            .post(&format!("{pubsub_uri}/test_mode"))
            .json(&json!("ScaleConnections"))
            .send()
            .await?;

        let (pool, tx, (_, rx)) =
            WsPool::start("test", format!("ws://localhost:{}", container.port)).await;

        for i in 0..50 {
            let topic = VideoPlaybackById { channel_id: i };
            _ = tx
                .send_async(Request::Listen(Topics::VideoPlaybackById(topic)))
                .await;
        }

        loop {
            let mut mock: serde_json::Value = client
                .get(&format!("{pubsub_uri}/test_stats"))
                .send()
                .await?
                .json()
                .await?;

            let topics = traverse_json(&mut mock, ".ScaleConnections.topics");
            if topics.unwrap().as_i64().unwrap() == 50 {
                let sockets = traverse_json(&mut mock, ".ScaleConnections.sockets");
                if sockets.unwrap().as_i64().unwrap() == 1 {
                    break;
                }
            } else {
                sleep(Duration::from_millis(1)).await;
            }
        }

        let topic = VideoPlaybackById { channel_id: 51 };
        _ = tx
            .send_async(Request::Listen(Topics::VideoPlaybackById(topic)))
            .await;

        loop {
            let mut mock: serde_json::Value = client
                .get(&format!("{pubsub_uri}/test_stats"))
                .send()
                .await?
                .json()
                .await?;

            let topics = traverse_json(&mut mock, ".ScaleConnections.topics");
            if topics.unwrap().as_i64().unwrap() == 51 {
                let sockets = traverse_json(&mut mock, ".ScaleConnections.sockets");
                if sockets.unwrap().as_i64().unwrap() == 2 {
                    break;
                }
            } else {
                sleep(Duration::from_millis(1)).await;
            }
        }

        let topic = VideoPlaybackById { channel_id: 51 };
        _ = tx
            .send_async(Request::UnListen(Topics::VideoPlaybackById(topic.clone())))
            .await;

        let res = rx.recv_async().await?;
        assert_eq!(
            res,
            TopicData::VideoPlaybackById {
                topic,
                reply: Box::new(VideoPlaybackReply::StreamDown { server_time: 0.0 })
            }
        );

        pool.abort();
        Ok(())
    }
}
