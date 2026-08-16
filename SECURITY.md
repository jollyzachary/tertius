# Security policy

## Supported versions

Tertius is currently a pre-1.0 public preview. Security fixes target the latest code on the default branch.

## Reporting a vulnerability

Do not publish security or privacy vulnerabilities in a GitHub issue, discussion, screenshot, or transcript.

Use GitHub's [private vulnerability reporting form](https://github.com/jollyzachary/tertius/security/advisories/new) for this repository.

If the private form is unavailable, open a public issue that asks the maintainer to establish private contact. Do not include exploit details, user data, credentials, transcripts, or audio in that issue.

Include only what is needed to reproduce and assess the problem:

- affected operating system and Tertius version or commit;
- the permission and application state required;
- a minimal reproduction using synthetic data;
- expected and observed behavior;
- security or privacy impact;
- whether the issue affects microphone capture, model installation, local history, clipboard behavior, input injection, or native window handling.

Never send real dictations, private documents, account data, signing credentials, or downloaded model files.

## Scope

High-priority reports include:

- raw audio or transcript data leaving the machine unexpectedly;
- history persisting beyond the documented retention boundary;
- path traversal or unsafe archive extraction during model installation;
- model-integrity verification bypass;
- command execution or privilege escalation;
- unintended capture outside an active dictation;
- clipboard or automatic-insertion behavior that targets the wrong application;
- Tauri capability or content-security-policy bypass.

Expected operating-system permission prompts and the documented clipboard fallback are not vulnerabilities by themselves.

## Disclosure

Please allow time to reproduce, patch, and prepare a coordinated disclosure. Do not test against another person's device or data.
