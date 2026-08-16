mod audio;
mod commands;
mod hotkey;
mod insert;
mod pipeline;
mod state;
mod transcriber;

use state::AppState;
use std::sync::atomic::Ordering;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| hotkey::handle(app, event.state))
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let state = AppState::open()?;
            app.manage(state);
            pipeline::spawn_target_tracker(app.handle().clone());
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window.set_resizable(true) {
                    tracing::error!(%error, "main window could not enable resizing");
                }
            }
            if let Some(window) = app.get_webview_window("overlay") {
                if let Err(error) = window.set_always_on_top(true) {
                    tracing::error!(%error, "dictation widget could not stay above other windows");
                }
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                if let Err(error) = window.set_visible_on_all_workspaces(true) {
                    tracing::error!(%error, "dictation widget could not appear on every workspace");
                }
                #[cfg(target_os = "macos")]
                configure_macos_overlay(&window)?;
            }
            let _ = hotkey::ensure_registered(app.handle());
            install_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::set_activation_mode,
            commands::enable_shortcut,
            commands::enable_auto_insert,
            commands::copy_text,
            commands::hide_main_window,
            commands::start_recording,
            commands::stop_recording,
            commands::cancel,
            commands::download_model,
        ])
        .build(tauri::generate_context!())
        .expect("Tertius could not start")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                let state = app.state::<AppState>();
                if !state.suppress_reopen.swap(false, Ordering::AcqRel)
                    && let Some(window) = app.get_webview_window("main")
                {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}

#[cfg(target_os = "macos")]
fn configure_macos_overlay(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2::{msg_send, runtime::AnyObject};

    let native_window = window.ns_window()? as *mut AnyObject;
    unsafe {
        let native_window = &*native_window;
        let _: () = msg_send![native_window, setAcceptsMouseMovedEvents: true];
        let _: () = msg_send![native_window, setHidesOnDeactivate: false];
    }
    Ok(())
}

fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open Tertius", true, None::<&str>)?;
    let show_widget = MenuItem::with_id(
        app,
        "show-widget",
        "Show Dictation Widget",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Tertius", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &show_widget, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Tertius by Farynth")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "show-widget" => {
                if let Some(window) = app.get_webview_window("overlay") {
                    let _ = window.show();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
