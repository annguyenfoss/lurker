# Lurker Codebase Directive

## Product Model

`lurker` is a Linux encryption utility with two first-class frontends:

- `lurker`: CLI for terminal/scripting use
- `lurker-desktop`: Slint desktop app for simple create/mount/unmount flows

Both frontends share `lurker-core`. Desktop privilege escalation goes through `lurker-helper`.

## Workspace Map

- `crates/lurker-core`
  - `src/api.rs`: shared serde command/response types
  - `src/model.rs`: enums such as `VolumeType`, `CreateCipher`, `SourceKind`
  - `src/workflow.rs`: source of truth for create/mount/unmount behavior and hardcoded crypto profiles
  - `src/linux.rs`: mapper naming, path resolution, mountinfo parsing, target detection
  - `src/system.rs`: tool discovery, subprocess execution, privilege-aware context
  - `src/output.rs`: structured human-facing output
- `crates/lurker-cli`
  - `src/cli.rs`: manual argv parsing and help rendering
  - `src/main.rs`: parse -> run core
- `crates/lurker-helper`
  - `src/main.rs`: desktop helper; reads `CommandAction` JSON on stdin and writes `OperationResponse` JSON on stdout
  - reruns itself via `pkexec` first, `sudo` fallback only when a TTY exists
- `apps/lurker-desktop`
  - `src/main.rs`: UI callbacks, zoom state, background threading, refresh flow
  - `src/logic.rs`: form validation and conversion into shared commands
  - `src/helper.rs`: helper discovery and process launch
  - `ui/app-window.slint`: main window
  - `ui/controls.slint`: custom button/select/checkbox controls
  - `ui/theme.slint`: layout tokens
  - `ui/zoom-metrics.slint`: regression surface for text zoom tests
- `legacy/bash-cli`
  - archived Bash implementation; still kept in sync for create cipher profiles
- `legacy/rust-cli-monolith`
  - historical only; not part of the active architecture

## Core Invariants

- Linux only.
- Supported create volume types: `luks`, `veracrypt`.
- Supported create ciphers: `aes`, `serpent`, `twofish`.
- Create cipher matters only on create. Mount/unmount detect existing volumes.
- Filesystem created after successful create is always `btrfs`.
- Btrfs mounts use `compress=zstd`.
- VeraCrypt tags are unsupported.
- The desktop app never invokes the installed CLI binary.
- The helper is an implementation detail for desktop privilege escalation.
- File-backed create currently uses same-directory temp files and atomic rename.
- Block-device create is destructive and requires force.

## Request Flow

- CLI: parse args -> build `CommandAction` -> `lurker_core::run()`
- Desktop: gather form state -> `apps/lurker-desktop/src/logic.rs` builds shared command -> `lurker-helper` runs core -> desktop refreshes active volumes
- Active volume listing comes from `lurker_core::list_active_volumes()`, which scans `/dev/mapper/lurker_*` and correlates mountinfo

## What Lives Where

- Change shared behavior in `lurker-core` first.
- Change CLI syntax/help in `crates/lurker-cli/src/cli.rs`.
- Change desktop validation/form semantics in `apps/lurker-desktop/src/logic.rs` and `ui/app-window.slint`.
- Change desktop helper lookup/spawn behavior in `apps/lurker-desktop/src/helper.rs`.
- Change desktop privilege escalation behavior in `crates/lurker-helper/src/main.rs`.
- Change cipher profiles only after reading `DIRECTIVES/LLM/CIPHER.md`.

## UI Rules

- Keep the desktop app narrow: create, mount, unmount, active volumes.
- Hide hardcoded expert defaults instead of exposing more knobs.
- Keep the custom Slint controls unless you re-verify text zoom behavior.
- `apps/lurker-desktop/build.rs` intentionally uses Slint `material`; `fluent` previously broke runtime text zoom/layout in this app.

## Sync Rules

When changing create cipher profiles or create-time crypto defaults, update all of:

- `crates/lurker-core/src/workflow.rs`
- `legacy/bash-cli/lurker`
- `DIRECTIVES/LLM/CIPHER.md`
- `README.md` if user-visible behavior changed

If you add or remove a supported create cipher, also update:

- `crates/lurker-core/src/model.rs`
- `crates/lurker-cli/src/cli.rs`
- `apps/lurker-desktop/src/logic.rs`
- `apps/lurker-desktop/ui/app-window.slint`

## Verification

- `cargo check --workspace`
- `cargo test --workspace`
- For desktop-only changes, also run `cargo test -p lurker-desktop`
