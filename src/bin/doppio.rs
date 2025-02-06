use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use const_format::formatcp;
use doppio::{
    get_socket_path,
    protocol::{ErrorKind, Request, Response},
};

#[derive(Parser)]
#[clap(name = "doppio", version)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Inhibit { id: String },
    Release { id: String },
}

const IS_RUNNING_MSG: &'static str = formatcp!("Is {}-daemon running?", env!("CARGO_PKG_NAME"));
const VERSION_MSG: &'static str = formatcp!(
    "Are {} and {}-daemon of the same version?",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME")
);
const RESTART_MSG: &'static str = formatcp!("Try restarting {}-daemon", env!("CARGO_PKG_NAME"));

fn main() -> Result<()> {
    let cli = Cli::parse();

    let socket_path = get_socket_path()?;
    let stream = UnixStream::connect(&socket_path).with_context(|| {
        format!(
            "Faild to connect to doppio socket at {}. {}",
            socket_path.to_string_lossy(),
            IS_RUNNING_MSG,
        )
    })?;

    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();

    match cli.command {
        Commands::Inhibit { id } => inhibit(stream, id),
        Commands::Release { id } => release(stream, id),
    }
}

fn inhibit(mut stream: UnixStream, id: String) -> Result<()> {
    match communicate(&mut stream, Request::Inhibit { id })? {
        Response::Ok => Ok(()),
        Response::Status { .. } | Response::ActiveInhibitors { .. } => Err(anyhow!(
            "Unexpected response from doppio-daemon. {}",
            VERSION_MSG
        )),
        Response::Error { kind } => Err(parse_error(kind, "inhibit")),
    }
}

fn release(mut stream: UnixStream, id: String) -> Result<()> {
    match communicate(&mut stream, Request::Release { id })? {
        Response::Ok => Ok(()),
        Response::Status { .. } | Response::ActiveInhibitors { .. } => Err(anyhow!(
            "Unexpected response from doppio-daemon. {}",
            VERSION_MSG
        )),
        Response::Error { kind } => Err(parse_error(kind, "inhibit")),
    }
}

fn parse_error(kind: ErrorKind, operation_name: &str) -> anyhow::Error {
    match kind {
        ErrorKind::SocketError => {
            anyhow!("doppio-daemon failed to respond. {}", VERSION_MSG)
        }
        ErrorKind::InvalidRequest => {
            anyhow!(
                "doppio-daemon failed did not understand the request. {}",
                VERSION_MSG
            )
        }
        ErrorKind::DaemonError => {
            anyhow!("doppio-daemon experienced internal error. {}", RESTART_MSG)
        }
        ErrorKind::OperationFailed => {
            anyhow!(
                "doppio-daemon failed to {}. {}",
                operation_name,
                RESTART_MSG
            )
        }
    }
}

fn communicate(stream: &mut UnixStream, request: Request) -> Result<Response> {
    communicate_write(stream, request)
        .with_context(|| format!("Failed to write to doppio sockedt. {}", IS_RUNNING_MSG))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .with_context(|| format!("Failed to read from doppio socket. {}", IS_RUNNING_MSG))?;

    Response::des(&response)
        .ok_or_else(|| anyhow!("Failed to parse doppio-daemon response. {}", VERSION_MSG))
}

fn communicate_write(stream: &mut UnixStream, request: Request) -> Result<()> {
    stream.write(&request.ser().into_bytes())?;
    stream.flush()?;
    stream.shutdown(std::net::Shutdown::Write)?; // Write EOF to stream

    Ok(())
}
