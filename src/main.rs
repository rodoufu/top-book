use crate::{
    orderbook::Orderbook,
    deribit::DeribitError,
    okx::OKXError,
};
use futures_util::{
    future,
    pin_mut,
    TryFutureExt,
};
use opentelemetry::{
    global,
    sdk::trace as sdktrace,
    trace::{
        FutureExt,
        TraceContextExt,
        Tracer,
        TraceError,
    },
    Context,
};
use opentelemetry_jaeger;
use opentelemetry::{sdk::propagation::TraceContextPropagator};
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::mpsc,
    task,
};
use std::{
    error::Error,
    io,
    net::SocketAddr,
};
use std::fmt::{Display, Formatter};
use std::future::Future;
use tokio::sync::mpsc::UnboundedReceiver;
use crate::orderbook::Operation;

mod orderbook;
mod deribit;
mod okx;

fn init_tracer() -> Result<sdktrace::Tracer, TraceError> {
    opentelemetry_jaeger::new_pipeline()
        .with_service_name("top-book")
        .install_batch(opentelemetry::runtime::Tokio)
}

#[derive(Debug)]
pub enum WebsocketError {
    Orderbook,
    Deribit(DeribitError),
    OKX(OKXError),
}

impl Display for WebsocketError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{:?}", self).as_str())
    }
}

impl Error for WebsocketError {}

async fn process_orderbook(receiver: &mut UnboundedReceiver<Operation>) -> Result<(), WebsocketError> {
    let tracer = global::tracer("orderbook_processor");
    let span = tracer.start("process_orderbook");
    let cx = Context::current_with_span(span);

    let mut orderbook = Orderbook::new(200);
    while let Some(operation) = receiver.recv().with_context(cx.clone()).await {
        let span = tracer.start("process_operation");
        let cx = Context::current_with_span(span);

        orderbook.process(operation);

        tokio::io::stdout().write_all(
            format!(
                "Orderbook size {:?}, content: {:?}\n",
                orderbook.len(),
                orderbook,
            ).as_bytes(),
        ).with_context(cx).await.map_err(|_| WebsocketError::Orderbook)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let tracer = init_tracer()?;
    let span = tracer.start("root");
    let cx = Context::current_with_span(span);

    Registry::default()
        .with(tracing_subscriber::EnvFilter::new("INFO"))
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    let (sender, mut receiver) = mpsc::unbounded_channel();

    let process_okx_ws = async {
        okx::consume_orderbook(sender.clone()).map_err(WebsocketError::OKX).
            with_context(cx.clone()).await?;
        let resp: Result<(), WebsocketError> = Ok(());
        resp
    };
    let process_deribit_ws = async {
        deribit::consume_orderbook(sender.clone()).
            map_err(WebsocketError::Deribit).with_context(cx.clone()).await?;
        let resp: Result<(), WebsocketError> = Ok(());
        resp
    };
    let process_ob = process_orderbook(&mut receiver).with_context(cx.clone());

    let okx_deribit = futures_util::future::join(process_okx_ws, process_deribit_ws).
        with_context(cx.clone());

    let ((okx, deribit), orderbook) = futures_util::future::join(okx_deribit, process_ob).
        with_context(cx.clone()).await;

    okx?;
    deribit?;
    orderbook?;

    global::shutdown_tracer_provider();
    Ok(())
}
