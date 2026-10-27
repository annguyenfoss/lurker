# Lurker Codebase Directive

## Product Model

`lurker` is a Linux and macOS file-container encryption utility with two first-class frontends:

- `lurker`: CLI for terminal/scripting use
- `lurker-desktop`: Slint desktop app for simple create/mount/unmount flows

Both frontends share `lurker-core`. Desktop privilege escalation goes through `lurker-helper`.

## Workspace Map

- `crates/lurker-core`
  - `src/api.rs`: shared serde command/response types
  - `src/model.rs`: enums such as `VolumeType`, `CreateCipher`, `SourceKind`
  - `src/workflow.rs`: source of truth for hardcoded crypto profiles and Linux workflow dispatch
  - `src/macos.rs`: hdiutil/VeraCrypt file-container workflows and native volume discovery
  - `src/linux_workflow.rs`: Linux create/mount/unmount implementation
  - `src/userspace.rs`: Linux unprivileged VeraCrypt file workflow and private session registry
  - `src/size.rs`: shared size validation
  - `src/linux.rs`: mapper naming, path resolution, mountinfo parsing, target detection
  - `src/system.rs`: tool discovery, subprocess execution, privilege-aware context
  - `src/output.rs`: structured human-facing output
- `crates/lurker-cli`
  - `src/cli.rs`: manual argv parsing and help rendering
  - `src/main.rs`: parse -> run core
- `crates/lurker-helper`
  - `src/main.rs`: desktop helper; reads `CommandAction` JSON on stdin and writes `OperationResponse` JSON on stdout
  - Privileged Linux requests rerun via `pkexec` first, `sudo` fallback only when a TTY exists; unprivileged requests run directly; macOS runs directly as the login user
- `apps/lurker-desktop`
  - `src/main.rs`: UI callbacks, zoom state, background threading, refresh flow
  - `src/logic.rs`: form validation and conversion into shared commands
  - `src/helper.rs`: helper discovery and process launch
  - `ui/app-window.slint`: main window
  - `ui/controls.slint`: custom button/select/checkbox controls
  - `ui/theme.slint`: layout tokens
  - `ui/zoom-metrics.slint`: regression surface for text zoom tests
- `scripts/build-macos-dmg.py`: native release app bundle and drag-to-Applications DMG packaging; helper stays beside the desktop executable inside `Contents/MacOS`
- `scripts/build-linux-userspace.py`: builds pinned, patched VeraCrypt and LKL filesystem helpers without installation or elevation; integration patches live under `native/linux-userspace/patches`
- `docs/linux-userspace.md`: stock VeraCrypt/FUSE distinction, Linux privileged/unprivileged comparison, security findings, implementation lessons, validation evidence and open hardening work
- `legacy/bash-cli`
  - archived Bash implementation; still kept in sync for create cipher profiles
- `legacy/rust-cli-monolith`
  - historical only; not part of the active architecture

## Core Invariants

- Linux and macOS only. macOS operations are file-backed virtual disks, never physical-device encryption.
- Linux create types: `luks` (default), `veracrypt`. macOS: `apple` (default), `veracrypt`.
- Apple encryption is AES-256 at the disk-image layer, with APFS inside; it is not system FileVault or APFS filesystem encryption.
- Supported create ciphers: `aes`, `serpent`, `twofish`.
- Create cipher matters only on create. Mount/unmount detect existing volumes.
- Linux create uses `btrfs`; macOS Apple images use APFS and VeraCrypt uses exFAT.
- Linux Btrfs mounts use `compress=zstd`.
- VeraCrypt tags are unsupported.
- The desktop app never invokes the installed CLI binary.
- The helper is an implementation detail for desktop privilege escalation.
- File-backed create uses same-directory temp files, atomic no-clobber publication without force, and atomic rename with force.
- Privileged Linux mounts use cryptsetup-managed `lurker_*` mappings; the installed VeraCrypt application is required only for creation on that route.
- Unprivileged Linux supports VeraCrypt regular files, with the same cipher profiles and Btrfs/zstd default. It never invokes sudo, pkexec, mount, losetup, or device-mapper. No fallback to elevation is permitted.
- The unprivileged crypto helper exposes a FUSE image; the filesystem helper runs Linux filesystem code inside LKL. Both run as the login user. Standard user FUSE mounting uses the system's existing fusermount3 helper.
- The helper JSON command and active-volume records carry a serde-defaulted `privilege_mode`; missing mode retains the original privileged behavior. macOS hides the selector and keeps its existing native user-session route.
- Userspace state belongs to the current uid in a private runtime directory. Record source device/inode, mount IDs, and process start time; never close a replaced mount or reused process. Drain the filesystem before closing its crypto image. Preserve recoverable sessions on busy/failed unmounts.
- Probe LUKS metadata before open; authenticate VeraCrypt during open instead of probing encrypted headers without a password.
- Register mapper cleanup only after a successful open, so failed duplicate operations cannot close an existing mapping.
- Readonly applies to both the crypto mapping and the filesystem mount.
- Block-device create is destructive and requires force.

## Request Flow

- CLI: parse args -> build `CommandAction` -> `lurker_core::run()`
- Desktop: gather form state -> `apps/lurker-desktop/src/logic.rs` builds shared command -> `lurker-helper` runs core -> desktop refreshes active volumes
- Active volume listing comes from `lurker_core::list_active_volumes()`, which scans `/dev/mapper/lurker_*` and correlates mountinfo on Linux. On macOS it reads hdiutil plists and VeraCrypt verbose listings, showing the user's attached encrypted disk images and VeraCrypt volumes

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
- Linux offers Privileged/Unprivileged in the main header. Unprivileged selects VeraCrypt files, hides LUKS/devices, and cannot unmount privileged rows. Each row retains its own backend mode across selector changes.
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
- On Linux, build the optional native helpers, then run `scripts/test-linux-userspace.sh` as a normal user for the real userspace cipher/filesystem and recovery suites. Do not run this suite with sudo.
- For desktop-only changes, also run `cargo test -p lurker-desktop`

## macOS Rules

- Use `/usr/bin/hdiutil` for Apple disk-image encryption, attach, detach, and structured discovery.
- VeraCrypt unmount must first use `/usr/sbin/diskutil unmount` to reject busy filesystems; some VeraCrypt builds force-detach internally.
- The existing Generate button must use system randomness, with no fixed example password fallback.
- Use the installed VeraCrypt application executable (also discoverable on PATH); its matching FUSE-T/macFUSE runtime is required.
- Passphrases travel over stdin only. hdiutil uses a NUL terminator; VeraCrypt uses a newline. Reject embedded NUL/newline characters and preserve spaces.
- The desktop still invokes lurker-helper, never the CLI; macOS helper requests run in the login user session without Linux escalation.
- Reject LUKS and physical devices on macOS, and reject Apple images on Linux with platform-specific errors.
- Mac create stages in a private same-directory folder, publishes atomically, and refuses to overwrite attached images.
- macOS active volume listing includes attached encrypted images belonging to the current user and VeraCrypt volumes, including those mounted outside Lurker. Unmount is always scoped to one identified encrypted container.
- Run the opt-in native suite with `cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1`; it creates only disposable file containers.
