mod state;

use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use doppio::{Request, Response, SOCKET_PATH};
use state::State;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};
use zbus::Connection;

static STATE: OnceLock<State<'static>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    let Ok(connection) = Connection::system().await else {
        return Err(anyhow!(
            "Could not connect to system bus!\nIs d-bus running?"
        ));
    };

    let _ = STATE.set(State::new(&connection).await.unwrap());

    if std::fs::exists(SOCKET_PATH).is_ok_and(|v| v) {
        return Err(anyhow!(
            "Socket {} already exists!\nIs another instance of {} runnig?",
            SOCKET_PATH,
            env!("CARGO_PKG_NAME")
        ));
    }

    let listener = UnixListener::bind(SOCKET_PATH).unwrap();

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };

        tokio::spawn(task(stream));
    }
}

async fn task<'a>(mut stream: UnixStream) -> Option<()> {
    let message = read(&mut stream).await?;

    let response = match message {
        Request::Inhibit { id } => inhibit(id).await,
        Request::Release { id } => release(id).await,
        Request::Status { id: _ } => todo!(),
        Request::ActiveInhibitors => todo!(),
    };

    let _ = stream.write_all(response.ser().as_bytes()).await;

    Some(())
}

async fn inhibit(id: String) -> Response {
    let Some(state) = STATE.get() else {
        return doppio::Error::DaemonError.response();
    };

    if state.inhibit(&id).await.is_err() {
        return doppio::Error::OperationFailed.response();
    }

    Response::Ok
}

async fn release(id: String) -> Response {
    let Some(state) = STATE.get() else {
        return doppio::Error::DaemonError.response();
    };

    if state.release(&id).await.is_err() {
        return doppio::Error::OperationFailed.response();
    }

    Response::Ok
}

async fn read(stream: &mut UnixStream) -> Option<Request> {
    let mut message = String::new();
    if let Err(_) = stream.read_to_string(&mut message).await {
        let _ = stream
            .write_all(doppio::Error::SocketError.response().ser().as_bytes())
            .await;
        return None;
    };

    return match serde_json::from_str(&message) {
        Ok(request) => Some(request),
        Err(_) => {
            let _ = stream
                .write_all(doppio::Error::InvalidRequest.response().ser().as_bytes())
                .await;

            return None;
        }
    };
}
