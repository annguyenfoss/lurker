# Lurker Agent Guide

Read these before editing:

1. `DIRECTIVES/LLM/CODEBASE.md`
2. `DIRECTIVES/LLM/DEVELOPMENT.md` for the development, build, test, and validation process
3. `DIRECTIVES/LLM/ENCRYPTION.md` if the task touches create defaults, ciphers, crypto flags, or related docs
4. `README.md` only for current user-facing build/install wording

Non-negotiables:

- Linux, macOS and the four BSDs follow the approved matrix in `DIRECTIVES/LLM/ENCRYPTION.md`. Physical-device encryption setup is in scope on all six OSes where feasible. Lurker currently supports this on Linux.
- First-class products are `lurker` CLI and `lurker-desktop`, both on top of `lurker-core`.
- `lurker-helper` is internal. The desktop app uses it; the desktop app must not shell out to the CLI.
- Use native system facilities for kernel crypto/filesystem operations (`cryptsetup`, formatters, mount tools, BSD base commands, macOS `hdiutil`/`diskutil`). Package third-party encryption engines privately; follow `DIRECTIVES/PLAN/CROSS_PLATFORMS.md`. Never elevate a user-writable private executable. macOS FUSE remains a separately installed prerequisite.
- Do not reintroduce Tauri, Electron, or any WebView stack.
- The desktop app is Slint + winit + software renderer.
- `apps/lurker-desktop/build.rs` intentionally uses Slint `material`; do not switch styles casually.
- Create profiles are hardcoded and opinionated. Do not move them into an external config file.
- If create cipher behavior changes, update `crates/lurker-core/src/workflow.rs`, `DIRECTIVES/LLM/ENCRYPTION.md`, and any user-facing docs together.

Default verification:

- `cargo check --workspace`
- `cargo test --workspace`
- For desktop-only work, at minimum `cargo test -p lurker-desktop`

## README encryption table layout

- Keep the encryption table within GitHub's rendered README width without horizontal scrolling. Avoid oversized fixed-width columns or footer cells that force overflow.
- Keep every OS name on one unbroken line. Do not insert zero-width spaces, soft breaks, or line breaks inside OS headers.
- Render the merged softraid label on exactly two lines: `softraid`, then `CRYPTO / RAID1C`. Keep the entire second line together.
- Verify the rendered GitHub layout, including header wrapping and horizontal overflow; source inspection and cell counts alone are insufficient.
