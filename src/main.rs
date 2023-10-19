use std::convert::Infallible;
use std::env::{var, var_os};
use std::error::Error as StdError;
use std::ffi::OsStr;
use std::fs::read_to_string;
use std::net::SocketAddr;
use std::process::Stdio;

use form_urlencoded::parse;
use hyper::{
    body::Body,
    header::CONTENT_TYPE,
    service::{make_service_fn, service_fn},
    Request, Response, Server, StatusCode,
};
use regex::{RegexSet, RegexSetBuilder};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tokio_stream::{wrappers::LinesStream, StreamExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Fallible {
    let bind_addr = var("BIND_ADDR")?.parse::<SocketAddr>()?;
    let exp_msgs = var_os("EXP_MSGS").expect("Missing environment variable EXP_MSGS");

    let exp_msgs = &*Box::leak(Box::new(build_exp_msgs(&exp_msgs)?));

    let service = make_service_fn(move |_| async move {
        Ok::<_, Infallible>(service_fn(move |req| async move {
            match run_journalctl(exp_msgs, req).await {
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

fn build_exp_msgs(path: &OsStr) -> Fallible<RegexSet> {
    let buffer = read_to_string(path)?;

    let patterns = buffer.lines().filter(|line| !line.starts_with('#'));

    let regex = RegexSetBuilder::new(patterns).build()?;

    Ok(regex)
}

async fn run_journalctl(
    exp_msgs: &'static RegexSet,
    req: Request<Body>,
) -> Fallible<Response<Body>> {
    let mut cmd = Command::new("journalctl");

    cmd.args(["--merge", "--reverse"]);
    cmd.stdout(Stdio::piped());

    let query = parse(req.uri().query().unwrap_or_default().as_bytes());
    let mut lines = false;
    let mut unexpected = false;

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
            "unexpected" => {
                unexpected = true;
                continue;
            }
            _ => continue,
        };
    }

    if !lines {
        cmd.arg("--lines");
    }

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();

    let lines =
        LinesStream::new(BufReader::new(stdout).lines()).filter_map(move |line| match line {
            Ok(mut line) => {
                if unexpected && exp_msgs.is_match(&line) {
                    return None;
                }

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
