use std::{
    sync::atomic::Ordering,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use tauri::{AppHandle, Emitter, Manager};
use tertius_core::{CleanupPipeline, Transcript, WritingContext, model_catalog};
use uuid::Uuid;

use crate::{
    insert::InsertOutcome,
    state::{AppState, InsertionTarget, Phase, RecordingMode},
};

const MAXIMUM_RECORDING: Duration = Duration::from_secs(300);

pub async fn begin_recording(app: AppHandle, mode: RecordingMode) -> Result<()> {
    let settings = app.state::<AppState>().store.snapshot().settings;
    let model = model_catalog()
        .into_iter()
        .find(|model| model.id == settings.model_id)
        .context("the selected speech model is not in the catalog")?;
    let model_path = app
        .state::<AppState>()
        .store
        .models_dir()
        .join(model.file_name);
    if !speech_model_ready(&model_path) {
        bail!("the selected speech engine has not been downloaded");
    }

    let target = capture_insertion_target(&app);
    let generation = {
        let state = app.state::<AppState>();
        let mut runtime = state.runtime.lock();
        if runtime.phase != Phase::Idle {
            bail!("Tertius is already working");
        }
        state.cancel.store(false, Ordering::Release);
        runtime.phase = Phase::Starting;
        runtime.mode = mode;
        runtime.started = None;
        runtime.pressed = Some(std::time::Instant::now());
        runtime.pending_finish = false;
        runtime.generation = runtime.generation.wrapping_add(1);
        runtime.app_name = target.as_ref().map(|target| target.app_name.clone());
        runtime.target_process_id = target.as_ref().map(|target| target.process_id);
        runtime.message = None;
        runtime.preview = None;
        runtime.generation
    };
    emit_status(&app);

    let app_for_audio = app.clone();
    let mut microphone_setup = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        crate::audio::ensure_microphone_access(&app_for_audio)?;
        app_for_audio
            .state::<AppState>()
            .recorder
            .start(MAXIMUM_RECORDING)
    });
    let setup_result = tokio::time::timeout(Duration::from_secs(70), &mut microphone_setup).await;
    match setup_result {
        Ok(joined) => {
            if !session_current(&app, generation) {
                app.state::<AppState>().recorder.cancel();
                return Ok(());
            }
            joined.context("microphone setup stopped unexpectedly")??;
        }
        Err(_) => {
            let cleanup_app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = microphone_setup.await;
                cleanup_app.state::<AppState>().recorder.cancel();
            });
            reset_session(&app, generation);
            bail!("microphone setup timed out");
        }
    }

    let pending_finish = {
        let state = app.state::<AppState>();
        let mut runtime = state.runtime.lock();
        if runtime.generation != generation || runtime.phase != Phase::Starting {
            drop(runtime);
            state.recorder.cancel();
            return Ok(());
        }
        runtime.phase = Phase::Recording;
        runtime.started = Some(std::time::Instant::now());
        runtime.pending_finish
    };
    emit_status(&app);
    if pending_finish {
        return finish_recording(app).await;
    }
    spawn_status_ticker(app.clone(), generation);
    Ok(())
}

pub async fn finish_recording(app: AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let (app_name, target_process_id, generation) = {
        let mut runtime = state.runtime.lock();
        if runtime.phase != Phase::Recording {
            return Ok(());
        }
        runtime.phase = Phase::Transcribing;
        runtime.pending_finish = false;
        (
            runtime.app_name.clone(),
            runtime.target_process_id,
            runtime.generation,
        )
    };
    emit_status(&app);
    let recording = state.recorder.stop()?;
    let peak = recording
        .samples
        .iter()
        .fold(0.0f32, |current, sample| current.max(sample.abs()));
    let rms = if recording.samples.is_empty() {
        0.0
    } else {
        (recording
            .samples
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / recording.samples.len() as f32)
            .sqrt()
    };
    tracing::info!(
        samples = recording.samples.len(),
        duration_ms = recording.duration.as_millis(),
        peak,
        rms,
        "microphone capture completed"
    );
    if recording.samples.len() < 1_600 {
        reset_session(&app, generation);
        return Ok(());
    }
    if peak < 0.001 && rms < 0.00025 {
        report_failure(
            &app,
            "The microphone returned silence. Check macOS Microphone access, then try again.",
        );
        return Ok(());
    }

    let data = state.store.snapshot();
    let model = model_catalog()
        .into_iter()
        .find(|model| model.id == data.settings.model_id)
        .context("the selected speech model is not in the catalog")?;
    let model_path = state.store.models_dir().join(model.file_name);
    if !speech_model_ready(&model_path) {
        report_failure(
            &app,
            "Set up the local speech engine once, then Tertius is ready offline.",
        );
        bail!("the selected speech engine has not been downloaded");
    }

    let samples = recording.samples;
    let cancel = state.cancel.clone();
    let model_path_for_task = model_path.clone();
    let app_for_task = app.clone();
    let transcription = tauri::async_runtime::spawn_blocking(move || {
        app_for_task.state::<AppState>().transcriber.transcribe(
            &model_path_for_task,
            &samples,
            cancel,
        )
    })
    .await;
    if !session_current(&app, generation) {
        return Ok(());
    }
    let raw = transcription??;
    if raw.trim().is_empty() {
        report_failure(&app, "I didn’t catch that. Try once more.");
        return Ok(());
    }

    set_phase_for_session(&app, generation, Phase::Cleaning, None);
    let context = WritingContext::for_app(app_name.as_deref());
    let cleanup = CleanupPipeline::default().run(&raw, context);
    if !session_current(&app, generation) {
        return Ok(());
    }

    set_preview_for_session(&app, generation, cleanup.text.clone());
    set_phase_for_session(&app, generation, Phase::Inserting, None);
    if !session_current(&app, generation) {
        return Ok(());
    }
    let text = cleanup.text.clone();
    let press_enter = cleanup.press_enter;
    let paste = target_process_id.is_some() && !is_tertius_window(app_name.as_deref());
    let app_for_insertion = app.clone();
    let insertion = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            crate::insert::insert_text_on_main_thread(
                &app_for_insertion,
                text,
                press_enter,
                paste,
                target_process_id,
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            crate::insert::insert_text(&text, press_enter, paste)
        }
    })
    .await;
    if !session_current(&app, generation) {
        return Ok(());
    }
    let insertion = insertion??;

    let transcript = Transcript {
        id: Uuid::new_v4(),
        created_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        duration_ms: recording.duration.as_millis() as u64,
        words: cleanup.text.split_whitespace().count(),
        text: cleanup.text,
        app_name,
    };
    if !session_current(&app, generation) {
        return Ok(());
    }
    state.store.add_transcript(transcript.clone())?;
    let _ = app.emit("transcript-added", transcript);
    let message = match insertion {
        InsertOutcome::Pasted => "Written and copied to your clipboard.",
        InsertOutcome::CopiedOnly => "Copied to your clipboard — paste anywhere.",
    };
    set_phase_for_session(&app, generation, Phase::Complete, Some(message.into()));
    tokio::time::sleep(Duration::from_millis(1_250)).await;
    reset_session(&app, generation);
    Ok(())
}

pub fn cancel(app: &AppHandle) {
    let state = app.state::<AppState>();
    let generation = {
        let runtime = state.runtime.lock();
        if runtime.phase == Phase::Idle {
            return;
        }
        runtime.generation
    };
    state.cancel.store(true, Ordering::Release);
    state.recorder.cancel();
    reset_session(app, generation);
}

pub fn emit_status(app: &AppHandle) {
    let status = app.state::<AppState>().status();
    let _ = app.emit("runtime-status", status);
}

fn set_phase_for_session(app: &AppHandle, generation: u64, phase: Phase, message: Option<String>) {
    let state = app.state::<AppState>();
    {
        let mut runtime = state.runtime.lock();
        if runtime.generation != generation {
            return;
        }
        runtime.phase = phase;
        runtime.message = message;
    }
    emit_status(app);
}

fn reset_session(app: &AppHandle, generation: u64) {
    let state = app.state::<AppState>();
    {
        let mut runtime = state.runtime.lock();
        if runtime.generation != generation {
            return;
        }
        runtime.phase = Phase::Idle;
        runtime.mode = RecordingMode::PushToTalk;
        runtime.started = None;
        runtime.pressed = None;
        runtime.pending_finish = false;
        runtime.app_name = None;
        runtime.target_process_id = None;
        runtime.message = None;
        runtime.preview = None;
        runtime.generation = runtime.generation.wrapping_add(1);
    }
    emit_status(app);
}

pub fn report_failure(app: &AppHandle, message: &str) {
    let generation = {
        let state = app.state::<AppState>();
        let mut runtime = state.runtime.lock();
        runtime.phase = Phase::Error;
        runtime.message = Some(message.to_owned());
        runtime.generation
    };
    emit_status(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if app.state::<AppState>().runtime.lock().phase == Phase::Error {
            reset_session(&app, generation);
        }
    });
}

fn session_current(app: &AppHandle, generation: u64) -> bool {
    let state = app.state::<AppState>();
    state.runtime.lock().generation == generation && !state.cancel.load(Ordering::Acquire)
}

fn speech_model_ready(path: &std::path::Path) -> bool {
    [
        "encoder-model.int8.onnx",
        "decoder_joint-model.int8.onnx",
        "nemo128.onnx",
        "vocab.txt",
    ]
    .iter()
    .all(|name| path.join(name).is_file())
}

fn is_tertius_window(app_name: Option<&str>) -> bool {
    app_name.is_some_and(|name| name.to_ascii_lowercase().contains("tertius"))
}

fn capture_insertion_target(app: &AppHandle) -> Option<InsertionTarget> {
    let state = app.state::<AppState>();
    if let Ok(window) = active_win_pos_rs::get_active_window()
        && !is_tertius_window(Some(&window.app_name))
    {
        let target = InsertionTarget {
            app_name: window.app_name,
            process_id: window.process_id,
        };
        *state.last_external_target.lock() = Some(target.clone());
        return Some(target);
    }
    state.last_external_target.lock().clone()
}

pub fn spawn_target_tracker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(140)).await;
            if let Ok(window) = active_win_pos_rs::get_active_window()
                && !is_tertius_window(Some(&window.app_name))
            {
                *app.state::<AppState>().last_external_target.lock() = Some(InsertionTarget {
                    app_name: window.app_name,
                    process_id: window.process_id,
                });
            }
        }
    });
}

fn set_preview_for_session(app: &AppHandle, generation: u64, preview: String) {
    let state = app.state::<AppState>();
    {
        let mut runtime = state.runtime.lock();
        if runtime.generation != generation {
            return;
        }
        runtime.preview = Some(preview);
    }
    emit_status(app);
}

fn spawn_status_ticker(app: AppHandle, generation: u64) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(70)).await;
            let state = app.state::<AppState>();
            let (phase, mode, elapsed, current_generation) = {
                let runtime = state.runtime.lock();
                (
                    runtime.phase,
                    runtime.mode,
                    runtime
                        .started
                        .map(|value| value.elapsed())
                        .unwrap_or_default(),
                    runtime.generation,
                )
            };
            if current_generation != generation || phase != Phase::Recording {
                break;
            }
            emit_status(&app);
            if mode != RecordingMode::PushToTalk && elapsed >= MAXIMUM_RECORDING {
                let _ = finish_recording(app.clone()).await;
                break;
            }
        }
    });
}
