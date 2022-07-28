use futures_util::{future, pin_mut, SinkExt, StreamExt};
use std::env;
use tokio::sync::mpsc;
use tokio::io::{
    AsyncReadExt,
    AsyncWriteExt,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::Message,
};
use url::Url;

mod orderbook;
mod okx
;

#[tokio::main]
async fn main() {
    let connect_addr = "wss://ws.okx.com:8443/ws/v5/public".to_string();
    let url = Url::parse(&connect_addr).unwrap();

    let (ws_stream, _) = connect_async(url).await.
        expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (mut write, read) = ws_stream.split();
    write.send(Message::Text(r#"{"op":"subscribe","args":[{"channel": "books","instId":"BTC-USDT"}]}"#.to_string())).await.expect("subscribing");

    let ws_to_stdout = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();
            tokio::io::stdout().write_all(&data).await.unwrap();
        })
    };

    // future::select(stdin_to_ws, ws_to_stdout).await;
    ws_to_stdout.await;
}
