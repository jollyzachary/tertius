use std::{thread, time::Duration};

use anyhow::{Context, Result};
use arboard::Clipboard;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Pasted,
    CopiedOnly,
}

#[cfg(target_os = "macos")]
pub fn insert_text_on_main_thread(
    app: &tauri::AppHandle,
    text: String,
    press_enter: bool,
    paste: bool,
    target_process_id: Option<u64>,
) -> Result<InsertOutcome> {
    use std::sync::mpsc;

    let (sender, receiver) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        if paste {
            restore_macos_target(target_process_id);
            thread::sleep(Duration::from_millis(80));
        }
        let _ = sender.send(insert_text(&text, press_enter, paste));
    })
    .context("automatic insertion could not reach the macOS main thread")?;

    receiver
        .recv_timeout(Duration::from_secs(5))
        .context("automatic insertion timed out on the macOS main thread")?
}

#[cfg(target_os = "macos")]
fn restore_macos_target(process_id: Option<u64>) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    let Some(process_id) = process_id.and_then(|value| i32::try_from(value).ok()) else {
        return;
    };
    let Some(application) =
        NSRunningApplication::runningApplicationWithProcessIdentifier(process_id)
    else {
        return;
    };
    application.activateWithOptions(NSApplicationActivationOptions::empty());
}

pub fn auto_insert_ready(prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    {
        use macos_accessibility_client::accessibility::{
            application_is_trusted, application_is_trusted_with_prompt,
        };
        if prompt {
            application_is_trusted_with_prompt()
        } else {
            application_is_trusted()
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = prompt;
        true
    }
}

pub fn insert_text(text: &str, press_enter: bool, paste: bool) -> Result<InsertOutcome> {
    copy_text(text)?;
    if !paste {
        return Ok(InsertOutcome::CopiedOnly);
    }
    // Give the destination application time to observe the clipboard update.
    thread::sleep(Duration::from_millis(100));

    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(enigo) => enigo,
        Err(error) => {
            tracing::error!(%error, "automatic insertion could not connect to keyboard input");
            return Ok(InsertOutcome::CopiedOnly);
        }
    };
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;
    if let Err(error) = enigo.key(modifier, Press) {
        tracing::error!(%error, "automatic insertion could not press the paste modifier");
        return Ok(InsertOutcome::CopiedOnly);
    }
    let pasted = enigo.key(Key::Unicode('v'), Click).is_ok();
    let released = enigo.key(modifier, Release).is_ok();
    if !pasted || !released {
        tracing::error!(
            pasted,
            released,
            "automatic insertion could not send the paste shortcut"
        );
        return Ok(InsertOutcome::CopiedOnly);
    }
    if press_enter {
        thread::sleep(Duration::from_millis(35));
        if let Err(error) = enigo.key(Key::Return, Click) {
            tracing::error!(%error, "automatic insertion could not press Return");
            return Ok(InsertOutcome::CopiedOnly);
        }
    }
    Ok(InsertOutcome::Pasted)
}

pub fn copy_text(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("clipboard is unavailable")?;
    clipboard
        .set_text(text)
        .context("could not place the transcript on the clipboard")?;
    Ok(())
}
