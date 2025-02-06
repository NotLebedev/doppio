use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use doppio::{Request, Response, SOCKET_PATH};

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

const IS_RUNNING_MSG: &'static str = "Is doppio-damon running?";
const VERSION_MSG: &'static str = "Are doppio and doppio-daemon of the same version?";
const RESTART_MSG: &'static str = "Try restarting doppio-daemon";

fn main() -> Result<()> {
    let cli = Cli::parse();

    let stream = UnixStream::connect(SOCKET_PATH).with_context(|| {
        format!(
            "Faild to connect to doppio socket at {}. {}",
            SOCKET_PATH, IS_RUNNING_MSG,
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

fn parse_error(kind: doppio::Error, operation_name: &str) -> anyhow::Error {
    match kind {
        doppio::Error::SocketError => {
            anyhow!("doppio-daemon failed to respond. {}", VERSION_MSG)
        }
        doppio::Error::InvalidRequest => {
            anyhow!(
                "doppio-daemon failed did not understand the request. {}",
                VERSION_MSG
            )
        }
        doppio::Error::DaemonError => {
            anyhow!("doppio-daemon experienced internal error. {}", RESTART_MSG)
        }
        doppio::Error::OperationFailed => {
            anyhow!(
                "doppio-daemon failed to {}. {}",
                operation_name,
                RESTART_MSG
            )
        }
    }
}

fn communicate(stream: &mut UnixStream, request: Request) -> Result<Response> {
    communicate_write(stream, request).with_context(|| {
        format!(
            "Failed to write to doppio sockedt at {}. {}",
            SOCKET_PATH, IS_RUNNING_MSG
        )
    })?;

    let mut response = String::new();
    stream.read_to_string(&mut response).with_context(|| {
        format!(
            "Failed to read from doppio socket at {}. {}",
            SOCKET_PATH, IS_RUNNING_MSG
        )
    })?;

    Response::des(&response)
        .ok_or_else(|| anyhow!("Failed to parse doppio-daemon response. {}", VERSION_MSG))
}

fn communicate_write(stream: &mut UnixStream, request: Request) -> Result<()> {
    stream.write(&request.ser().into_bytes())?;
    stream.flush()?;
    stream.shutdown(std::net::Shutdown::Write)?; // Write EOF to stream

    Ok(())
}
