mod state;

use std::{
    fs::{self, remove_file, OpenOptions},
    io,
    sync::OnceLock,
};

use anyhow::{anyhow, Result};
use doppio::{
    protocol::{ErrorKind, Request, Response, Status},
    Locations,
};
use nix::fcntl::{Flock, FlockArg};
use state::State;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};
use zbus::Connection;

const ANOTHER_MSG: &'static str = "Is another instance of doppio-daemon running?";

#[tokio::main]
async fn main() -> Result<()> {
    let locations = Locations::new()?;

    let Ok(connection) = Connection::system().await else {
        return Err(anyhow!(
            "Could not connect to system bus! Is d-bus running?"
        ));
    };

    let Ok(state) = State::new(&connection).await else {
        return Err(anyhow!(
            "Could not connect to login1 Manager! Is systemd configured correctly?"
        ));
    };

    static STATE: OnceLock<State> = OnceLock::new();
    let state = STATE.get_or_init(|| state);

    let Ok(_lock) = acquire_run_lock(&locations) else {
        return Err(anyhow!("Could not acquire lock file! {}", ANOTHER_MSG));
    };

    let _ = remove_file(&locations.socket_path);
    let Ok(listener) = UnixListener::bind(&locations.socket_path) else {
        return Err(anyhow!("Could not connect to socket! {}", ANOTHER_MSG));
    };

    loop {
        if let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(task(&state, stream));
        }
    }
}

fn acquire_run_lock(locations: &Locations) -> Result<Flock<std::fs::File>> {
    let _ = fs::create_dir_all(&locations.tmp_dir);

    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&locations.lock_path)?;

    Flock::lock(lock_file, FlockArg::LockExclusiveNonblock)
        .map_err(|(_, err)| io::Error::from(err))
        .map_err(anyhow::Error::msg)
}

async fn task(state: &State<'_>, mut stream: UnixStream) {
    let Some(message) = read(&mut stream).await else {
        return;
    };

    let response = match message {
        Request::Inhibit { id } => inhibit(&state, id).await,
        Request::Release { id } => release(&state, id).await,
        Request::Status { id } => status(&state, id).await,
        Request::ActiveInhibitors => active_inhibitors(&state).await,
    };

    let _ = stream.write_all(response.ser().as_bytes()).await;
}

async fn inhibit(state: &State<'_>, id: String) -> Response {
    if state.inhibit(&id).await.is_err() {
        ErrorKind::OperationFailed.response()
    } else {
        Response::Ok
    }
}

async fn release(state: &State<'_>, id: String) -> Response {
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

    match Request::des(&message) {
        Some(request) => Some(request),
        None => {
            let _ = stream
                .write_all(ErrorKind::InvalidRequest.response().ser().as_bytes())
                .await;

            None
        }
    }
}

async fn status(state: &State<'_>, id: String) -> Response {
    let status = if state.is_inhibited(&id).await {
        Status::Inhibits
    } else {
        Status::Free
    };

    Response::Status { status }
}

async fn active_inhibitors(state: &State<'_>) -> Response {
    let active_inhibitors = state.active_inhibitors().await;
    Response::ActiveInhibitors { active_inhibitors }
}
