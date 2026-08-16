# Troubleshooting Tertius

Start with the failure stage. Microphone capture, transcription, clipboard copy, and automatic insertion are separate parts of the pipeline.

## The speech engine is not ready

The first run downloads about 478 MB. Confirm that:

- the device has enough free disk space for the archive, extraction staging area, and installed model;
- the network can reach `https://blob.handy.computer`;
- the app's data directory is writable;
- a proxy or security product did not replace or truncate the archive.

Tertius checks both file size and SHA-256. If integrity verification fails, the temporary archive is rejected. Retry on a trusted network. Do not disable integrity verification.

## The shortcut does nothing

The default shortcut is:

- macOS: `Control + Option + V`
- Windows and Linux: `Control + Alt + V`

Another application can reserve the same combination. Open Tertius and choose **Retry shortcut**, or use the voice widget while diagnosing the conflict. Hold mode starts on key press and finishes on key release. Press-on/press-off mode requires a second press.

On Linux, global shortcut behavior depends on the desktop session. X11 is the current baseline. Wayland compositors may block or reserve global shortcuts.

## Tertius cannot hear the microphone

1. Confirm that the operating system allows Tertius to use the microphone.
2. Confirm that the intended microphone is the system default input device.
3. Check that another application is not holding the input device exclusively.
4. Speak for at least a short phrase. Extremely short recordings are discarded.
5. Quit and reopen Tertius after changing microphone permissions.

On macOS, review **System Settings > Privacy & Security > Microphone**. A local rebuild with a different signature or path can appear as a new app identity.

On Windows, enable microphone access for desktop applications.

On Linux, confirm that the default PipeWire, PulseAudio, or ALSA input works for other native applications in the same session.

## Dictation is copied but not inserted

Automatic insertion uses the clipboard and the operating system's normal paste shortcut.

On macOS:

1. Open **System Settings > Privacy & Security > Accessibility**.
2. Confirm the exact Tertius application you launched is enabled.
3. If several old Tertius entries exist, remove stale entries, add the current app, and reopen it.
4. Keep the destination text field selected before activating dictation.

The clipboard copy is the fallback. If the text is present on the clipboard, microphone capture, transcription, and cleanup worked even if input injection did not.

On Windows or Linux, check whether the destination application, remote-desktop session, compositor, or security policy blocks synthesized keyboard input.

## Clicking the voice widget targets the wrong place

Select the destination text field first, then click the widget's microphone. The widget is designed not to take focus and Tertius records the last external application as the insertion target.

If the destination closes, changes process identity, or blocks reactivation before transcription finishes, Tertius leaves the text on the clipboard instead.

## The widget is missing

Use the tray or menu-bar icon and choose **Show Dictation Widget**. The widget remembers its previous display position. If that display is disconnected, Tertius clamps the widget to an available monitor's working area when it initializes.

## The main window closes but Tertius keeps running

This is expected. Closing the main window hides it so the global shortcut and voice widget can remain available. Use **Quit Tertius** from the tray or menu-bar menu to stop the process.

## High CPU or a slow compose step

The default inference path uses the CPU. Duration depends on processor speed and dictation length. Optional CoreML, DirectML, CUDA, ROCm, WebGPU, and XNNPACK features are available, but each accelerated build needs host-specific runtime and driver verification.

Cancelling prevents an old result from being inserted or saved. The current in-process ONNX call can continue consuming CPU until inference returns.

## Filing a useful bug

Include:

- operating system, version, architecture, and Linux desktop session if applicable;
- Tertius version or commit;
- activation path: shortcut hold, shortcut toggle, main button, or voice widget;
- pipeline stage that failed;
- whether text reached the clipboard;
- whether a clean synthetic phrase reproduces the problem;
- relevant logs with paths, account data, transcripts, tokens, and signing information removed.

Do not attach real dictations, microphone recordings, private documents, model files, or screenshots that expose unrelated applications.
