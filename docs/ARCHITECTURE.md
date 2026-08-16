# Tertius architecture

## Decision

Use a Rust product core with a Tauri 2 desktop shell and a React/TypeScript interface.

Dictation sits at the operating-system boundary: microphone streams, global shortcuts, active-window metadata, local inference, clipboard ownership, synthesized input, and native packaging. Rust gives those paths one compiled implementation. Tauri provides native macOS, Windows, and Linux bundles while keeping the audio and inference loop outside a browser service.

## Runtime flow

1. The operating system registers Control + Option + V on macOS, or Control + Alt + V on Windows and Linux, and reports both press and release events.
2. The state machine chooses hold-to-talk, toggle, finish, or no action.
3. CPAL captures the default input device, mixes it to mono, and resamples it to 16 kHz.
4. The non-focus-stealing top overlay appears as soon as activation begins, then receives live level and phase events.
5. `transcribe-rs` runs the local INT8 Parakeet ONNX model.
6. `tertius-core` applies deterministic cleanup, explicit formatting cues, and a broad app-category rule.
7. Tertius places the finished text on the clipboard, pastes it into the focused field when the OS permits synthesized input, and leaves the same text copied as a fallback.
8. The final transcript is added to a rolling three-day local history. Audio and the raw transcript are discarded.

The voice widget is a focus-disabled always-on-top window, so clicking it does not steal the target field. It collapses to a small edge bump, expands on hover or while active, and stores its edge/monitor anchor locally. Drag release resolves to the nearest top, right, bottom, or left edge center.

## State and cancellation

```text
IDLE -> STARTING -> RECORDING -> TRANSCRIBING -> CLEANING -> INSERTING -> COMPLETE -> IDLE
           |            |              |             |           |
           +------------+------ cancel +-------------+-----------+
```

Every recording receives a monotonically changing generation. Each asynchronous boundary verifies that generation before changing state, inserting text, or saving history. This prevents a slow microphone startup, delayed transcription, old status ticker, or delayed error timer from touching a newer session.

Cancellation is immediate at the product boundary: the interface resets and a cancelled result cannot be inserted or saved. The current ONNX call may continue internally until inference returns. Moving inference into an isolated worker will add immediate process-level cancellation.

## Formatting contract

The speech model provides automatic punctuation and capitalization. The deterministic cleanup layer then handles only rules it can apply without guessing:

- filler words `um`, `uh`, and `erm`;
- “scratch that” backtracking;
- “new line” and “new paragraph”;
- explicit punctuation commands such as “insert comma”;
- repeated bullet or numbered item cues;
- a final “press enter” command.

Ordinary uses of words such as “period,” “comma,” or “colon” are left untouched. This prevents the cleanup layer from corrupting legitimate sentences and leaves natural punctuation to the speech model.

App awareness uses only the active application name:

- messaging apps receive sentence casing without a forced final period;
- native email and writing apps receive complete-sentence punctuation;
- development tools preserve casing and technical syntax;
- unknown apps and browsers use a neutral rule.

Page-level or field-level classification would require accessibility or browser inspection and is intentionally outside this lightweight build.

## Data boundary

All user data is JSON in the native application-data directory. Persisted settings contain only the activation mode and model identifier. History contains only the finished text, application name, duration, timestamp, and word count. Entries older than three days are pruned on startup and whenever a new dictation is saved. The speech model lives beside the data in a `models` directory.

The model installer streams a compressed archive into the application-data directory while calculating SHA-256. It rejects the download unless both the exact byte count and pinned digest match. Only then does it extract the four expected model files into a private staging directory, confirm that all four exist, and install them. Flattening archive entries to an allowlist of file names prevents the archive from choosing output paths. The application has no account, analytics SDK, remote transcription provider, or background service.

## Why ONNX Parakeet

A single ONNX path gives Tertius a practical hardware matrix without separate inference implementations: CPU, CoreML, DirectML, CUDA, ROCm, WebGPU, and XNNPACK. Parakeet v3 also supplies automatic punctuation, capitalization, language detection, and 25-language recognition before the local cleanup layer runs.

The tradeoff is a roughly 478 MB model and a young Rust integration. `Transcriber` and `ModelDescriptor` isolate the engine so another local model can be introduced without changing capture, cleanup, insertion, or UI contracts.

## Cross-platform boundary

The core and desktop source are shared, but native artifacts must be built on their target operating systems:

- macOS: Tauri `.app` and `.dmg`; optional `coreml`
- Windows: NSIS/MSI; optional `directml` or `cuda`
- Linux: AppImage, Debian, or RPM; optional `cuda`, `rocm`, or `webgpu`

Linux global shortcuts and input injection depend on the desktop session. X11 is the current baseline. Wayland requires compositor-specific QA and may eventually need a portal or `libei` insertion backend.
