# Design research

Tertius is built around a simple observation: desktop dictation feels useful only when the complete interaction is reliable. Recognition quality matters, but so do activation, focus, formatting, insertion, recovery, privacy, and the amount of interface left behind.

## Product principles

### Stay with the cursor

The user should begin in the application where the text belongs. A global shortcut or the floating voice widget starts dictation without turning Tertius into the main workspace.

### Return finished writing

Speech recognition produces a transcript. A writing tool must also handle punctuation, casing, corrections, line breaks, and explicit list structure. Tertius combines the model's punctuation with a small deterministic cleanup layer so spoken formatting cues behave predictably.

### Keep the data boundary visible

Audio stays in memory and local inference turns it into text. Finished dictations remain recoverable for three days, while raw audio is discarded. The app uses the active application name for broad formatting context and avoids reading page or field content.

### Make failure recoverable

Automatic insertion depends on operating-system input permissions and the destination application. The finished text is always copied first, so a successful transcription remains available even when synthetic paste is blocked.

### Keep the interface quiet

The floating widget is small, movable, and always available. It expands for direct interaction, shows live microphone energy while listening, and uses a separate animation while composing. The main window can stay hidden while the shortcut and tray process continue working.

## Interaction decisions

Hold-to-talk is the default because press and release create a clear recording boundary. Press-on/press-off mode supports longer dictations and different mobility needs.

The floating widget is focus-disabled. Clicking its microphone records the previously active external application and restores that target before paste on macOS. Shortcut activation leaves the widget condensed and uses its animation only as status feedback.

Closing the main window hides it instead of terminating the process. The tray or menu-bar menu remains the explicit place to reopen the interface, restore the widget, or quit.

## Writing decisions

The speech model supplies automatic punctuation and capitalization. The cleanup layer handles only cues with clear intent:

- filler words such as “um,” “uh,” and “erm”;
- “scratch that” backtracking;
- “new line” and “new paragraph”;
- explicit punctuation requests such as “insert comma”;
- repeated bullet or numbered-item cues;
- a final “press enter” command.

Ordinary uses of words such as “period,” “comma,” and “colon” remain untouched. This keeps deterministic formatting from rewriting legitimate sentences.

Application context is deliberately broad:

- messaging apps favor sentence casing without forcing a final period;
- native email and writing apps favor complete-sentence punctuation;
- development tools preserve technical casing and syntax;
- browsers and unknown applications use neutral formatting.

## Technology selection

Rust owns the operating-system boundary: microphone streams, global shortcuts, active-window metadata, local inference, clipboard access, synthesized input, persistence, and native packaging.

Tauri 2 supplies cross-platform windows, tray integration, webview capabilities, and native bundles. React and TypeScript handle the main interface and floating widget.

CPAL provides shared microphone capture. `transcribe-rs` provides the Parakeet ONNX integration and optional execution providers for CPU, CoreML, DirectML, CUDA, ROCm, WebGPU, and XNNPACK.

Parakeet TDT 0.6B v3 was selected for automatic punctuation, capitalization, language detection, multilingual recognition, and practical local ONNX inference. The INT8 package is approximately 478 MB, making the first-time download larger in exchange for an offline daily workflow.

## Known tradeoffs

- Native packages must be built and checked on their target operating system.
- Linux shortcut and synthetic-input behavior depends on the desktop session, especially under Wayland.
- CPU transcription speed varies with dictation length and hardware.
- Cancelling a session prevents insertion and history updates, while an active in-process ONNX call can continue until inference returns.
- Application-name context is less specific than page-level inspection, but it keeps the privacy and permission model smaller.

## Technical references

- [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri platform signing](https://v2.tauri.app/distribute/sign/)
- [CPAL](https://github.com/RustAudio/cpal)
- [transcribe-rs](https://github.com/cjpais/transcribe-rs)
- [NVIDIA Parakeet TDT 0.6B v3 model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)
- [Parakeet ONNX conversion](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx)
