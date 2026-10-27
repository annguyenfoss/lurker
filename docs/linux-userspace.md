# Linux VeraCrypt: privileged and unprivileged implementations

Findings recorded on 2026-10-23 against the source revisions below. This document distinguishes stock VeraCrypt from Lurker’s two Linux routes. Functional verification and architectural security conclusions are recorded separately.

The desktop header selects Privileged or Unprivileged. The CLI uses `--unprivileged`; missing JSON/CLI mode retains the original privileged behavior. The selector is hidden on macOS. Desktop still calls `lurker-helper`, never the CLI. An unprivileged request runs directly as the login user and cannot fall back to elevation.

## Stock VeraCrypt already supports FUSE

Stock VeraCrypt supports FUSE3 through the build option `WITHFUSE3=1`; the checked upstream Makefile otherwise selects FUSE2 on Linux. FUSE support alone does not make its complete Linux mounting operation unprivileged.

In stock VeraCrypt’s userspace-crypto path, the flow is:

```text
encrypted container -> VeraCrypt decrypted FUSE image -> loop device -> host filesystem mount
```

Loop setup and mounting the inner filesystem still need privileges. In the checked upstream source, `MountVolumeRequest::RequiresElevation()` returns `!Core->HasAdminPrivileges()` on Linux. `--mount-options=nokernelcrypto` changes the crypto engine; it does not remove the privilege requirement for the remaining mount steps.

Lurker’s Privileged route uses a different opening path: system VeraCrypt creates the container, but cryptsetup opens it and the host mounts its filesystem. The installed VeraCrypt application is not needed to mount an existing container through that route.

Lurker’s Unprivileged route reuses VeraCrypt’s crypto/FUSE implementation, bypasses loop-device setup, and uses LKL/FUSE for the inner filesystem. The bundled modified `lurker-veracrypt` helper is unprivileged-only and refuses root execution. Stock VeraCrypt and the bundled helper are not interchangeable executables.

## Linux implementation comparison

| Area | Privileged | Unprivileged |
|---|---|---|
| Execution | CLI privileged commands use root/sudo; desktop helper uses pkexec with terminal sudo fallback | CLI/helper and both native backends run as the login user; no elevation fallback |
| Create | System VeraCrypt, followed by mkfs.btrfs through a kernel mapping | Bundled modified VeraCrypt, followed by mkfs.btrfs on the decrypted FUSE file |
| Open | `cryptsetup open --type tcrypt --veracrypt`, device-mapper and kernel crypto | VeraCrypt userspace crypto exposing a decrypted FUSE image |
| Filesystem | Host filesystem driver or mount helper | Linux filesystem implementation inside the LKL process, exported through FUSE |
| Targets | Regular files, disks and partitions | Regular files only |
| Existing filesystems | Those supported by the host’s mount tools | Validated Btrfs, ext4, exFAT, FAT, NTFS3 and XFS |
| New Btrfs ownership | Initially root-owned | Initialized for the login uid/gid |
| Btrfs management | Host ioctls/tools available with appropriate permissions; not exposed as new Lurker UI controls | Ordinary file operations; no host Btrfs snapshot/scrub/device-management ioctls |
| Updates | Installed VeraCrypt/cryptsetup and host filesystem components | Host FUSE plus bundled, pinned VeraCrypt/LKL helpers, which need maintained revisions and rebuilds |
| Status | Existing kernel-backed Lurker workflow | Experimental integration with additional native patches and process lifecycle logic |

Both creation routes use the same normal-volume AES/Serpent/Twofish, SHA-512, PIM 0 and no-keyfile profiles, and create Btrfs with zstd mounting. Choosing Unprivileged does not select weaker encryption defaults.

The container format is intended to be interchangeable after unmounting, subject to filesystem support and permissions. Tests confirmed that cryptsetup accepts headers/passwords from userspace-created containers using `--test-passphrase`. That check does not prove a full cross-mode filesystem read/write cycle; dedicated mount/write/remount tests in both directions remain useful follow-up work. Never open the same container concurrently through the two routes.

No comparative throughput or latency benchmark has been run. The userspace route adds two FUSE layers and a 192 MiB LKL memory arena per mounted filesystem, plus VeraCrypt buffers. Extra overhead is an architectural expectation, not a measured slowdown.

## Supported unprivileged operations

- Create normal VeraCrypt file containers with AES, Serpent or Twofish, SHA-512, PIM 0 and no keyfiles. Cipher profiles match the privileged route. Files are fully initialized, protected with mode 0600, and contain Btrfs; minimum size is 128 MiB.
- Mount existing normal password-only VeraCrypt files, including headers using hashes other than the create default. Validated filesystem drivers are Btrfs, ext4, exFAT, FAT, NTFS3 and XFS. The ext4 driver also supports ext2/3 formats; those variants have not had separate integration runs.
- Read/write, read-only at both layers, Unicode paths and passphrases, preserved spaces, source symlinks, same-directory staged creation, atomic no-clobber publication and forced replacement after successful initialization.
- Btrfs zstd compression and ordinary file operations, including user ownership, permissions, directories, links, rename, truncate, sparse files, and fsync. The driver uses the login uid inside its userspace kernel; an existing filesystem's stored ownership is not rewritten.
- Active volume discovery after restarting the desktop/helper, and unmount by source, mountpoint or decrypted image. Busy filesystems and raw images remain open for a later retry. Unmount does not require the crypto or filesystem executables still to be installed.

LUKS, physical-device operations and LUKS tags use Privileged mode. The existing interface still does not expose keyfiles, custom PIM, hidden-volume controls or cipher cascades. Kernel Btrfs ioctls (snapshots, scrub, device management) are not provided by the FUSE filesystem interface.

## Architecture and privilege boundary

`lurker-core/src/userspace.rs` orchestrates two external processes:

1. `lurker-veracrypt` uses upstream VeraCrypt's userspace crypto and exposes the decrypted image through FUSE. Its narrow integration patch disables sudo/doas elevation, refuses execution as root, bypasses loop setup for file-only userspace opens, and adapts sub-sector writes without changing encryption algorithms or container format.
2. `lurker-fs` uses the Linux Kernel Library (LKL) to run the filesystem implementation as an ordinary process. The host kernel sees FUSE rather than a loop/device-mapper block device. New filesystem initialization assigns the root directory to the caller. The LKL kernel uses that uid for normal filesystem access; its internal saved uid is used only to perform its own clean unmount, and grants no host privileges.

Normal user FUSE mounting uses the distribution's installed setuid `fusermount3` helper. There is no sudo, polkit dialog, administrator password, new setuid executable or system configuration change per operation. This is distinct from a claim that no system service/helper has any privileges. Systems which disable unprivileged FUSE cannot use this route.

Native revisions and integration patches are fixed in `scripts/build-linux-userspace.py` and `native/linux-userspace/patches`. Helpers identify their protocol version before use; an ordinary upstream VeraCrypt executable cannot accidentally substitute for the file-only helper. Build outputs and source caches live under ignored `target/` directories. The builder verifies patch content even for stamped caches and regenerates the fixed filesystem configuration. Each mounted filesystem uses a 192 MiB LKL memory arena in addition to VeraCrypt buffers; performance differs from the kernel route. The AUR recipe includes the helpers and their upstream licenses.

## Security findings and remaining hardening

The architectural advantage is a smaller initial privilege scope for some failures. Exploiting a filesystem parser in the host kernel could compromise the host; exploiting the LKL parser would initially gain the filesystem process’s normal-user permissions. Host kernel/FUSE vulnerabilities remain a separate possible escalation path. This is a containment inference from the architecture, not proof that the experimental route is safer overall.

- **Normal user is not a sandbox.** The helpers run as the logged-in account and are not confined to the vault by a host sandbox or separate account. Compromising either process can expose that user’s accessible home files and mounted data. LKL’s internal root and saved uid 0 are inside the library’s kernel state; they do not grant host root and do not create a host isolation boundary.
- **System privileges still exist.** The route relies on the installed privileged fusermount3 helper and the kernel FUSE driver. It adds no setuid executable, sudoers rule or polkit grant. The crypto FUSE patch does not enable `allow_other` for the normal user, retaining FUSE’s default mount-owner access restriction.
- **Memory protection needs separate verification.** Userspace crypto retains live keys in a user process, and LKL adds plaintext buffers in user-process memory. The wrapper does not add memory locking or a general dump/ptrace hardening policy. Swap, core dumps, same-uid debugger access, hibernation and complete key erasure have not been validated for the complete route. Both frontends handle passphrases in user memory, and applications can hold plaintext in either mode.
- **A compromised account or administrator can still expose an unlocked vault.** Neither route protects plaintext that malware can read using that account’s access. Read-only mounts prevent writes through that mount; they do not authenticate container contents or stop another process with access to the backing file from modifying it. VeraCrypt’s format does not provide general encrypted-data authenticity/integrity guarantees.
- **More integration code needs review.** Sector adaptation, process startup, journal draining and recovery are new code paths. The tests below establish exercised behavior, not an independent security audit. Narrower host privileges do not establish stronger overall confidentiality or data-loss resistance.
- **Pinned dependencies need ongoing maintenance.** Host kernel updates do not update the filesystem code embedded in lurker-fs. Review upstream VeraCrypt/LKL security fixes and refresh/rebuild the bundled helpers when needed; pinning provides reproducibility, not an assurance that a revision has no vulnerabilities.

Further work should verify memory/dump/debugger policies, assess a dedicated process sandbox, and extend fault injection to resource exhaustion and abrupt termination across startup/shutdown stages. Benchmark performance and exercise full cross-mode persistence before making stronger comparative claims. These are open review/verification items, not implemented protections.

## Unprivileged session safety and recovery

Each uid has a private directory under `XDG_RUNTIME_DIR`, or a private uid-specific temporary directory when no runtime directory exists. Session records contain paths and resource identities, never passphrases. JSON is published atomically. A per-user operation lock serializes userspace changes.

Source device/inode identity prevents duplicate opens through aliases. Existing kernel loop attachments are refused. Privileged requests are checked before desktop elevation to prevent replacing a file already served by this user's userspace backend. Mount IDs prevent a stale session from unmounting a replacement mount. A filesystem process is identified by pid and process start time, not by a reusable pid alone.

Unmount first releases the filesystem, remounts its internal disk read-only to commit journal state, and waits for a durable shutdown acknowledgement and driver exit before releasing the crypto FUSE image. Read-only opens also advertise a read-only virtual block device inside LKL. It never uses lazy or forced unmount. A busy filesystem leaves both layers intact. If filesystem shutdown is incomplete, or the raw image is busy, the session remains available in Manage and can be retried. After a driver crash, unmount restarts the filesystem on the retained decrypted image to recover its journal, then requires a clean close. Recovery needs the native helpers; a failure preserves the session. Only data already flushed before a crash can be recovered. Incomplete staged creates retain their recovery record when cleanup cannot safely finish; an existing destination is not published over until the new container is closed and synced.

## Implementation notes from testing

- Filesystem tools issue writes smaller than a crypto sector. The crypto FUSE patch serializes sector read/modify/write adaptation and checks image bounds; it does not change the encryption algorithms.
- A driver exiting successfully was insufficient evidence of a clean journal. Shutdown must return to LKL’s original root, remount its filesystem read-only, unmount and sync the backing image, then acknowledge completion. The parent keeps crypto open until the driver drains. Tests exposed dirty ext4/XFS journal behavior before this was fixed.
- Read-only must reach the virtual block device inside LKL as well as the host image descriptor and filesystem mount. Advertising the virtio disk as read-only prevents XFS from attempting otherwise permitted log writes during a read-only mount. Recovery that requires writes must fail on a read-only image rather than silently enabling writes.
- The filesystem driver needs its own process session (`setsid`) and redirected standard streams. Otherwise closing the CLI’s terminal can kill a supposedly mounted filesystem. Pseudoterminal tests cover survival after caller exit.
- Disable terminal echo before printing a password prompt. Fast automated input exposed a race in the convenience prompt’s ordering. The current wrapper also restores terminal settings on typed Ctrl-C; this does not establish correct cleanup for every externally delivered signal.
- wxWidgets needs a UTF-8 locale for Unicode filenames and secrets. The wrapper uses C.UTF-8, preserves spaces, rejects NUL/newline secrets, and sends secrets over stdin rather than argv.
- Build caches cannot substitute for patch verification. GNU patch can guess the opposite direction during a reverse dry run unless forced. The builder checks the actual patch state on each run, resolves symlinked source paths, and regenerates the fixed LKL configuration so old caches cannot silently omit filesystem drivers.
- The desktop checks helper protocol 2 and unprivileged capability before sending an operation; native helpers must report `lurker-userspace-2`. An old helper must be rejected before it could ignore the new privilege field and elevate.

## Build and verification

```bash
python scripts/build-linux-userspace.py
cargo build --release --workspace --locked
cargo check --workspace --locked
cargo test --workspace --locked
python -m unittest discover -s scripts/tests -v
./scripts/test-linux-userspace.sh
python scripts/test-linux-frontends.py target/release --unprivileged
```

Run the userspace suite as a normal user. It creates its own runtime directory and fresh `/tmp/lurker-userspace-test-*` containers; escalation/kernel-tool commands are denied in its PATH. It does not accept existing user volumes as fixtures. The existing personal vault is excluded from all operations.

The real suite covers all three create ciphers, Unicode and padded secrets, POSIX operations, zstd-compressed extents, duplicate/busy/overwrite guards, automatic and explicit types, readonly crypto and filesystem writes, source aliases, standard cryptsetup header compatibility without mapping, exFAT/FAT/ext4 interoperability, injected formatter failures, stale mount identities, partial-unmount recovery when helpers are unavailable, and recovery after killing the filesystem driver.

Optional NTFS/XFS interoperability coverage uses locally available formatters, without installing packages:

```bash
python scripts/test-linux-userspace-extra.py
# Or use formatters from locally extracted distro packages:
python scripts/test-linux-userspace-extra.py --formatters /path/to/package-root
```

The frontend suite also opens and closes real pseudoterminals, verifies that mounted drivers survive the caller’s terminal-session exit, checks password prompts without secret echo, and exercises helper JSON success and failure responses with elevation tools denied.

The normal Cargo suite covers backward-compatible JSON defaults, privilege-mode parsing, helper error handling, UI mode transitions, software rendering and existing macOS form behavior. Privileged Linux integration suites remain separate and use their own disposable loops. Native macOS tools require a Mac to run; Linux-only backend changes are compile-time gated.

## Recorded validation and evidence limits

The implementation run on 2026-10-22 recorded 47 passing workspace tests, four native-source preparation tests, six privileged real-workflow suites (including 12 create-profile combinations), and two real userspace suites. Optional NTFS/XFS checks, CLI/helper scenarios, and the packaged desktop’s isolated Wayland/X11 launch checks also passed. Userspace tests denied elevation/kernel-tool commands through their PATH and ran as the normal user. These results were recorded before this documentation update; they are not new runs of the suites.

Local evidence is retained under ignored `dist/validation/linux-userspace/`: report.json, source-manifest.json and test logs. These generated artifacts are not required to exist in a fresh checkout. The tests and build scripts in the repository provide the reproducible checks. Performance, a comprehensive adversarial audit and the memory hardening described above were not established by those passes.

All integration fixtures were new disposable images and dedicated loops. The existing personal LUKS vault was excluded from operations. Future verification must use fresh fixtures as well.

## Source references

The build script pins VeraCrypt revision `8172bc55f834485f67be271e02c1167981423621` and LKL revision `d0f76a77e8c2ca68bd7d96190b8da09040eab072`. The stock behavior described above was checked against that VeraCrypt snapshot, before Lurker’s patches. Update these findings when changing the pins.

- Stock FUSE selection: src/Makefile, WITHFUSE3.
  https://github.com/veracrypt/VeraCrypt/blob/8172bc55f834485f67be271e02c1167981423621/src/Makefile
- Stock elevation policy: src/Core/Unix/CoreServiceRequest.cpp, MountVolumeRequest::RequiresElevation.
  https://github.com/veracrypt/VeraCrypt/blob/8172bc55f834485f67be271e02c1167981423621/src/Core/Unix/CoreServiceRequest.cpp
- Stock decrypted-image/loop path: src/Core/Unix/CoreUnix.cpp, MountAuxVolumeImage.
  https://github.com/veracrypt/VeraCrypt/blob/8172bc55f834485f67be271e02c1167981423621/src/Core/Unix/CoreUnix.cpp
- LKL source snapshot:
  https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
- Linux FUSE privilege/access model:
  https://docs.kernel.org/filesystems/fuse/fuse.html
- VeraCrypt security model and RAM limitations:
  https://veracrypt.io/en/Security%20Model.html
  https://veracrypt.io/en/Unencrypted%20Data%20in%20RAM.html

Local implementation references: crates/lurker-core/src/userspace.rs, linux_workflow.rs and workflow.rs; crates/lurker-helper/src/main.rs; apps/lurker-desktop/src/helper.rs; native/linux-userspace/patches; scripts/build-linux-userspace.py; crates/lurker-core/tests/userspace_workflows.rs; scripts/test-linux-frontends.py; scripts/tests/test_linux_userspace_build.py.
