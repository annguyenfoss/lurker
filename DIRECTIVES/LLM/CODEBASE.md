# Lurker Codebase Directive

## Product Model

`lurker` is a disk/container and directory encryption utility with two first-class frontends. The CLI also handles encryption and decryption of individual files. Native BSD CLI support and the additional selected backends are being qualified alongside Linux/macOS:

- `lurker`: CLI for terminal/scripting use
- `lurker-desktop`: Slint desktop app for simple create/mount/unmount flows

Both frontends share `lurker-core`. Desktop privilege escalation goes through `lurker-helper`.

## Cross-platform Architecture

Before implementing another platform or bundled backend, read `DIRECTIVES/PLAN/CROSS_PLATFORMS.md`, especially its reuse boundaries and KVM validation topology. The core now dispatches to Linux/macOS, native BSD adapters and a shared directory-backend lifecycle. Native packaging and functional qualification remain separate from compilation. The selected matrix records scope, not successful native qualification. Keep unfinished or untested pairs labeled accordingly.

Use one shared core and reusable backend integrations with small OS-specific adapters. Share upstream encryption code and backend policy; isolate native mounts, devices, authorization and discovery. All six Linux distributions use the same Linux integration code, with differences handled through packaging and demonstrated tool capabilities. Do not copy entire workflows per OS or reimplement encryption primitives. The plan defines five primary KVM OS development targets and ten build/functional-test guests; macOS is tested separately on a Mac.

Bundled third-party engines replace a separately installed VeraCrypt requirement as their native package is qualified. System facilities remain responsible for kernel crypto, filesystems and native Apple/BSD formats. Elevated code must execute only administrator-controlled backend paths; development backend overrides are unprivileged. macOS FUSE is installed separately.

Follow `DIRECTIVES/LLM/DEVELOPMENT.md` for implementation, batched builds and tests, and validation. The workstation is the editor and coordinator. Use isolated `/tmp/lurker-*` directories on the named remote hosts, build remotely by default, and run all tests remotely (macOS on Buran). Never touch installed Lurker sessions on the workstation.

## Workspace Map

- `crates/lurker-core`
  - `src/file_crypto/`: login-user single-file age, GnuPG, and OpenSSL CMS workflows with private output staging
  - `src/folder.rs`: shared gocryptfs/CryFS/Cryptomator lifecycle, fixed profiles, private backend discovery and identity-checked sessions. Directory I/O runs as the login user; `folder/openbsd.rs` supplies authorized native CryFS mounting, held-directory confinement and caller-owned publication on OpenBSD.
  - `src/bsd.rs`, `src/bsd/`: shared BSD file-container lifecycle with FreeBSD GELI and PEFS, NetBSD CGD, OpenBSD CRYPTO and RAID1C, and DragonFly LUKS1. FreeBSD and OpenBSD use the private VeraCrypt engine. DragonFly VeraCrypt uses the private `tcplay` wrapper and native device mapper.
  - `src/bsd/physical.rs`, `src/bsd/physical/`: physical-device identity and lifecycle using the same native session journal. FreeBSD GELI/VeraCrypt validate GEOM topology, GPT identity and disk serial; NetBSD CGD validates GPT wedge geometry/UUID and keeps mandatory recovery parameters under `/var/db/lurker-cgd/`. OpenBSD CRYPTO/RAID1C use checked whole disks, while VeraCrypt uses checked data partitions. DragonFly LUKS1/VeraCrypt use native GPT partition geometry and storage UUIDs. Physical providers are never treated as owned vnode leases. Final native package/matrix qualification remains separate from adapter implementation.
  - `src/api.rs`: shared serde command/response types
  - `src/model.rs`: enums such as `VolumeType`, `CreateCipher`, `SourceKind`
  - `src/credentials.rs`: password/keyfile exclusivity and keyfile validation
  - `src/keyslots.rs`: Linux LUKS keyslot inspection, addition and authenticated removal
  - `src/workflow.rs`: source of truth for hardcoded crypto profiles and Linux workflow dispatch
  - `src/macos.rs`: hdiutil/VeraCrypt file-container workflows and native volume discovery
  - `src/macos/apfs.rs`: diskutil APFS encrypted data-volume lifecycle for image files and external partitions
  - `src/linux_workflow.rs`: Linux create/mount/unmount implementation
  - `src/userspace.rs`: Linux unprivileged VeraCrypt file workflow and private session registry
  - `src/size.rs`: shared size validation
  - `src/linux.rs`: mapper naming, path resolution, mountinfo parsing, target detection
  - `src/system.rs`: tool discovery, subprocess execution, privilege-aware context
  - `src/output.rs`: structured human-facing output
- `crates/lurker-cli`
  - `src/file_cli.rs`: single-file encryption and decryption arguments and CLI dispatch
  - `src/cli.rs`: manual argv parsing and help rendering
  - `src/main.rs`: parse -> run core
- `crates/lurker-helper`
  - `src/main.rs`: desktop helper; reads `CommandAction` JSON on stdin and writes `OperationResponse` JSON on stdout
  - Privileged Linux/BSD requests rerun through a trusted helper via `pkexec` first, then `sudo`; without a TTY, `sudo -n` uses existing authorization without prompting. Login-user directory engines and macOS operations run directly.
  - BSD `list-native` uses the same authorization boundary to expose the native session namespace without making its root-owned registry readable. OpenBSD includes that caller's authorized CryFS records; other login-user directory rows are merged separately by the desktop.
- `apps/lurker-desktop`
  - `src/main.rs`: UI callbacks, zoom state, background threading, refresh flow
  - `src/logic.rs`: form validation and conversion into shared commands
  - `src/helper.rs`: helper discovery and process launch
  - `ui/app-window.slint`: main window
  - `ui/controls.slint`: custom button/select/checkbox controls
  - `ui/theme.slint`: layout tokens
  - `ui/zoom-metrics.slint`: regression surface for text zoom tests
- `scripts/build-dragonfly-tcplay.py`: verifies and builds the pinned DragonFly userspace `tcplay` sources, then stages the private wrapper and notices for `scripts/stage-unix.py`.
- `scripts/build-macos-backends.py`: pinned native VeraCrypt/wx build and audited private dependency staging; compatible FUSE remains external
- `scripts/build-folder-backends.py`, `scripts/build-cryptomator.py`: pinned private directory engines and a private Cryptomator Java runtime
- `scripts/remote-test.py`: isolated exact-commit remote jobs with bounded summaries; see `docs/remote-testing.md`
- `scripts/test-backends.py`: CLI conformance using private `/tmp` fixtures, PTY passwords and retained JSON evidence
- `scripts/build-macos-dmg.py`: native release app bundle and drag-to-Applications DMG packaging; helper stays beside the desktop executable inside `Contents/MacOS`
- `scripts/build-linux-userspace.py`: builds only the private VeraCrypt userspace transport and creator from verified sources, without installation or elevation. `native/linux-userspace/build-host-filesystem.py` separately builds the administrator-installed LKL filesystem prerequisite, applying the pinned Linux stable and integration patches. Filesystem drivers and FUSE stay outside the application bundle.
- `docs/linux-userspace.md`: stock VeraCrypt/FUSE distinction, Linux privileged/unprivileged comparison, security findings, implementation lessons, validation evidence and open hardening work

## Core Invariants

- Filesystem facilities and drivers are host prerequisites, not bundled Lurker
  components. Discover actual OS capabilities and wrap ordinary native format,
  mount and dismount operations. Bundle independent eligible encryption engines
  privately; install OS-coupled crypto facilities and FUSE on the host. The fresh
  qualification contract is `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/BRIEF.md`;
  paid-only/trial routes and VM/network-filesystem compatibility workarounds do
  not satisfy its native filesystem acceptance.

- **The product goal is Create + Read/Write for every targeted OS/filesystem pair.** Cannot create, cannot mount, or can only read means incomplete support, not a desired feature or a successful Create + Read/Write result.
- **There is one mount action and one unmount action.** Do not add a user-selectable read-only/read-write mode, CLI flag, API option, desktop toggle, or separate read-only feature/test requirement. Mount uses the best available filesystem/driver capability and must use read/write when available. Read-only access is a capability limitation to report explicitly; never choose it when an eligible writable route can satisfy the request.
- Qualify filesystem support with the ordinary create, mount, write, normal close, reopen, verify, append and reopen flow. Research establishes the expected filesystem/driver capabilities; a successful representative Lurker flow with no discovered blocker can fulfill that expectation without an exhaustive filesystem-feature audit. Record every shortcoming against Create + Read/Write, including any historical promise limited to imports or reading.
- The matrix in `ENCRYPTION.md` defines approved backend scope and current support. Physical-device setup is in scope on Linux, macOS, and the four BSDs where feasible. Implemented device routes include Linux, macOS external APFS data partitions, FreeBSD GELI, NetBSD CGD and OpenBSD CRYPTO; shared FreeBSD/OpenBSD VeraCrypt device integration has landed and native qualification continues. BSD physical providers use the existing identity-checked session journal without vnode detach ownership. Exact container, qcow2 and Toshiba USB results, remaining backends and frontend qualification are recorded in `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/`.
- Container defaults: Linux `luks` (LUKS2), macOS `apple`, FreeBSD `geli`, NetBSD `cgd`, OpenBSD `softraid`, DragonFly `luks1`. Explicit Linux `luks1` preserves its compatible PBKDF2 profile. Directory backends are explicit selections with no size or generic cipher option.
- Apple image encryption is fixed AES-256 at the disk-image layer, with APFS by default and native host-tool routes for explicit exFAT/ext4/NTFS/ZFS. Its profiles remain in `workflow.rs`; it is distinct from FileVault and native APFS-volume encryption.
- Supported create ciphers: `aes`, `serpent`, `twofish`.
- Create cipher matters only on create. Mount/unmount detect existing volumes.
- Linux create defaults to `btrfs`; macOS Apple images default to APFS and VeraCrypt to exFAT. CLI file-container creation can select an alternate inner filesystem with `--filesystem` where its native adapter and installed tools support it. Native APFS encryption remains APFS-only; directory formats have no inner-filesystem selector.
- Linux Btrfs mounts use `compress=zstd`.
- OpenBSD mounts writable FFS synchronously on CRYPTO, RAID1C, and VeraCrypt. Filesystem repair remains a manual native operation.
- VeraCrypt tags are unsupported.
- Do not implement VeraCrypt authentication requiring a password and keyfile together: losing either required credential can lose access to the data. This decision does not prohibit keyfile-only authentication; see the authentication decision in `ENCRYPTION.md`.
- The desktop app never invokes the installed CLI binary. Check helper capability flags before sending keyfile or keyslot requests so older helpers cannot ignore credential fields.
- The helper is an implementation detail for desktop privilege escalation. Protocol 3 adds directory-backend capability negotiation; directory engines normally run as the login user even when an older request supplies the default privileged mode. PEFS and OpenBSD CryFS use native authorization. OpenBSD CryFS confines vault/state I/O to the invoking account, regains root only for native FUSE setup, and permanently drops privilege before filesystem initialization. Its supervisor and per-caller session registry remain root controlled. Privileged operations require the protocol-3 trusted-tool capability, including plain password requests. Protocol 2 remains accepted only for compatible login-session credential operations. Additive capabilities require supervised directory lifecycle (`folder_supervision_protocol: 1`), owned Linux loop leases (`linux_loop_lease_protocol: 1`), OpenBSD mirror members (`raid1c_members`), authorized BSD native discovery (`native_volume_listing: 1`), and macOS Cryptomator underlay fencing (`cryptomator_fence_protocol: 1`). The Cryptomator fence contract also gates every macOS Auto request so source/session detection cannot bypass an older helper's missing protection. Older helpers must fail before receiving unsupported requests.
- File-backed create uses same-directory temp files, atomic no-clobber publication without force, and atomic rename with force.
- Privileged Linux mounts use cryptsetup-managed `lurker_*` mappings; creation uses the packaged restricted private VeraCrypt creator. Its path and every ancestor must be administrator-controlled before elevated execution.
- Linux userspace contract 5 pairs private crypto transport 4 (persistent source-inode locks and backing-file barriers) with host filesystem protocol 5 (sticky I/O failure and explicit shutdown acknowledgements). New create/mount requests must negotiate contract 5. A known failure or malformed shutdown record retains the encrypted image and session; automatic journal recovery is only for an absent acknowledgement after a crash. Owned protocol-4 sessions can still close without installed backend executables. Drain legacy sessions before component upgrades.
- Unprivileged Linux supports VeraCrypt regular files, with the same cipher profiles and Btrfs/zstd default. It never invokes sudo, pkexec, mount, losetup, or device-mapper. No fallback to elevation is permitted.
- The unprivileged crypto helper exposes a FUSE image; the filesystem helper runs Linux filesystem code inside LKL. Both run as the login user. Standard user FUSE mounting uses the system's existing fusermount3 helper.
- Linux helper protocol support and installed prerequisites are separate. `capabilities --userspace-readiness` reports `userspace_backend_ready` and its error only for a login-user query; ordinary or administrator capability queries do not execute configured probes. The desktop checks explicit readiness failures before sending create/mount secrets. Missing backend executables do not block closing an owned session through its existing processes and fusermount.
- The helper JSON command and active-volume records carry a serde-defaulted `privilege_mode`; missing mode retains the original privileged behavior. macOS hides the selector and uses its native user-session route, including APFS encrypted files and external data partitions.
- Userspace state belongs to the current uid in a private runtime directory. Record source device/inode, mount IDs, and process start time; never close a replaced mount or reused process. Drain the filesystem before closing its crypto image. Preserve recoverable sessions on busy/failed unmounts.
- Probe LUKS metadata before open; authenticate VeraCrypt during open instead of probing encrypted headers without a password.
- Register mapper cleanup only after a successful open, so failed duplicate operations cannot close an existing mapping.
- Keep observed read-only state and native write-protection checks for capability reporting, safe teardown and detection of an unexpected writable-mount downgrade. These internal checks do not define a user-selectable read-only mode.
- Block-device create is destructive and requires force.

## Request Flow

- CLI: parse args -> build `CommandAction` -> `lurker_core::run()`
- Desktop: gather form state -> `apps/lurker-desktop/src/logic.rs` builds shared command -> `lurker-helper` runs core -> desktop refreshes active volumes
- Active volume listing comes from `lurker_core::list_active_volumes()`, which scans `/dev/mapper/lurker_*` and correlates mountinfo on Linux, merged with private directory sessions. BSD native volumes come from their identity-checked registry and native mount tables. The ordinary-user desktop obtains those private native records through authorized `lurker-helper list-native`, separately from its own folder sessions; root's unrelated folder registry is excluded. On macOS it reads hdiutil plists and VeraCrypt verbose listings, showing the user's attached encrypted disk images and VeraCrypt volumes

## What Lives Where

- Change shared behavior in `lurker-core` first.
- Change CLI syntax/help in `crates/lurker-cli/src/cli.rs`.
- Change desktop validation/form semantics in `apps/lurker-desktop/src/logic.rs` and `ui/app-window.slint`.
- Change desktop helper lookup/spawn behavior in `apps/lurker-desktop/src/helper.rs`.
- Change desktop privilege escalation behavior in `crates/lurker-helper/src/main.rs`.
- Change cipher profiles only after reading `DIRECTIVES/LLM/ENCRYPTION.md`.

## UI Rules

- Keep the desktop app narrow: create, mount, unmount, active volumes and LUKS credentials. Linux Privileged mode exposes the LUKS keys screen; other modes hide it.
- Hide hardcoded expert defaults instead of exposing more knobs.
- Derive create choices from the core's implemented capabilities and use the native OS default. Directory targets hide size and generic cipher fields; fixed native profiles hide alternate ciphers. Show keyfiles and physical-device creation only where the backend supports them.
- Linux offers Privileged/Unprivileged in the main header. Unprivileged offers VeraCrypt files and portable directory engines, hides LUKS/devices, and cannot unmount privileged rows. Each row retains its own backend mode across selector changes. BSD/macOS hide the Linux selector and keyslot screen.
- Keep the custom Slint controls unless you re-verify text zoom behavior.
- `apps/lurker-desktop/build.rs` intentionally uses Slint `material`; `fluent` previously broke runtime text zoom/layout in this app.

## Sync Rules

When changing create cipher profiles or create-time crypto defaults, update all of:

- `crates/lurker-core/src/workflow.rs`
- `DIRECTIVES/LLM/ENCRYPTION.md`
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
- Prefer the packaged native VeraCrypt backend under `Contents/Helpers` or the private CLI layout. The default VeraCrypt build uses FUSE-T SMB loopback; that transport is excluded from native-only qualification. An explicit audited `--fuse-runtime macfuse` build uses native macFUSE without changing the default build policy. Use local FUSE-T FSKit for gocryptfs, CryFS and Cryptomator. gocryptfs uses the qualified FUSE-T 1.2.9 helper. Optional inner-filesystem adapters may additionally require separately installed macFUSE or native filesystem extensions; they do not replace the encryption transport. Diagnose each missing prerequisite explicitly.
- Passphrases travel over stdin only. hdiutil uses a NUL terminator; VeraCrypt uses a newline. Reject embedded NUL/newline characters and preserve spaces.
- The desktop still invokes lurker-helper, never the CLI; macOS helper requests run in the login user session without Linux escalation.
- Reject LUKS on macOS and Apple images on Linux with platform-specific errors. The macOS create routes accept files, directories, native APFS external partitions and qualified VeraCrypt external GPT data partitions. Physical VeraCrypt requires an administrator-controlled private engine before its native authorization service can run; bind native/GPT/hardware identities and refuse internal/system/whole/mounted/reserved targets. Its initial physical inner filesystem is exFAT; file-only foreign-filesystem adapters remain separate. See `native/macos/veracrypt-partitions.md`. Native APFS data-volume setup uses `/usr/sbin/diskutil`, with `/usr/bin/fdesetup` for supported startup-volume FileVault management.
- Mac create stages in a private same-directory folder, publishes atomically, and refuses to overwrite attached images. Apple ext4/NTFS/ZFS creation wraps an identified zero-only native virtual disk using the fixed `workflow.rs` AES-256 profile, then formats the authenticated device. Filesystem sessions journal Apple-versus-VeraCrypt crypto ownership and image/device/mount identities before writable operations. Close the filesystem or export its recorded sole-vdev ZFS pool before normal hdiutil detach. `native/macos/apple-filesystems.md` documents host prerequisites and unsupported transports.
- macOS active volume listing includes attached encrypted images belonging to the current user and VeraCrypt volumes, including those mounted outside Lurker. Unmount is always scoped to one identified encrypted container.
- Run the opt-in native suite with `cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1`; it creates only disposable file containers.
