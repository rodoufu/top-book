use crate::{
    orderbook::{
        Orderbook,
        Operation,
    },
    deribit::DeribitError,
    okx::OKXError,
};
use futures_util::{
    future,
    pin_mut,
    TryFutureExt,
};
use opentelemetry_jaeger;
use opentelemetry::{
    global::{
        self,
        BoxedTracer,
    },
    sdk::trace as sdktrace,
    trace::{
        FutureExt,
        mark_span_as_active,
        Span,
        TraceContextExt,
        Tracer,
        TraceError,
    },
    Context,
    Key,
    sdk::{
        propagation::TraceContextPropagator,
        Resource,
    },
};
use tracing_subscriber::{
    prelude::*,
    Registry,
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::mpsc::{
        self,
        UnboundedReceiver,
        UnboundedSender,
    },
    task,
};
use std::{
    error::Error,
    fmt::{
        Display,
        Formatter,
    },
    future::Future,
    io,
    net::SocketAddr,
    thread,
};

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

async fn process_orderbook(
    receiver: &mut UnboundedReceiver<Operation>,
) -> Result<(), WebsocketError> {
    let tracer = global::tracer("orderbook_processor");
    let span = tracer.start("process_orderbook");
    let ctx = Context::current_with_span(span);

    let mut orderbook = Orderbook::new(200);
    while let Some(operation) = receiver.recv()
        .with_context(ctx.clone())
        .await {
        let span = tracer.start("process_orderbook_operation");
        let ctx = Context::current_with_span(span);
        let (asks_len, bids_len) = operation.len();
        ctx.span().add_event("processing orderbook message", vec![
            Key::new("operation_asks_len").i64(asks_len as i64),
            Key::new("operation_bids_len").i64(bids_len as i64),
        ]);
        orderbook.process(operation);

        let (asks_len, bids_len) = orderbook.len();
        ctx.span().add_event(
            "orderbook message processed", vec![
                Key::new("asks_len").i64(asks_len as i64),
                Key::new("bids_len").i64(bids_len as i64),
            ],
        );
        tokio::io::stdout().write_all(
            format!(
                "Orderbook size {:?}, content: {:?}\n",
                orderbook.len(),
                orderbook,
            ).as_bytes(),
        )
            .with_context(ctx.clone())
            .await.map_err(|_| WebsocketError::Orderbook)?;
    }

    Ok(())
}

async fn process_okx_ws(
    ctx: Context, sender: UnboundedSender<Operation>,
) -> Result<(), WebsocketError> {
    okx::consume_orderbook(sender).map_err(WebsocketError::OKX)
        .with_context(ctx)
        .await
}

async fn process_deribit_ws(
    ctx: Context, sender: UnboundedSender<Operation>,
) -> Result<(), WebsocketError> {
    deribit::consume_orderbook(sender).map_err(WebsocketError::Deribit)
        .with_context(ctx)
        .await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let tracer = init_tracer()?;

    let span = tracer.start("root");
    let ctx = Context::current_with_span(span);
    ctx.span().add_event("starting application", vec![]);

    let (sender, mut receiver) = mpsc::unbounded_channel();

    let process_ob = process_orderbook(&mut receiver).
        with_context(ctx.clone());

    let okx_deribit = futures_util::future::join(
        process_okx_ws(ctx.clone(), sender.clone()),
        process_deribit_ws(ctx.clone(), sender.clone()),
    )
        .with_context(ctx.clone());

    let ((okx, deribit), orderbook) = futures_util::future::join(
        okx_deribit,
        process_ob,
    )
        .with_context(ctx.clone()).await;

    okx?;
    deribit?;
    orderbook?;

    global::shutdown_tracer_provider();
    Ok(())
}
