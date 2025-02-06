use std::{env, path::PathBuf};

use anyhow::{anyhow, Result};

pub mod protocol;

const DIR_NAME: &'static str = "doppio";

pub fn get_tmp_dir() -> Result<PathBuf> {
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| anyhow!("XDG_RUNTIME_DIR not set. Is your session running?"))?;

    let mut result = PathBuf::new();
    result.push(runtime_dir);
    result.push(DIR_NAME);
    Ok(result)
}

pub fn get_socket_path() -> Result<PathBuf> {
    let mut result = get_tmp_dir()?;
    result.push("doppio.sock");
    Ok(result)
}

pub fn get_lock_path() -> Result<PathBuf> {
    let mut result = get_tmp_dir()?;
    result.push("doppio.lock");
    Ok(result)
}
