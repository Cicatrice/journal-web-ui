use std::convert::Infallible;
use std::env::var;
use std::error::Error as StdError;
use std::net::SocketAddr;
use std::process::Stdio;

use form_urlencoded::parse;
use hyper::{
    body::Body,
    header::CONTENT_TYPE,
    service::{make_service_fn, service_fn},
    Request, Response, Server, StatusCode,
};
use tokio::process::Command;
use tokio_util::io::ReaderStream;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Fallible {
    let bind_addr = var("BIND_ADDR")?.parse::<SocketAddr>()?;

    let service = make_service_fn(move |_| async move {
        Ok::<_, Infallible>(service_fn(move |req| async move {
            match run_journalctl(req).await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    let msg = format!("Failed to run journalctl: {}", err);

                    eprintln!("{}", msg);

                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(CONTENT_TYPE, "text/plain")
                        .body(msg.into())
                }
            }
        }))
    });

    Server::bind(&bind_addr).serve(service).await?;

    Ok(())
}

async fn run_journalctl(req: Request<Body>) -> Fallible<Response<Body>> {
    let mut cmd = Command::new("journalctl");

    cmd.args(["--merge", "--reverse"]);
    cmd.stdout(Stdio::piped());

    let query = parse(req.uri().query().unwrap_or_default().as_bytes());
    let mut lines = false;

    for (key, value) in query {
        match &*key {
            "lines" => {
                lines = true;
                cmd.arg(format!("--lines={value}"))
            }
            "unit" => cmd.arg(format!("--unit={value}")),
            "since" => cmd.arg(format!("--since={value}")),
            "until" => cmd.arg(format!("--until={value}")),
            "grep" => cmd.arg(format!("--grep={value}")),
            "hostname" => cmd.arg(format!("_HOSTNAME={value}")),
            "matches" => cmd.arg(&*value),
            _ => continue,
        };
    }

    if !lines {
        cmd.arg("--lines");
    }

    let mut child = cmd.spawn()?;
    let stdout = ReaderStream::new(child.stdout.take().unwrap());

    let resp = Response::builder()
        .header(CONTENT_TYPE, "text/plain")
        .body(Body::wrap_stream(stdout))
        .unwrap();

    Ok(resp)
}

type Fallible<T = ()> = Result<T, Error>;

type Error = Box<dyn StdError + Send + Sync>;
