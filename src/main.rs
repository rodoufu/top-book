use crate::{
    orderbook::Orderbook,
    okx::consume_orderbook,
};
use futures_util::{
    future,
    pin_mut,
};
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc,
    task,
};

mod orderbook;
mod okx;

#[tokio::main]
async fn main() {
    let (sender, mut receiver) = mpsc::unbounded_channel();

    // TODO add logger
    let process_orderbook = task::spawn(async move {
        let mut orderbook = Orderbook::new(10);
        while let Some(operation) = receiver.recv().await {
            orderbook.process(operation);

            tokio::io::stdout().write_all(
                format!(
                    "Orderbook size {:?}, content: {:?}\n",
                    orderbook.len(),
                    orderbook,
                ).as_bytes(),
            ).await.unwrap();
        }
    });

    let process_okx_ws = consume_orderbook(sender.clone());

    pin_mut!(process_orderbook, process_okx_ws);
    future::select(process_orderbook, process_okx_ws).await;
}
