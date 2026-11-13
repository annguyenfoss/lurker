# Lurker Codebase Directive

## Product Model

`lurker` is a disk/container and directory encryption utility with two first-class frontends. Native BSD CLI support and the additional selected backends are being qualified alongside Linux/macOS:

- `lurker`: CLI for terminal/scripting use
- `lurker-desktop`: Slint desktop app for simple create/mount/unmount flows

Both frontends share `lurker-core`. Desktop privilege escalation goes through `lurker-helper`.

## Cross-platform Architecture

Before implementing another platform or bundled backend, read `DIRECTIVES/PLAN/CROSS_PLATFORMS.md`, especially its reuse boundaries and KVM validation topology. The core now dispatches to Linux/macOS, native BSD adapters and a shared directory-backend lifecycle. Native packaging and functional qualification remain separate from compilation. The selected matrix records scope, not successful native qualification. Keep unfinished or untested pairs labeled accordingly.

Use one shared core and reusable backend integrations with small OS-specific adapters. Share upstream encryption code and backend policy; isolate native mounts, devices, authorization and discovery. All six Linux distributions use the same Linux integration code, with differences handled through packaging and demonstrated tool capabilities. Do not copy entire workflows per OS or reimplement encryption primitives. The plan defines five primary KVM OS development targets and ten build/functional-test guests; macOS is tested separately on a Mac.

Bundled third-party engines replace a separately installed VeraCrypt requirement as their native package is qualified. System facilities remain responsible for kernel crypto, filesystems and native Apple/BSD formats. Elevated code must execute only administrator-controlled backend paths; development backend overrides are unprivileged. macOS FUSE is installed separately. Build/test jobs use isolated `/tmp/lurker-*` directories on the named remote hosts; endeavour may build code but must not run storage tests or touch its installed Lurker sessions.

## Workspace Map

- `crates/lurker-core`
  - `src/folder.rs`: shared login-user gocryptfs/CryFS/Cryptomator lifecycle, fixed profiles, private backend discovery and identity-checked sessions
  - `src/bsd.rs`, `src/bsd/`: shared native BSD file-container lifecycle with FreeBSD GELI, NetBSD CGD, OpenBSD softraid and DragonFly LUKS1 adapters; PEFS and private VeraCrypt qualification continue
  - `src/api.rs`: shared serde command/response types
  - `src/model.rs`: enums such as `VolumeType`, `CreateCipher`, `SourceKind`
  - `src/credentials.rs`: password/keyfile exclusivity and keyfile validation
  - `src/keyslots.rs`: Linux LUKS keyslot inspection, addition and authenticated removal
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
- `scripts/build-macos-backends.py`: pinned native VeraCrypt/wx build and audited private dependency staging; compatible FUSE remains external
- `scripts/build-folder-backends.py`, `scripts/build-cryptomator.py`: pinned private directory engines and a private Cryptomator Java runtime
- `scripts/remote-test.py`: isolated exact-commit remote jobs with bounded summaries; see `docs/remote-testing.md`
- `scripts/test-backends.py`: CLI conformance using private `/tmp` fixtures, PTY passwords and retained JSON evidence
- `scripts/build-macos-dmg.py`: native release app bundle and drag-to-Applications DMG packaging; helper stays beside the desktop executable inside `Contents/MacOS`
- `scripts/build-linux-userspace.py`: builds pinned, patched VeraCrypt and LKL filesystem helpers without installation or elevation; integration patches live under `native/linux-userspace/patches`
- `docs/linux-userspace.md`: stock VeraCrypt/FUSE distinction, Linux privileged/unprivileged comparison, security findings, implementation lessons, validation evidence and open hardening work

## Core Invariants

- The approved six-OS matrix in `CIPHER.md` controls backend dispatch. Physical-device creation remains Linux-only; BSD native creation uses dedicated file-backed devices. macOS operations are file-backed virtual disks or selected directory backends.
- Container defaults: Linux `luks` (LUKS2), macOS `apple`, FreeBSD `geli`, NetBSD `cgd`, OpenBSD `softraid`, DragonFly `luks1`. Explicit Linux `luks1` preserves its compatible PBKDF2 profile. Directory backends are explicit selections with no size or generic cipher option.
- Apple encryption is AES-256 at the disk-image layer, with APFS inside; it is not system FileVault or APFS filesystem encryption.
- Supported create ciphers: `aes`, `serpent`, `twofish`.
- Create cipher matters only on create. Mount/unmount detect existing volumes.
- Linux create uses `btrfs`; macOS Apple images use APFS and VeraCrypt uses exFAT.
- Linux Btrfs mounts use `compress=zstd`.
- VeraCrypt tags are unsupported.
- Do not implement VeraCrypt authentication requiring a password and keyfile together: losing either required credential can lose access to the data. This decision does not prohibit keyfile-only authentication; see the authentication decision in `CIPHER.md`.
- The desktop app never invokes the installed CLI binary. Check helper capability flags before sending keyfile or keyslot requests so older helpers cannot ignore credential fields.
- The helper is an implementation detail for desktop privilege escalation. Protocol 3 adds directory-backend capability negotiation; directory engines run as the login user even when an older request supplies the default privileged mode. Privileged operations require the protocol-3 trusted-tool capability, including plain password requests. Protocol 2 remains accepted only for compatible login-session credential operations.
- File-backed create uses same-directory temp files, atomic no-clobber publication without force, and atomic rename with force.
- Privileged Linux mounts use cryptsetup-managed `lurker_*` mappings; creation uses the packaged restricted private VeraCrypt creator. Its path and every ancestor must be administrator-controlled before elevated execution.
- Linux native userspace transport 3 requires persistent source-inode locking and rejects older unlocked helper binaries; frontend capability negotiation must enforce it. Drain legacy sessions before component upgrades.
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
- Active volume listing comes from `lurker_core::list_active_volumes()`, which scans `/dev/mapper/lurker_*` and correlates mountinfo on Linux, merged with private directory sessions. BSD native volumes come from their identity-checked registry and native mount tables. On macOS it reads hdiutil plists and VeraCrypt verbose listings, showing the user's attached encrypted disk images and VeraCrypt volumes

## What Lives Where

- Change shared behavior in `lurker-core` first.
- Change CLI syntax/help in `crates/lurker-cli/src/cli.rs`.
- Change desktop validation/form semantics in `apps/lurker-desktop/src/logic.rs` and `ui/app-window.slint`.
- Change desktop helper lookup/spawn behavior in `apps/lurker-desktop/src/helper.rs`.
- Change desktop privilege escalation behavior in `crates/lurker-helper/src/main.rs`.
- Change cipher profiles only after reading `DIRECTIVES/LLM/CIPHER.md`.

## UI Rules

- Keep the desktop app narrow: create, mount, unmount, active volumes and LUKS credentials. Linux Privileged mode exposes the LUKS keys screen; other modes hide it.
- Hide hardcoded expert defaults instead of exposing more knobs.
- Linux offers Privileged/Unprivileged in the main header. Unprivileged selects VeraCrypt files, hides LUKS/devices, and cannot unmount privileged rows. Each row retains its own backend mode across selector changes.
- Keep the custom Slint controls unless you re-verify text zoom behavior.
- `apps/lurker-desktop/build.rs` intentionally uses Slint `material`; `fluent` previously broke runtime text zoom/layout in this app.

## Sync Rules

When changing create cipher profiles or create-time crypto defaults, update all of:

- `crates/lurker-core/src/workflow.rs`
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
- Prefer the packaged native VeraCrypt backend under `Contents/Helpers` or the private CLI layout; installed-app discovery is a compatibility fallback. Its qualified external FUSE runtime is required. FUSE-T and macFUSE are not interchangeable without native workload tests.
- Passphrases travel over stdin only. hdiutil uses a NUL terminator; VeraCrypt uses a newline. Reject embedded NUL/newline characters and preserve spaces.
- The desktop still invokes lurker-helper, never the CLI; macOS helper requests run in the login user session without Linux escalation.
- Reject LUKS and physical devices on macOS, and reject Apple images on Linux with platform-specific errors.
- Mac create stages in a private same-directory folder, publishes atomically, and refuses to overwrite attached images.
- macOS active volume listing includes attached encrypted images belonging to the current user and VeraCrypt volumes, including those mounted outside Lurker. Unmount is always scoped to one identified encrypted container.
- Run the opt-in native suite with `cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1`; it creates only disposable file containers.
