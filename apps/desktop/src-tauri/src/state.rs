use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use anyhow::Result;
use parking_lot::Mutex;
use serde::Serialize;
use tertius_core::DataStore;

use crate::{audio::AudioRecorder, transcriber::Transcriber};

pub struct AppState {
    pub store: DataStore,
    pub recorder: AudioRecorder,
    pub transcriber: Transcriber,
    pub cancel: Arc<AtomicBool>,
    pub shortcut_ready: AtomicBool,
    pub suppress_reopen: AtomicBool,
    pub last_external_target: Mutex<Option<InsertionTarget>>,
    pub runtime: Mutex<RuntimeInternal>,
}

impl AppState {
    pub fn open() -> Result<Self> {
        Ok(Self {
            store: DataStore::open()?,
            recorder: AudioRecorder::new(),
            transcriber: Transcriber::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            shortcut_ready: AtomicBool::new(false),
            suppress_reopen: AtomicBool::new(false),
            last_external_target: Mutex::new(None),
            runtime: Mutex::new(RuntimeInternal::default()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct InsertionTarget {
    pub app_name: String,
    pub process_id: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    #[default]
    Idle,
    Starting,
    Recording,
    Transcribing,
    Cleaning,
    Inserting,
    Complete,
    Error,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecordingMode {
    #[default]
    PushToTalk,
    HandsFree,
    Manual,
}

#[derive(Default)]
pub struct RuntimeInternal {
    pub phase: Phase,
    pub mode: RecordingMode,
    pub started: Option<Instant>,
    pub pressed: Option<Instant>,
    pub pending_finish: bool,
    pub generation: u64,
    pub app_name: Option<String>,
    pub target_process_id: Option<u64>,
    pub message: Option<String>,
    pub preview: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub phase: Phase,
    pub mode: RecordingMode,
    pub level: f32,
    pub elapsed_ms: u64,
    pub message: Option<String>,
    pub preview: Option<String>,
}

impl AppState {
    pub fn status(&self) -> RuntimeStatus {
        let runtime = self.runtime.lock();
        RuntimeStatus {
            phase: runtime.phase,
            mode: runtime.mode,
            level: self.recorder.level(),
            elapsed_ms: runtime
                .started
                .map(|started| started.elapsed().as_millis() as u64)
                .unwrap_or(0),
            message: runtime.message.clone(),
            preview: runtime.preview.clone(),
        }
    }
}
