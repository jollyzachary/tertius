# Tertius privacy boundary

Tertius keeps speech processing on the device. This document explains the data path and the points where the operating system or a destination application can receive text.

## Data flow

1. Tertius opens the operating system's default microphone only after dictation starts.
2. Audio samples are mixed to mono, resampled to 16 kHz, and held in process memory.
3. A local ONNX model produces a raw transcript.
4. Deterministic cleanup formats the result.
5. Tertius copies the finished text to the system clipboard.
6. When automatic insertion is available, Tertius sends the normal paste shortcut to the previously active application.
7. Tertius stores the finished text and limited metadata in its local history.
8. The in-memory audio buffer and raw transcript are dropped after the operation. They are not written to an audio or transcript file.

## What is stored

The local JSON store contains:

- activation mode;
- selected speech-model identifier;
- finished dictation text;
- creation timestamp;
- dictation duration;
- word count;
- name of the active application, when available.

History entries older than three days are removed at startup and whenever a new transcript is saved. The currently downloaded speech model is stored beside the JSON data in a `models` directory.

Default application-data locations are:

| Platform | Location |
|---|---|
| macOS | `~/Library/Application Support/Farynth/Tertius` |
| Windows | `%LOCALAPPDATA%\\Farynth\\Tertius` |
| Linux | `$XDG_DATA_HOME/Farynth/Tertius`, normally `~/.local/share/Farynth/Tertius` |

The operating system or directory library can choose a different base path.

## Local processing guarantees

Tertius operates with:

- no account or profile;
- no telemetry, product analytics, or advertising;
- no crash-upload service;
- no cloud speech recognition or remote prompt processing;
- no background microphone monitoring;
- no browser page, document, or text-field inspection.

Application-aware formatting uses only the active application's name. It does not read the application window's contents.

## Network requests

The packaged application makes one product network request when the user starts first-time speech-engine setup:

- `GET https://blob.handy.computer/parakeet-v3-int8.tar.gz`

The archive is expected to be exactly 478,517,071 bytes and must match the SHA-256 pinned in `crates/tertius-core/src/model.rs`. Installation stops before extraction if either check fails.

The model host and intervening network providers can observe ordinary connection metadata such as the requesting IP address, time, and transfer size. After the model is installed, routine dictation does not require a network connection.

Development tools make their own package-registry requests during `npm ci`, Cargo dependency resolution, and source checkout. Those are not runtime requests from the packaged app.

## Clipboard and destination applications

Every finished dictation is copied to the system clipboard. This makes recovery reliable when input injection is blocked, but it also means clipboard history tools, cloud clipboard synchronization, device-management software, or another process with clipboard access may retain the text.

If automatic insertion succeeds, the destination application receives the text and applies its own privacy, synchronization, retention, and logging rules. Tertius cannot control data after it reaches another application.

Avoid dictating secrets into a device or destination application you do not trust.

## Permissions

Tertius requests microphone access because microphone capture cannot work without it. On macOS, automatic insertion also depends on Accessibility access. Tertius uses that permission to send the paste shortcut; the current implementation does not use it to scrape interface content.

Operating-system permission databases associate approval with an application identity and location. Rebuilding, moving, or changing the signature of a local app can make the operating system treat it as a different application and request access again.

## Removing local data

Quit Tertius before removing data. Delete the Tertius application-data directory shown above to remove:

- saved settings;
- the rolling transcript history;
- the downloaded speech model;
- the remembered model installation state.

The voice widget's screen position is stored by the embedded webview as local storage. Removing the application-data directory may not clear every webview cache on every operating system. Operating-system clipboard history and text already inserted into another app must be cleared through those systems separately.

## Logs

The Rust process emits operational logs for events such as microphone sample count, duration, peak level, RMS level, shortcut registration, and insertion failures. The application does not intentionally log captured audio or transcript text. A user or development environment can redirect process logs, so debug logs should still be reviewed before sharing.

## Changes to this boundary

A change that adds remote processing, telemetry, crash uploads, account data, longer retention, background capture, or interface-content inspection must update this document in the same pull request and receive explicit product review.
