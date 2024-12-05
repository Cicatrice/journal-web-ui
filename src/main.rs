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
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tokio_stream::{wrappers::LinesStream, StreamExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Fallible {
    let bind_addr = var("BIND_ADDR")?.parse::<SocketAddr>()?;

    let service = make_service_fn(move |_| async move {
        Ok::<_, Infallible>(service_fn(move |req| async move {
            match run_logs(req).await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    let msg = format!("Failed to fetch logs: {}", err);

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

async fn run_logs(
    req: Request<Body>,
) -> Fallible<Response<Body>> {
    let query = parse(req.uri().query().unwrap_or_default().as_bytes());

    let mut provider = String::from("journalctl");

    for (key, value) in query {
        match &*key {
            "provider" => {
                provider = format!("{value}");
                continue
            },
            _ => continue,
        };
    }


    let mut cmd;
    let mut lines = false;

    match &*provider {
        "journalctl" => {

            cmd = Command::new("journalctl");

            cmd.args(["--merge", "--reverse"]);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

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
                    "container" => cmd.arg(format!("CONTAINER_NAME={value}")),
                    _ => continue,
                };
            }

            if !lines {
                cmd.arg("--lines");
            }
        },
        "docker" => {
            cmd = Command::new("docker");
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd.args(["logs"]);

            let mut container_name = String::from("MISSING_CONTAINER_NAME");

            for (key, value) in query {
                match &*key {
                    "lines" => cmd.arg(format!("--tail={value}")),
                    "container" => {
                        container_name = value.to_string() ;
                        continue
                    },
                    _ => continue,
                };
            }

            cmd.arg(container_name);

        },

        _ => return Err(format!("Unsupported provider {provider:?}").into())

    }
 

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout = LinesStream::new(BufReader::new(stdout).lines());
    let stderr = LinesStream::new(BufReader::new(stderr).lines());
    let merged = StreamExt::merge(stdout, stderr);

    let lines =
        merged.filter_map(move |line| match line {
            Ok(mut line) => {
                line.push('\n');
                Some(Ok(line))
            }
            err => Some(err),
        });

    let resp = Response::builder()
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::wrap_stream(lines))
        .unwrap();

    Ok(resp)
}

type Fallible<T = ()> = Result<T, Error>;

type Error = Box<dyn StdError + Send + Sync>;
