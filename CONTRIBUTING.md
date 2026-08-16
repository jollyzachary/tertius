# Contributing to Tertius

Tertius is an early cross-platform desktop project. Small, focused changes with a clear operating-system impact are easiest to review.

## Before opening a pull request

1. Search existing issues and pull requests.
2. Open an issue before starting a large feature, new dependency, inference engine, or platform backend.
3. Keep product behavior local-first. Do not add accounts, telemetry, remote transcription, or automatic data upload without prior design discussion.
4. Do not commit speech models, audio, transcripts, signing material, build output, or machine-specific configuration.

## Development setup

Install the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your host, Node.js 20+, and Rust 1.85+.

```bash
git clone https://github.com/jollyzachary/tertius.git
cd tertius/apps/desktop
npm ci
npm run tauri dev
```

The one-time speech-model download happens inside the app and must remain outside the repository.

## Change boundaries

- Put platform-neutral settings, history, model metadata, and cleanup rules in `crates/tertius-core`.
- Keep microphone, shortcut, active-window, clipboard, insertion, tray, and native window code in `apps/desktop/src-tauri`.
- Keep interface and voice-widget behavior in `apps/desktop/src`.
- Prefer a shared implementation with a small, explicit operating-system boundary.
- Do not weaken the content security policy or expand Tauri capabilities without explaining the concrete need.
- Preserve clipboard fallback whenever automatic insertion is unavailable.
- Preserve the rule that raw audio is never written to disk.

## Checks

Run the checks relevant to your change:

```bash
# Repository root
cargo fmt --all -- --check
cargo check --workspace

# apps/desktop
npm run build
```

For native behavior, report the operating system, version, architecture, desktop session where relevant, microphone path, activation mode, and whether insertion and clipboard fallback both worked. Do not claim another platform is verified from compilation alone.

## Pull requests

Keep the title concrete and the diff narrow. In the description, include:

- what changed and why;
- platform impact;
- privacy or permission impact;
- exact checks performed;
- behavior that remains unverified;
- screenshots only when they do not expose transcripts, account names, desktop content, or other personal data.

By contributing, you agree that your contribution is licensed under the repository's MIT License.
