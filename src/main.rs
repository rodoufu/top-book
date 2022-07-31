use crate::{
    orderbook::Orderbook,
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
mod deribit;
mod okx;

#[tokio::main]
async fn main() {
    let (sender, mut receiver) = mpsc::unbounded_channel();

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

    let process_okx_ws = okx::consume_orderbook(sender.clone());
    let process_deribit_ws = deribit::consume_orderbook(sender.clone());

    let process_okx_deribit = async {
        pin_mut!(process_okx_ws, process_deribit_ws);
        future::select(process_okx_ws, process_deribit_ws).await;
    };

    pin_mut!(process_orderbook, process_okx_deribit);
    future::select(process_orderbook, process_okx_deribit).await;
}
