mod state;

use std::{
    fs::{self, remove_file, OpenOptions},
    io,
    sync::OnceLock,
};

use anyhow::{anyhow, Result};
use const_format::formatcp;
use doppio::{
    get_lock_path, get_socket_path, get_tmp_dir,
    protocol::{ErrorKind, Request, Response, Status},
};
use nix::fcntl::{Flock, FlockArg};
use state::State;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};
use zbus::Connection;

static STATE: OnceLock<State<'static>> = OnceLock::new();

const ANOTHER_MSG: &'static str = formatcp!(
    "Is another instance of {}-daemon running?",
    env!("CARGO_PKG_NAME")
);

#[tokio::main]
async fn main() -> Result<()> {
    let Ok(connection) = Connection::system().await else {
        return Err(anyhow!(
            "Could not connect to system bus! Is d-bus running?"
        ));
    };

    let _ = STATE.set(State::new(&connection).await.unwrap());

    let Ok(_lock) = acquire_run_lock() else {
        return Err(anyhow!("Could not acquire lock file! {}", ANOTHER_MSG));
    };

    let socket_path = get_socket_path()?;
    let _ = remove_file(&socket_path);
    let Ok(listener) = UnixListener::bind(socket_path) else {
        return Err(anyhow!("Could not connect to socket! {}", ANOTHER_MSG));
    };

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };

        tokio::spawn(task(stream));
    }
}

fn acquire_run_lock() -> Result<Flock<std::fs::File>> {
    let _ = fs::create_dir_all(get_tmp_dir()?);

    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(get_lock_path()?)?;

    Flock::lock(lock_file, FlockArg::LockExclusiveNonblock)
        .map_err(|(_, err)| io::Error::from(err))
        .map_err(anyhow::Error::msg)
}

async fn task<'a>(mut stream: UnixStream) -> Option<()> {
    let message = read(&mut stream).await?;

    let response = match message {
        Request::Inhibit { id } => inhibit(id).await,
        Request::Release { id } => release(id).await,
        Request::Status { id } => status(id).await,
        Request::ActiveInhibitors => active_inhibitors().await,
    };

    let _ = stream.write_all(response.ser().as_bytes()).await;

    Some(())
}

async fn inhibit(id: String) -> Response {
    let Some(state) = STATE.get() else {
        return ErrorKind::DaemonError.response();
    };

    if state.inhibit(&id).await.is_err() {
        return ErrorKind::OperationFailed.response();
    }

    Response::Ok
}

async fn release(id: String) -> Response {
    let Some(state) = STATE.get() else {
        return ErrorKind::DaemonError.response();
    };

    state.release(&id).await;

    Response::Ok
}

async fn read(stream: &mut UnixStream) -> Option<Request> {
    let mut message = String::new();
    if let Err(_) = stream.read_to_string(&mut message).await {
        let _ = stream
            .write_all(ErrorKind::SocketError.response().ser().as_bytes())
            .await;
        return None;
    };

    return match serde_json::from_str(&message) {
        Ok(request) => Some(request),
        Err(_) => {
            let _ = stream
                .write_all(ErrorKind::InvalidRequest.response().ser().as_bytes())
                .await;

            return None;
        }
    };
}

async fn status(id: String) -> Response {
    let Some(state) = STATE.get() else {
        return ErrorKind::DaemonError.response();
    };

    let is_inhibited = state.is_inhibited(&id).await;

    Response::Status {
        status: if is_inhibited {
            Status::Inhibits
        } else {
            Status::Free
        },
    }
}

async fn active_inhibitors() -> Response {
    let Some(state) = STATE.get() else {
        return ErrorKind::DaemonError.response();
    };

    Response::ActiveInhibitors {
        active_inhibitors: state.active_inhibitors().await,
    }
}
