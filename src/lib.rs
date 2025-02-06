use serde::{Deserialize, Serialize};

pub const SOCKET_PATH: &'static str = "/tmp/doppio.sock";

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    Inhibit { id: String },
    Release { id: String },
    Status { id: String },
    ActiveInhibitors,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    Ok,
    Status { status: Status },
    ActiveInhibitors { active_inhibitors: Vec<String> },
    Error { kind: Error },
}

#[derive(Serialize, Deserialize)]
pub enum Status {
    Inhibits,
    Free,
}

#[derive(Serialize, Deserialize)]
pub enum Error {
    SocketError,
    InvalidRequest,
    DaemonError,
    OperationFailed,
}

impl Request {
    pub fn des(data: &str) -> Option<Self> {
        serde_json::from_str(data).ok()
    }

    pub fn ser(&self) -> String {
        // Unwrap should not panic, because Serialize
        // implementation from macro is used and
        // no maps are used in Respone at all
        serde_json::to_string(self).unwrap()
    }
}

impl Response {
    pub fn des(data: &str) -> Option<Self> {
        serde_json::from_str(data).ok()
    }

    pub fn ser(&self) -> String {
        // Unwrap should not panic, because Serialize
        // implementation from macro is used and
        // no maps are used in Respone at all
        serde_json::to_string(self).unwrap()
    }
}

impl Error {
    pub fn response(self) -> Response {
        Response::Error { kind: self }
    }
}
