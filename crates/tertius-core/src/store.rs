use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use atomicwrites::{AllowOverwrite, AtomicFile};
use parking_lot::RwLock;

use crate::{Transcript, UserData};

const HISTORY_RETENTION_MS: u64 = 3 * 24 * 60 * 60 * 1_000;

pub struct DataStore {
    root: PathBuf,
    data: RwLock<UserData>,
}

impl DataStore {
    pub fn open() -> Result<Self> {
        let root = dirs::data_local_dir()
            .context("the operating system did not provide a local data directory")?
            .join("Farynth")
            .join("Tertius");
        fs::create_dir_all(root.join("models"))?;
        let path = root.join("tertius.json");
        let mut needs_save = !path.exists();
        let mut data: UserData = if path.exists() {
            let bytes = fs::read(&path)?;
            match serde_json::from_slice(&bytes) {
                Ok(data) => data,
                Err(error) => {
                    tracing::warn!(%error, "could not read user data; replacing it with safe defaults");
                    needs_save = true;
                    UserData::default()
                }
            }
        } else {
            UserData::default()
        };
        let previous_history_len = data.history.len();
        prune_history(&mut data.history, now_ms());
        needs_save |= data.history.len() != previous_history_len;
        let store = Self {
            root,
            data: RwLock::new(data),
        };
        if needs_save {
            store.save()?;
        }
        Ok(store)
    }

    pub fn snapshot(&self) -> UserData {
        self.data.read().clone()
    }

    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    pub fn update(&self, update: impl FnOnce(&mut UserData)) -> Result<UserData> {
        let mut data = self.data.write();
        let mut snapshot = data.clone();
        update(&mut snapshot);
        self.save_snapshot(&snapshot)?;
        *data = snapshot.clone();
        Ok(snapshot)
    }

    pub fn add_transcript(&self, transcript: Transcript) -> Result<()> {
        let now = now_ms();
        self.update(|data| {
            data.history.insert(0, transcript);
            prune_history(&mut data.history, now);
        })?;
        Ok(())
    }

    fn save(&self) -> Result<()> {
        self.save_snapshot(&self.data.read())
    }

    fn save_snapshot(&self, data: &UserData) -> Result<()> {
        let destination = self.root.join("tertius.json");
        let encoded = serde_json::to_vec_pretty(data)?;
        AtomicFile::new(destination, AllowOverwrite)
            .write(|file| file.write_all(&encoded))
            .map_err(std::io::Error::from)?;
        Ok(())
    }
}

fn now_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

fn prune_history(history: &mut Vec<Transcript>, now: u64) {
    let cutoff = now.saturating_sub(HISTORY_RETENTION_MS);
    history.retain(|transcript| transcript.created_at_ms >= cutoff);
}
