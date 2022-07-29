use crate::{
    okx::WebsocketResponse,
    orderbook::Orderbook,
};
use futures_util::{
    future,
    pin_mut,
    SinkExt,
    StreamExt,
};
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc,
    task,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::Message,
};
use url::Url;

mod orderbook;
mod okx;

#[tokio::main]
async fn main() {
    // TODO move websocket logic
    let connect_addr = "wss://ws.okx.com:8443/ws/v5/public".to_string();
    let url = Url::parse(&connect_addr).unwrap();

    let (ws_stream, _) = connect_async(url).await.
        expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (mut write, read) = ws_stream.split();
    write.send(Message::Text(r#"{"op":"subscribe","args":[{"channel": "books","instId":"BTC-USDT"}]}"#.to_string())).await.expect("subscribing");

    let (sender, mut receiver) = mpsc::unbounded_channel();

    let okx_sender = sender.clone();
    let ws_to_stdout = {
        read.for_each(|message| async {
            let okx_parse: serde_json::Result<WebsocketResponse> = serde_json::from_slice(&message.unwrap().into_data());
            match okx_parse {
                Ok(resp) => {
                    match resp {
                        WebsocketResponse::Action(action) => {
                            okx_sender.send(action.into()).ok().unwrap();
                        }
                        WebsocketResponse::Response { event } => {
                            tokio::io::stdout().write_all(
                                format!("Got event {:?}\n", event).as_bytes(),
                            ).await.unwrap();
                        }
                    }
                }
                Err(err) => {
                    tokio::io::stdout().write_all(
                        format!("Got parse error {:?}\n", err).as_bytes(),
                    ).await.unwrap();
                }
            }
        })
    };

    // TODO add logger
    let process_orderbook = task::spawn(async move {
        let mut orderbook = Orderbook::new(10);
        while let Some(operation) = receiver.recv().await {
            orderbook.process(operation);

            tokio::io::stdout().write_all(
                format!(
                    "Orderbook size {:?}, orderbook: {:?}\n",
                    orderbook.len(),
                    orderbook,
                ).as_bytes(),
            ).await.unwrap();
        }
    });

    pin_mut!(process_orderbook, ws_to_stdout);
    future::select(process_orderbook, ws_to_stdout).await;
}
