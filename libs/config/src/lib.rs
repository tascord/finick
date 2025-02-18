use std::{
    env::var,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};
use ty::*;

pub mod ty;

pub fn finick_root() -> PathBuf {
    let path = Path::new(&var("HOME").expect("No home dir exists"))
        .join(".config")
        .join(".finick");

    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .create(path.clone())
            .expect("Failed to create finick dir");
    }

    path
}

pub fn get_config<T: DeserializeOwned + Serialize + Default>(app: App) -> Result<T> {
    let path = finick_root().join(app.to_string());
    if !path.exists() {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path.clone())?;
        serde_json::to_writer_pretty(file, &T::default())?;
        Ok(T::default())
    } else {
        serde_json::from_reader(File::open(path)?).map_err(|e| anyhow!(e))
    }
}

pub fn write_config<T: Serialize>(app: App, value: T) -> Result<()> {
    let path = finick_root().join(app.to_string());
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path.clone())?;
    serde_json::to_writer_pretty(file, &value)?;
    Ok(())
}
