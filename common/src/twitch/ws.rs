use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use color_eyre::eyre::{Context, Report, Result};
use flume::{Receiver, RecvTimeoutError, Sender};
use futures::{
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
use tracing::{debug, info, warn};
use twitch_api::pubsub::{listen_command, unlisten_command, Response, TopicData, Topics};

use super::{CLIENT_ID, FIREFOX_USER_AGENT};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct WsPool {
    connections: Vec<WsConn>,
    rx: Receiver<Request>,
    tx: Sender<TopicData>,
    access_token: String,
}

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    ) -> (JoinHandle<Result<()>>, Sender<Request>, Receiver<TopicData>) {
        let (req_tx, req_rx) = flume::unbounded();
        let (res_tx, res_rx) = flume::unbounded();

        let pool = spawn(WsPool::run(WsPool {
            connections: vec![],
            rx: req_rx,
            tx: res_tx,
            access_token: access_token.to_owned(),
        }));

        (pool, req_tx, res_rx)
    }

    async fn run(mut self) -> Result<()> {
        loop {
            if self.connections.is_empty() {
                self.retry_add_connection().await;
            }

            match self.rx.recv_timeout(Duration::from_millis(1)) {
                Ok(Request::Listen(topic)) => self.listen_command(topic).await,
                Ok(Request::UnListen(topic)) => {
                    let mut conn = None;
                    self.connections = self
                        .connections
                        .drain(..)
                        .filter_map(|x| {
                            if x.topics.iter().any(|x| x.0.eq(&topic)) && conn.is_none()
                            {
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
                }
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
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
                        warn!("Twitch did not respond to ping");
                        conn = self.reconnect(conn).await;
                    }
                }

                if !state.retry_commands.is_empty() {
                    for nonce in state.retry_commands {
                        let mut topic = None;
                        for conn in &mut self.connections {
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
                        }
                        if let Some(topic) = topic {
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
        Ok(())
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
        loop {
            match self.add_connection().await {
                Ok(conn) => self.connections.push(conn),
                Err(err) => {
                    warn!("Failed to add connection {err:#?}");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn add_connection(&mut self) -> Result<WsConn> {
        let (writer, reader) = connect_twitch_ws(&self.access_token)
            .await
            .context("Connecting to twitch pubsub")?;

        let state = Arc::new(Mutex::new(WsConnState {
            last_update: Instant::now(),
            stream_state: WsStreamState::Open,
            retry_commands: Vec::new(),
        }));
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
}

impl WsConn {
    /// Returns the nonce
    async fn listen_topic(&mut self, topic: &Topics) -> Result<String> {
        let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 30);
        let msg = listen_command(&[topic.clone()], self.access_token.as_str(), nonce.as_str())
            .context("Generate listen command")?;
        self.writer
            .send(Message::Text(msg))
            .await
            .context("Send WS message")?;
        Ok(nonce)
    }

    /// Returns the nonce
    async fn unlisten_topic(&mut self, topic: &Topics) -> Result<()> {
        let msg = unlisten_command(&[topic.clone()], self.access_token.as_str())
            .context("Generate listen command")?;
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
            match Response::parse(&m) {
                Ok(r) => match r {
                    Response::Response(data) => {
                        if data.error.is_some() {
                            warn!("Command error {:#?}", data.error);
                            state
                                .lock()
                                .await
                                .retry_commands
                                .push(data.nonce.unwrap_or_default());
                        }
                    }
                    Response::Message { data } => tx
                        .send_async(data)
                        .await
                        .context("Could not send topic data")?,
                    Response::Pong => state.lock().await.last_update = Instant::now(),
                    Response::Reconnect => {
                        state.lock().await.stream_state = WsStreamState::Reconnect;
                        warn!("Reconnect");
                        break;
                    }
                    _ => warn!("Unknown response {:#?}", r),
                },
                Err(err) => warn!("Failed to parse ws message {:#?}", err),
            }
        }
    }
    Ok(())
}

async fn connect_twitch_ws(
    access_token: &str,
) -> Result<(SplitSink<WsStream, Message>, SplitStream<WsStream>)> {
    let request = http::Request::builder()
        .uri("wss://pubsub-edge.twitch.tv/v1")
        .header("Authorization", format!("OAuth {}", access_token))
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", FIREFOX_USER_AGENT)
        .header("Host", "pubsub-edge.twitch.tv")
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("sec-websocket-version", 13)
        .body(())?;
    let (socket, _) = connect_async(request).await?;

    Ok(socket.split())
}
