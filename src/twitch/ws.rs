use color_eyre::{eyre::Context, Result};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde_json::json;
use tokio::{
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
    time::interval,
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn connect_twitch_ws(
    url: &str,
    access_token: &str,
) -> Result<(SplitSink<WsStream, Message>, SplitStream<WsStream>)> {
    let request = http::Request::builder()
        .uri(url)
        .header("Authorization", format!("OAuth {}", access_token))
        .header("Host", "localhost")
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("sec-websocket-version", 13)
        .body(())
        .context(format!("Couldn't build request for {}", url))?;
    let (socket, _) = connect_async(request).await?;

    Ok(socket.split())
}

pub async fn writer(
    mut rx: Receiver<String>,
    mut write: SplitSink<WsStream, Message>,
) -> Result<()> {
    while let Some(msg) = rx.recv().await {
        write.send(Message::Text(msg)).await?;
    }
    Ok(())
}

pub async fn ping_loop(tx: Sender<String>) -> Result<()> {
    let mut interval = interval(std::time::Duration::from_secs(3 * 60));
    let ping = json!({"type": "PING"}).to_string();
    loop {
        interval.tick().await;

        if let Err(_) = tx.send(ping.clone()).await {
            break;
        }
    }
    Ok(())
}
