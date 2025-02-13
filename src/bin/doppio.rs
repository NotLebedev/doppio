use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use doppio::{
    protocol::{ErrorKind, Request, Response, Status},
    Locations,
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
    Status { id: Option<String> },
}

const IS_RUNNING_MSG: &'static str = "Is doppio-daemon running?";
const VERSION_MSG: &'static str = "Are doppio and doppio-daemon of the same version?";
const RESTART_MSG: &'static str = "Try restarting doppio-daemon";

fn main() -> Result<()> {
    let cli = Cli::parse();

    let locations = Locations::new()?;

    let stream = setup_socket(&locations.socket_path).with_context(|| {
        format!(
            "Faild to connect to doppio socket at {}. {}",
            locations.socket_path.to_string_lossy(),
            IS_RUNNING_MSG,
        )
    })?;

    match cli.command {
        Commands::Inhibit { id } => inhibit(stream, id),
        Commands::Release { id } => release(stream, id),
        Commands::Status { id: Some(id) } => status(stream, id),
        Commands::Status { id: None } => all_status(stream),
    }
}

fn all_status(mut stream: UnixStream) -> Result<()> {
    match communicate(&mut stream, Request::ActiveInhibitors)? {
        Response::Ok | Response::Status { .. } => Err(unexpected_response()),
        Response::ActiveInhibitors { active_inhibitors } => {
            for inhibitor in active_inhibitors {
                println!("{inhibitor}");
            }
            Ok(())
        }
        Response::Error { kind } => Err(parse_error(kind, "get status")),
    }
}

fn status(mut stream: UnixStream, id: String) -> Result<()> {
    match communicate(&mut stream, Request::Status { id })? {
        Response::Status { status } => {
            match status {
                Status::Inhibits => println!("inhibits"),
                Status::Free => println!("free"),
            }
            Ok(())
        }
        Response::Ok | Response::ActiveInhibitors { .. } => Err(unexpected_response()),
        Response::Error { kind } => Err(parse_error(kind, "get status")),
    }
}

fn inhibit(mut stream: UnixStream, id: String) -> Result<()> {
    match communicate(&mut stream, Request::Inhibit { id })? {
        Response::Ok => Ok(()),
        Response::Status { .. } | Response::ActiveInhibitors { .. } => Err(unexpected_response()),
        Response::Error { kind } => Err(parse_error(kind, "inhibit")),
    }
}

fn release(mut stream: UnixStream, id: String) -> Result<()> {
    match communicate(&mut stream, Request::Release { id })? {
        Response::Ok => Ok(()),
        Response::Status { .. } | Response::ActiveInhibitors { .. } => Err(unexpected_response()),
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
        ErrorKind::OperationFailed => {
            anyhow!(
                "doppio-daemon failed to {}. {}",
                operation_name,
                RESTART_MSG
            )
        }
    }
}

fn unexpected_response() -> anyhow::Error {
    anyhow!("Unexpected response from doppio-daemon. {}", VERSION_MSG)
}

fn communicate(stream: &mut UnixStream, request: Request) -> Result<Response> {
    communicate_write(stream, request)
        .with_context(|| format!("Failed to write to doppio socket. {}", IS_RUNNING_MSG))?;

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

fn setup_socket(socket_path: &Path) -> Result<UnixStream> {
    let stream = UnixStream::connect(socket_path)?;

    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;

    Ok(stream)
}
