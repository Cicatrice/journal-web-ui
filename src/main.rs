use std::env::var;
use std::error::Error as StdError;
use std::net::SocketAddr;
use std::process::Stdio;

use axum::{
    body::StreamBody,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router, Server,
};
use tokio::process::{ChildStdout, Command};
use tokio_util::io::ReaderStream;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Fallible {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let bind_addr = var("BIND_ADDR")?.parse::<SocketAddr>()?;

    let router = Router::new()
        .route("/", get(run_journalctl))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .into_make_service();

    tracing::info!("Listening on {}", bind_addr);
    Server::bind(&bind_addr).serve(router).await?;

    Ok(())
}

async fn run_journalctl() -> Result<StreamBody<ReaderStream<ChildStdout>>, ServerError> {
    let mut child = Command::new("journalctl").stdout(Stdio::piped()).spawn()?;
    let stdout = ReaderStream::new(child.stdout.take().unwrap());
    Ok(StreamBody::new(stdout))
}

struct ServerError(Error);

impl<E> From<E> for ServerError
where
    Error: From<E>,
{
    fn from(err: E) -> Self {
        Self(Error::from(err))
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

type Fallible<T = ()> = Result<T, Error>;

type Error = Box<dyn StdError + Send + Sync>;
