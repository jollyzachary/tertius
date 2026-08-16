# Tertius agent instructions

Read `README.md`, `docs/ARCHITECTURE.md`, and `docs/PRIVACY.md` before making changes.

## Product contracts

- Tertius is a local-first desktop dictation tool for macOS, Windows, and Linux.
- Raw audio stays in memory and is never written to disk.
- Routine transcription does not use a network service.
- Every finished dictation is copied to the clipboard.
- Automatic insertion must degrade to clipboard-only behavior when input injection is unavailable.
- Finished transcript history stays local and is pruned after three days.
- The floating widget must not steal focus from the field that should receive dictation.
- Closing the main window hides it while the tray process and shortcut continue running.

Do not remove or weaken these contracts unless the requested change explicitly changes the product definition.

## Privacy and security

- Never add telemetry, crash upload, cloud transcription, account data, background microphone capture, or interface-content inspection without explicit product approval and a matching update to `docs/PRIVACY.md`.
- Never log transcript text, captured audio, credentials, signing details, or full personal paths.
- Never commit downloaded models, audio, transcripts, application data, build output, environment files, certificates, or signing keys.
- Keep the model byte count and SHA-256 verification intact.
- Keep archive extraction restricted to the expected model file allowlist.
- Treat new Tauri capabilities, content-security-policy changes, shell execution, and network endpoints as security-sensitive.

## Implementation boundaries

- Put platform-neutral settings, history, model metadata, and cleanup rules in `crates/tertius-core`.
- Put native audio, shortcut, inference, insertion, permission, tray, and window behavior in `apps/desktop/src-tauri`.
- Put React interface and overlay behavior in `apps/desktop/src`.
- Prefer the smallest shared implementation with explicit operating-system branches where native behavior differs.
- Inspect existing dependencies and types before adding a package or abstraction.

## Platform claims

Compilation is not runtime proof. State the exact host, architecture, desktop session, activation path, microphone result, insertion result, and clipboard result for any native behavior you claim is verified.

The macOS Apple silicon app is the supported reference build. Windows, macOS Intel, and Linux are experimental targets. X11 is the Linux baseline; Wayland behavior varies by compositor.
