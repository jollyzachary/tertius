use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;
use tar::Archive;
use tauri::{AppHandle, Emitter, Manager};
use tertius_core::{ActivationMode, ModelDescriptor, UserData, model_catalog};
use tokio::io::AsyncWriteExt;

use crate::{
    pipeline,
    state::{AppState, RecordingMode},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    data: UserData,
    models: Vec<ModelStatus>,
    runtime: crate::state::RuntimeStatus,
    shortcut_ready: bool,
    auto_insert_ready: bool,
    platform: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    #[serde(flatten)]
    descriptor: ModelDescriptor,
    downloaded: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    id: String,
    downloaded: u64,
    total: u64,
}

#[tauri::command]
pub fn bootstrap(app: AppHandle) -> Bootstrap {
    let state = app.state::<AppState>();
    Bootstrap {
        data: state.store.snapshot(),
        models: model_statuses(&state),
        runtime: state.status(),
        shortcut_ready: state.shortcut_ready.load(Ordering::Acquire),
        auto_insert_ready: crate::insert::auto_insert_ready(false),
        platform: std::env::consts::OS.to_uppercase(),
    }
}

#[tauri::command]
pub fn set_activation_mode(app: AppHandle, mode: ActivationMode) -> Result<ActivationMode, String> {
    let state = app.state::<AppState>();
    state
        .store
        .update(|data| data.settings.activation_mode = mode)
        .map_err(|error| error.to_string())?;
    Ok(mode)
}

#[tauri::command]
pub fn enable_shortcut(app: AppHandle) -> bool {
    crate::hotkey::ensure_registered(&app)
}

#[tauri::command]
pub fn enable_auto_insert(prompt: bool) -> bool {
    crate::insert::auto_insert_ready(prompt)
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    crate::insert::copy_text(&text).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn hide_main_window(app: AppHandle) {
    let state = app.state::<AppState>();
    state.suppress_reopen.store(true, Ordering::Release);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        app.state::<AppState>()
            .suppress_reopen
            .store(false, Ordering::Release);
    });
}

#[tauri::command]
pub async fn start_recording(app: AppHandle) -> Result<(), String> {
    pipeline::begin_recording(app.clone(), RecordingMode::Manual)
        .await
        .map_err(|error| {
            let detail = error.to_string().to_ascii_lowercase();
            let message = if detail.contains("microphone access") || detail.contains("permission") {
                "Allow Tertius to use the microphone, then try dictation again."
            } else if detail.contains("timed out") {
                "The microphone did not respond. Check Sound settings, then try again."
            } else {
                "Tertius is not ready to listen yet."
            };
            pipeline::report_failure(&app, message);
            error.to_string()
        })
}

#[tauri::command]
pub async fn stop_recording(app: AppHandle) -> Result<(), String> {
    pipeline::finish_recording(app.clone())
        .await
        .map_err(|error| {
            pipeline::report_failure(&app, "Dictation was interrupted. Try once more.");
            error.to_string()
        })
}

#[tauri::command]
pub fn cancel(app: AppHandle) {
    pipeline::cancel(&app);
}

#[tauri::command]
pub async fn download_model(app: AppHandle, model_id: String) -> Result<Vec<ModelStatus>, String> {
    let descriptor = model_catalog()
        .into_iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| "unknown speech model".to_owned())?;
    let state = app.state::<AppState>();
    let destination = state.store.models_dir().join(&descriptor.file_name);
    if model_is_ready(&destination) {
        return Ok(model_statuses(&state));
    }
    download(&app, &descriptor, destination)
        .await
        .map_err(|error| error.to_string())?;
    Ok(model_statuses(&state))
}

async fn download(
    app: &AppHandle,
    model: &ModelDescriptor,
    destination: PathBuf,
) -> anyhow::Result<()> {
    let models_dir = destination
        .parent()
        .context("speech model directory has no parent")?;
    tokio::fs::create_dir_all(models_dir).await?;
    let temporary = models_dir.join(format!(".{}.tar.gz.download", model.id));
    let response = reqwest::Client::new()
        .get(&model.url)
        .send()
        .await?
        .error_for_status()?;
    if let Some(content_length) = response.content_length()
        && content_length != model.size_bytes
    {
        anyhow::bail!(
            "speech model download has an unexpected size: expected {}, received {}",
            model.size_bytes,
            content_length
        );
    }
    let total = model.size_bytes;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&temporary).await?;
    let mut digest = Sha256::new();
    let mut downloaded = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        digest.update(&chunk);
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        if downloaded > model.size_bytes {
            drop(file);
            let _ = tokio::fs::remove_file(&temporary).await;
            anyhow::bail!("speech model download exceeded its expected size");
        }
        let _ = app.emit(
            "model-download-progress",
            DownloadProgress {
                id: model.id.clone(),
                downloaded,
                total,
            },
        );
    }
    file.flush().await?;
    drop(file);
    let actual_sha256 = format!("{:x}", digest.finalize());
    if downloaded != model.size_bytes || actual_sha256 != model.sha256 {
        let _ = tokio::fs::remove_file(&temporary).await;
        anyhow::bail!("speech model integrity verification failed");
    }
    let archive = temporary.clone();
    let model_id = model.id.clone();
    tokio::task::spawn_blocking(move || install_archive(&archive, &destination, &model_id))
        .await
        .context("model installer stopped unexpectedly")??;
    Ok(())
}

fn model_statuses(state: &AppState) -> Vec<ModelStatus> {
    model_catalog()
        .into_iter()
        .map(|descriptor| ModelStatus {
            downloaded: model_is_ready(&state.store.models_dir().join(&descriptor.file_name)),
            descriptor,
        })
        .collect()
}

const MODEL_FILES: [&str; 4] = [
    "encoder-model.int8.onnx",
    "decoder_joint-model.int8.onnx",
    "nemo128.onnx",
    "vocab.txt",
];

fn model_is_ready(directory: &Path) -> bool {
    MODEL_FILES
        .iter()
        .all(|name| directory.join(name).is_file())
}

fn install_archive(archive_path: &Path, destination: &Path, model_id: &str) -> anyhow::Result<()> {
    let models_dir = destination
        .parent()
        .context("speech model directory has no parent")?;
    let extraction = models_dir.join(format!(".{model_id}-extracting"));
    if extraction.exists() {
        fs::remove_dir_all(&extraction)?;
    }
    fs::create_dir_all(&extraction)?;

    let archive_file = fs::File::open(archive_path)?;
    let mut archive = Archive::new(GzDecoder::new(archive_file));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let Some(name) = entry
            .path()?
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if MODEL_FILES.contains(&name.as_str()) {
            entry.unpack(extraction.join(name))?;
        }
    }
    if !model_is_ready(&extraction) {
        anyhow::bail!("downloaded archive does not contain the expected Parakeet files");
    }
    fs::create_dir_all(destination)?;
    for name in MODEL_FILES {
        fs::copy(extraction.join(name), destination.join(name))?;
    }
    fs::remove_dir_all(extraction)?;
    fs::remove_file(archive_path)?;
    Ok(())
}
