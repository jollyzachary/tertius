use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tertius_core::ActivationMode;

use crate::{
    pipeline,
    state::{AppState, Phase, RecordingMode},
};

pub fn ensure_registered(app: &AppHandle) -> bool {
    if app
        .state::<AppState>()
        .shortcut_ready
        .load(Ordering::Acquire)
    {
        return true;
    }

    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV);
    match app.global_shortcut().register(shortcut) {
        Ok(()) => {
            app.state::<AppState>()
                .shortcut_ready
                .store(true, Ordering::Release);
            true
        }
        Err(error) => {
            tracing::error!(%error, shortcut = "Control+Alt+V", "global dictation shortcut could not be registered");
            false
        }
    }
}

pub fn handle(app: &AppHandle, state: ShortcutState) {
    let app_state = app.state::<AppState>();
    let activation = app_state.store.snapshot().settings.activation_mode;
    let phase = app_state.runtime.lock().phase;

    match (activation, state, phase) {
        (ActivationMode::Hold, ShortcutState::Pressed, Phase::Idle) => {
            begin(app.clone(), RecordingMode::PushToTalk)
        }
        (ActivationMode::Hold, ShortcutState::Released, Phase::Starting) => {
            mark_pending_finish(app)
        }
        (ActivationMode::Hold, ShortcutState::Released, Phase::Recording) => finish(app.clone()),
        (ActivationMode::Toggle, ShortcutState::Pressed, Phase::Idle) => {
            begin(app.clone(), RecordingMode::HandsFree)
        }
        (ActivationMode::Toggle, ShortcutState::Pressed, Phase::Starting) => {
            mark_pending_finish(app)
        }
        (ActivationMode::Toggle, ShortcutState::Pressed, Phase::Recording) => finish(app.clone()),
        _ => {}
    }
}

fn begin(app: AppHandle, mode: RecordingMode) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = pipeline::begin_recording(app.clone(), mode).await {
            tracing::error!(%error, "dictation could not start");
            pipeline::report_failure(&app, &friendly_error(&error.to_string()));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

fn mark_pending_finish(app: &AppHandle) {
    app.state::<AppState>().runtime.lock().pending_finish = true;
}

fn finish(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = pipeline::finish_recording(app.clone()).await {
            tracing::error!(%error, "dictation pipeline failed");
            pipeline::report_failure(&app, &friendly_error(&error.to_string()));
        }
    });
}

fn friendly_error(error: &str) -> String {
    if error.contains("timed out") {
        "The microphone did not respond. Check Sound settings, then try again.".into()
    } else if error.contains("speech engine") {
        "Set up the local speech engine once, then Tertius is ready offline.".into()
    } else if error.contains("microphone") || error.contains("input device") {
        "Tertius needs microphone access to listen.".into()
    } else {
        "Tertius was interrupted. Try the shortcut again.".into()
    }
}
