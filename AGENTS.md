# Lurker Agent Guide

Read these before editing:

1. `DIRECTIVES/LLM/CODEBASE.md`
2. `DIRECTIVES/LLM/CIPHER.md` if the task touches create defaults, ciphers, crypto flags, or related docs
3. `README.md` only for current user-facing build/install wording

Non-negotiables:

- Linux only.
- First-class products are `lurker` CLI and `lurker-desktop`, both on top of `lurker-core`.
- `lurker-helper` is internal. The desktop app uses it; the desktop app must not shell out to the CLI.
- Use system binaries for crypto/filesystem work: `cryptsetup`, `veracrypt`, `mkfs.btrfs`, `mount`, `umount`, `lsblk`, optional `blkid`.
- Do not reintroduce Tauri, Electron, or any WebView stack.
- The desktop app is Slint + winit + software renderer.
- `apps/lurker-desktop/build.rs` intentionally uses Slint `material`; do not switch styles casually.
- Create profiles are hardcoded and opinionated. Do not move them into an external config file.
- If create cipher behavior changes, update `crates/lurker-core/src/workflow.rs`, `legacy/bash-cli/lurker`, `DIRECTIVES/LLM/CIPHER.md`, and any user-facing docs together.
- `legacy/` is archival reference, not the active architecture.

Default verification:

- `cargo check --workspace`
- `cargo test --workspace`
- For desktop-only work, at minimum `cargo test -p lurker-desktop`
