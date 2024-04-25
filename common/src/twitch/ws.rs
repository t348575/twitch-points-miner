use color_eyre::{eyre::Context, Result};
use flume::{Receiver, Sender};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde_json::json;
use tokio::{net::TcpStream, time::interval};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::{CLIENT_ID, FIREFOX_USER_AGENT};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn connect_twitch_ws(
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

pub async fn writer(rx: Receiver<String>, mut write: SplitSink<WsStream, Message>) -> Result<()> {
    while let Ok(msg) = rx.recv_async().await {
        write
            .send(Message::Text(msg))
            .await
            .context("Could not send ws message")?;
    }
    Ok(())
}

pub async fn ping_loop(tx: Sender<String>) -> Result<()> {
    let mut interval = interval(std::time::Duration::from_secs(60));
    let ping = json!({"type": "PING"}).to_string();
    loop {
        if tx.send_async(ping.clone()).await.is_err() {
            break;
        }
        interval.tick().await;
    }
    Ok(())
}
