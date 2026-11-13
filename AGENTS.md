# Lurker Agent Guide

Read these before editing:

1. `DIRECTIVES/LLM/CODEBASE.md`
2. `DIRECTIVES/LLM/CIPHER.md` if the task touches create defaults, ciphers, crypto flags, or related docs
3. `README.md` only for current user-facing build/install wording

Non-negotiables:

- Linux, macOS and the four BSDs follow the approved matrix in `DIRECTIVES/LLM/CIPHER.md`. Physical-device create remains Linux-only; native BSD adapters initially support file-backed containers.
- First-class products are `lurker` CLI and `lurker-desktop`, both on top of `lurker-core`.
- `lurker-helper` is internal. The desktop app uses it; the desktop app must not shell out to the CLI.
- Use native system facilities for kernel crypto/filesystem operations (`cryptsetup`, formatters, mount tools, BSD base commands, macOS `hdiutil`/`diskutil`). Package third-party encryption engines privately; follow `DIRECTIVES/PLAN/CROSS_PLATFORMS.md`. Never elevate a user-writable private executable. macOS FUSE remains a separately installed prerequisite.
- Do not reintroduce Tauri, Electron, or any WebView stack.
- The desktop app is Slint + winit + software renderer.
- `apps/lurker-desktop/build.rs` intentionally uses Slint `material`; do not switch styles casually.
- Create profiles are hardcoded and opinionated. Do not move them into an external config file.
- If create cipher behavior changes, update `crates/lurker-core/src/workflow.rs`, `DIRECTIVES/LLM/CIPHER.md`, and any user-facing docs together.

Default verification:

- `cargo check --workspace`
- `cargo test --workspace`
- For desktop-only work, at minimum `cargo test -p lurker-desktop`
