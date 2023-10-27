use std::convert::Infallible;
use std::env::{var, var_os};
use std::error::Error as StdError;
use std::ffi::OsStr;
use std::fs::read_to_string;
use std::net::SocketAddr;
use std::process::{exit, Stdio};
use std::time::{Duration, Instant};

use form_urlencoded::parse;
use hyper::{
    body::Body,
    header::CONTENT_TYPE,
    service::{make_service_fn, service_fn},
    Request, Response, Server, StatusCode,
};
use lettre::{
    message::{header::ContentType, Mailbox},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use regex::{RegexSet, RegexSetBuilder};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    task::spawn,
};
use tokio_stream::{wrappers::LinesStream, StreamExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Fallible {
    let bind_addr = var("BIND_ADDR")?.parse::<SocketAddr>()?;

    let exp_msgs = var_os("EXP_MSGS").expect("Missing environment variable EXP_MSGS");

    let mail_srv = var("MAIL_SRV").expect("Missing environment variable MAIL_SRV");
    let mail_from = var("MAIL_FROM")
        .expect("Missing environment variable MAIL_FROM")
        .parse()?;
    let mail_to = var("MAIL_TO")
        .expect("Missing environment variable MAIL_TO")
        .parse()?;

    let exp_msgs = &*Box::leak(Box::new(build_exp_msgs(&exp_msgs)?));

    spawn(async move {
        if let Err(err) = collect_unexp_msgs(exp_msgs, mail_srv, mail_from, mail_to).await {
            eprintln!("Failed to collect unexpected messages: {}", err);
            exit(1);
        }
    });

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

async fn collect_unexp_msgs(
    exp_msgs: &'static RegexSet,
    mail_srv: String,
    mail_from: Mailbox,
    mail_to: Mailbox,
) -> Fallible<()> {
    let mut child = Command::new("journalctl")
        .args(["--merge", "--follow"])
        .stdout(Stdio::piped())
        .spawn()?;

    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

    let mut buf = Vec::new();
    let mut last_send = Instant::now();
    const INTERVAL: Duration = Duration::from_secs(15 * 60);

    while let Some(line) = lines.next_line().await? {
        if !exp_msgs.is_match(&line) {
            buf.push(line);
        }

        if !buf.is_empty() && last_send.elapsed() >= INTERVAL {
            eprintln!("Sending {} unexpected log messages via mail...", buf.len());

            let mail = Message::builder()
                .from(mail_from.clone())
                .to(mail_to.clone())
                .subject("Unexpected log messages")
                .header(ContentType::TEXT_PLAIN)
                .body(buf.join("\r\n"))?;

            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(mail_srv.clone())
                .build()
                .send(mail)
                .await?;

            buf.clear();
            last_send = Instant::now();
        }
    }

    let status = child.wait().await?;

    Err(format!("journalctl exited unexpected with status: {status:?}").into())
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
