use std::{env, path::PathBuf};

use anyhow::{anyhow, Result};

pub mod protocol;

const DIR_NAME: &'static str = "doppio";

pub struct Locations {
    pub tmp_dir: PathBuf,
    pub socket_path: PathBuf,
    pub lock_path: PathBuf,
}

impl Locations {
    pub fn new() -> Result<Self> {
        let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
            .ok_or_else(|| anyhow!("XDG_RUNTIME_DIR not set. Is your session running?"))?;

        let mut tmp_dir = PathBuf::from(runtime_dir);
        tmp_dir.push(DIR_NAME);

        let mut socket_path = tmp_dir.clone();
        socket_path.push("doppio.sock");

        let mut lock_path = tmp_dir.clone();
        lock_path.push("doppio.lock");

        Ok(Locations {
            tmp_dir,
            socket_path,
            lock_path,
        })
    }
}
