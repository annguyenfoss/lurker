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

Lurker’s Privileged route uses a different opening path: a private, pinned VeraCrypt-derived creator initializes the container, but cryptsetup opens it and the host mounts its filesystem. The installed VeraCrypt application is not needed to mount an existing container through that route.

Lurker’s Unprivileged route reuses VeraCrypt’s crypto/FUSE implementation, bypasses loop-device setup, and uses LKL/FUSE for the inner filesystem. The bundled modified `lurker-veracrypt` helper is unprivileged-only and refuses root execution. Stock VeraCrypt and the bundled helper are not interchangeable executables.

## Linux implementation comparison

| Area | Privileged | Unprivileged |
|---|---|---|
| Execution | CLI privileged commands use root/sudo; desktop helper uses pkexec with terminal sudo fallback | CLI/helper and both native backends run as the login user; no elevation fallback |
| Create | Bundled private VeraCrypt creator, followed by mkfs.btrfs through a kernel mapping | Bundled modified VeraCrypt, followed by mkfs.btrfs on the decrypted FUSE file |
| Open | `cryptsetup open --type tcrypt --veracrypt`, device-mapper and kernel crypto | VeraCrypt userspace crypto exposing a decrypted FUSE image |
| Filesystem | Host filesystem driver or mount helper | Linux filesystem implementation inside the LKL process, exported through FUSE |
| Targets | Regular files, disks and partitions | Regular files only |
| Existing filesystems | Those supported by the host’s mount tools | Validated Btrfs, ext4, exFAT, FAT, NTFS3 and XFS |
| New Btrfs ownership | Initially root-owned | Initialized for the login uid/gid |
| Btrfs management | Host ioctls/tools available with appropriate permissions; not exposed as new Lurker UI controls | Ordinary file operations; no host Btrfs snapshot/scrub/device-management ioctls |
| Updates | Bundled creator plus installed cryptsetup and host filesystem components | Host FUSE plus bundled, pinned VeraCrypt/LKL helpers, which need maintained revisions and rebuilds |
| Status | Existing kernel-backed Lurker workflow | Experimental integration with additional native patches and process lifecycle logic |

Both creation routes use the same normal-volume AES/Serpent/Twofish, SHA-512, PIM 0 and password-or-keyfile authentication profiles, and create Btrfs with zstd mounting. Choosing Unprivileged does not select weaker encryption defaults.

The container format is intended to be interchangeable after unmounting, subject to filesystem support and permissions. Tests confirmed that cryptsetup accepts both password-only and keyfile-only headers from userspace-created containers using `--test-passphrase`. That check does not prove a full cross-mode filesystem read/write cycle; dedicated mount/write/remount tests in both directions remain useful follow-up work. Never open the same container concurrently through the two routes.

No comparative throughput or latency benchmark has been run. The userspace route adds two FUSE layers and a 192 MiB LKL memory arena per mounted filesystem, plus VeraCrypt buffers. Extra overhead is an architectural expectation, not a measured slowdown.

Interactive LUKS creation retains two passphrase entries. Lurker validates staged-file publication and explicit destructive `--force` itself, then uses native batch mode with explicit passphrase verification to avoid a second cryptsetup `YES` confirmation.

## Bundled privileged-route creator

The privileged route discovers `lurker-veracrypt-create` beside the application under `backends/` or `userspace/`, in the package prefix at `../libexec/lurker/backends`, then under `/usr/lib/lurker/{backends,userspace}`. It does not search PATH for a separate VeraCrypt application. Normal-user developer runs may set `LURKER_BACKEND_DIR`; root ignores that override. Missing creator disables only VeraCrypt creation, not LUKS or mounting existing VeraCrypt files.

The creator is built separately from the same pinned upstream source as the root-refusing userspace helper. Its entry-point whitelist allows only Lurker's create profile arguments, one target and upstream's in-memory build self-test; it rejects mount, formatter, service/elevation and general VeraCrypt operations. Keyfile operands must be canonical absolute paths or the empty password-mode operand, preventing an option from hiding inside a keyfile argument. The existing userspace helper remains unprivileged-only. No cipher primitive is replaced.

Before any root execution or block-device elevation, discovery canonicalizes the creator and requires the executable and every ancestor to be root-owned and not group/world writable. A developer build under a home directory or `/tmp` cannot be elevated. Install a trusted package first for a root desktop helper or physical-device create. Ordinary file creation from a normal-user CLI can execute its user-owned build without elevating the creator; the later cryptsetup/filesystem operations retain their existing privilege boundary.

Build only the creator with `python scripts/build-linux-userspace.py --only-creator`; the full build produces all three helpers. `creator-source.json` records the source revision, pinned archive checksum, patch hash and output hash, with the VeraCrypt license beside it. Host dynamic library dependencies still require packaging qualification; this build does not claim a portable static executable.

`python scripts/test-linux-bundled.py --cli /path/to/lurker --result /tmp/lurker-result.json` runs the native CLI matrix as root **inside a disposable test VM**. It uses private `/tmp/lurker-bundled-test-*` images, checks all three algorithms for LUKS2, LUKS1 and VeraCrypt, and tests persistence, SQLite WAL, rename, incorrect keys, duplicate mounts, no-clobber create and readonly. It does not invoke a separately installed VeraCrypt. Logs/results survive failure, and backing storage is retained when safe unmount fails. The runner also checks native mounts/loop attachments before removing fixtures and preserves uncertain state after an operation timeout. This is additional coverage; optional stock-VeraCrypt interoperability tests remain separate. `python scripts/test-linux-creator.py --creator /path/to/lurker-veracrypt-create` checks rejection of mount/service/formatter operations, option injection and unsupported crypto arguments without creating a volume. Adding `--cli /path/to/lurker` as root in the test VM also verifies a backend beneath writable `/tmp` is rejected before execution.

### Native validation recorded 2026-11-14

On `archlinux-core-01` (x86-64, Linux 7.2.15, glibc 2.44), the CLI built from `1f03193cb574be085ad1fd5036f9dce1ad6b2889` passed all **9** privileged-route file-container cases: LUKS2/LUKS1/VeraCrypt × AES/Serpent/Twofish, binary-keyfile authentication, wrong keys, persistence, SQLite WAL/fsync, rename, duplicate/no-clobber guards and readonly. No separate `veracrypt` executable was installed. The root-refusing userspace stack passed all **3** native regression suites (265.94 s), including password/keyfile profiles, external filesystem interoperability and driver-crash recovery. No suite mounts remained.

The tightened creator and elevated discovery passed **10** native argument/trust boundary checks. Generated reports with artifact hashes are retained under `target/remote-tests/linux-bundled-1f03193/`; full guest logs remain under private `/tmp/lurker-*` test directories. These are development-revision results, not qualification of later revisions, other distributions, release packages or physical devices. Runtime encryption tests ran only on the disposable guest; local work was limited to code/builds.

### Staged package qualification recorded 2026-11-14

The exact `76f05fbd2167313273c53201a070252719e4f4ea` package passed **195/195** CLI conformance checks on the Arch guest: LUKS2/LUKS1/kernel VeraCrypt and userspace VeraCrypt across all three ciphers, plus gocryptfs, CryFS and Cryptomator. It also passed **8** cross-mode exclusion cases, **9** owned-loop block-create profiles and **30** directory-daemon crash checks with committed, uncheckpointed SQLite FULL/WAL data. No separate VeraCrypt, Java or Go executable was on PATH. Cryptomator used private FUSE-NIO 5.1.0; CryFS included the reviewed fsync/mutation-lock patch. The terminal driver used committed ECHO-readiness fix `8bf9f0e`; a corrected user-process environment resumed the remaining phases without repeating passed kernel cells.

The final `a16cd18b8462293c670e66803fc4e37fc4a9c59f` package includes guarded forced-publication fixes added after that baseline. Its focused delta passed **8/8** cross-mode cases and **4/4** real forced-replacement cases (24 checks: privileged LUKS2/LUKS1/VeraCrypt and userspace VeraCrypt, AES), including unchanged-target replacement, no-force preservation, filesystem replacement, persistence and readonly. The four new publication race/identity regressions passed within **41** core unit tests at `fd0e06c`. The final desktop release build passed; this is build evidence, not a desktop runtime test. No loops, Lurker mappings or backend processes remained after either completed package run.

The native protocol-3 helpers are byte-identical to those that passed all **3** native userspace regression suites at `03ae13b` (262.63 s), including binary keyfiles, multiple inner filesystems and LKL crash recovery. Evidence and file hashes are retained under `target/linux-package-{76f05fbd21,a16cd18b84}/qualification/` and `target/native-source3-03ae13b96e/`. Final archive SHA-256: `4af894b74c1bc424ce6155f85c73c2917516c156e1ef7d9c06999787da5cb02b`. These runs qualify the staged Arch/glibc dependency closure and private fixtures; they do not establish cross-distribution binary portability, physical-disk behavior or a security audit. All encryption runtime work used the guest; endeavour was used only for code and builds.

## Supported unprivileged operations

- Create normal VeraCrypt file containers with AES, Serpent or Twofish, SHA-512, PIM 0 and either a password or one binary keyfile. Cipher profiles match the privileged route. Files are fully initialized, protected with mode 0600, and contain Btrfs; minimum size is 128 MiB.
- Mount existing normal password- or keyfile-protected VeraCrypt files, including headers using hashes other than the create default. Validated filesystem drivers are Btrfs, ext4, exFAT, FAT, NTFS3 and XFS. The ext4 driver also supports ext2/3 formats; those variants have not had separate integration runs.
- Read/write, read-only at both layers, Unicode paths and passphrases, preserved spaces, source symlinks, same-directory staged creation, atomic no-clobber publication and forced replacement after successful initialization.
- Btrfs zstd compression and ordinary file operations, including user ownership, permissions, directories, links, rename, truncate, sparse files, and fsync. The driver uses the login uid inside its userspace kernel; an existing filesystem's stored ownership is not rewritten.
- Active volume discovery after restarting the desktop/helper, and unmount by source, mountpoint or decrypted image. Busy filesystems and raw images remain open for a later retry. Unmount does not require the crypto or filesystem executables still to be installed.

LUKS, physical-device operations and LUKS tags use Privileged mode. Keyfile authentication uses an empty native password; combining a password and keyfile is deliberately unsupported. The interface does not expose custom PIM, hidden-volume controls or cipher cascades. Kernel Btrfs ioctls (snapshots, scrub, device management) are not provided by the FUSE filesystem interface.

## Architecture and privilege boundary

`lurker-core/src/userspace.rs` orchestrates two external processes:

1. `lurker-veracrypt` uses upstream VeraCrypt's userspace crypto and exposes the decrypted image through FUSE. Its narrow integration patch disables sudo/doas elevation, refuses execution as root, bypasses loop setup for file-only userspace opens, and adapts sub-sector writes without changing encryption algorithms or container format.
2. `lurker-fs` uses the Linux Kernel Library (LKL) to run the filesystem implementation as an ordinary process. The host kernel sees FUSE rather than a loop/device-mapper block device. New filesystem initialization assigns the root directory to the caller. The LKL kernel uses that uid for normal filesystem access; its internal saved uid is used only to perform its own clean unmount, and grants no host privileges.

Normal user FUSE mounting uses the distribution's installed setuid `fusermount3` helper. There is no sudo, polkit dialog, administrator password, new setuid executable or system configuration change per operation. This is distinct from a claim that no system service/helper has any privileges. Systems which disable unprivileged FUSE cannot use this route.

Native revisions and integration patches are fixed in `scripts/build-linux-userspace.py` and `native/linux-userspace/patches`. Helpers identify their protocol version before use; an ordinary upstream VeraCrypt executable cannot accidentally substitute for the file-only helper. Build outputs and source caches live under ignored `target/` directories. The builder verifies pinned source-archive SHA-256 checksums before extraction or reuse, verifies patch content even for stamped caches, and regenerates the fixed filesystem configuration. Offline builds can re-extract a verified archive cache. Archive verification does not audit arbitrary changes inside a mutable generated build tree. Each mounted filesystem uses a 192 MiB LKL memory arena in addition to VeraCrypt buffers; performance differs from the kernel route.

## Security findings and remaining hardening

The architectural advantage is a smaller initial privilege scope for some failures. Exploiting a filesystem parser in the host kernel could compromise the host; exploiting the LKL parser would initially gain the filesystem process’s normal-user permissions. Host kernel/FUSE vulnerabilities remain a separate possible escalation path. This is a containment inference from the architecture, not proof that the experimental route is safer overall.

- **Normal user is not a sandbox.** The helpers run as the logged-in account and are not confined to the vault by a host sandbox or separate account. Compromising either process can expose that user’s accessible home files and mounted data. LKL’s internal root and saved uid 0 are inside the library’s kernel state; they do not grant host root and do not create a host isolation boundary.
- **System privileges still exist.** The route relies on the installed privileged fusermount3 helper and the kernel FUSE driver. It adds no setuid executable, sudoers rule or polkit grant. The crypto FUSE patch does not enable `allow_other` for the normal user, retaining FUSE’s default mount-owner access restriction.
- **Memory protection needs separate verification.** Userspace crypto retains live keys in a user process, and LKL adds plaintext buffers in user-process memory. The wrapper does not add memory locking or a general dump/ptrace hardening policy. Swap, core dumps, same-uid debugger access, hibernation and complete key erasure have not been validated for the complete route. Both frontends handle passphrases in user memory, and applications can hold plaintext in either mode.
- **A compromised account or administrator can still expose an unlocked vault.** Neither route protects plaintext that malware can read using that account’s access. Read-only mounts prevent writes through that mount; they do not authenticate container contents or stop another process with access to the backing file from modifying it. VeraCrypt’s format does not provide general encrypted-data authenticity/integrity guarantees.
- **More integration code needs review.** Sector adaptation, process startup, journal draining and recovery are new code paths. The tests below establish exercised behavior, not an independent security audit. Narrower host privileges do not establish stronger overall confidentiality or data-loss resistance.
- **Pinned dependencies need ongoing maintenance.** Host kernel updates do not update the filesystem code embedded in lurker-fs. Review upstream VeraCrypt/LKL security fixes and refresh/rebuild the bundled helpers when needed; pinning provides reproducibility, not an assurance that a revision has no vulnerabilities.

Further work should verify memory/dump/debugger policies, assess a dedicated process sandbox, and extend fault injection to resource exhaustion and abrupt termination across startup/shutdown stages. Benchmark performance and exercise full cross-mode persistence before making stronger comparative claims. These are open review/verification items, not implemented protections.

## Source identity and cross-mode exclusion

Native Linux file mounts hold a nonblocking exclusive inode lock through mapper startup. Administrator-controlled `losetup` attaches the pinned source descriptor; cryptsetup queries and opens that loop block device, avoiding a conflicting metadata flock on the container file. The loop backing filesystem/inode and the opened mapper are verified before filesystem access. The owned loop is detached after handled failures or marked for automatic removal when its mapper closes. Hard links and renamed backing files cannot bypass the inode check. The CLI needs read access to lock the source; run the privileged CLI as root for root-owned unreadable containers. Missing or ambiguous native identity metadata fails closed.

An externally delivered `SIGKILL` during kernel startup can leave an attached loop before mapper acquisition: the caller does not hold a loop descriptor, and autoclear is set after the mapper opens. Later starts refuse that active backing inode. Recovery is manual: first use normal Lurker unmount if a mapper exists; otherwise compare the retained source filesystem device/inode with `losetup --json --list --output NAME,BACK-INO,BACK-MAJ:MIN`, verify the matching loop has no holders or mounts, then detach only that verified device. Never use detach-all or assume a reused loop number still belongs to the failed operation. This fail-closed resource leak is not covered by ordinary error cleanup.

The private userspace crypto driver holds the same exclusive inode lock for the mounted volume's entire lifetime, including read-only opens. It checks active loop backing identities again **after** taking that lock, closing the kernel-first startup race. Unprivileged startup refuses deleted or inaccessible active loop backing paths when it cannot establish their identity. It does not elevate to bypass that check. Forced replacement also locks an existing destination before staging the new volume.

Forced creation in either Linux route replaces only the unchanged regular-file inode locked at startup. If no destination existed at startup, publication uses atomic no-clobber even with `--force`; a destination appearing during encryption is preserved. Replacing or removing an originally locked path aborts publication. This identity check coordinates cooperative Lurker operations; it does not make check-and-rename atomic against malicious changes to a writable parent directory.

Native bundle protocol 3 is required. Before upgrading a protocol-2 installation, close its existing userspace volumes using the old installed Lurker; already-running old drivers do not acquire the new locks merely because files on disk were updated. Do not kill mounted drivers as an upgrade procedure. This work does not install or replace the production Lurker on endeavour.

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
- The desktop negotiates helper JSON capabilities and requires userspace backend protocol 3 before sending an unprivileged operation; native helpers must report `lurker-userspace-3`. An old helper must be rejected before it could ignore the new privilege field and elevate.

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

## Keyfile verification (2026-11-11)

The `keyfile_only_userspace_workflows` native test passed for AES, Serpent and Twofish using the installed helpers at the same pinned VeraCrypt/LKL revisions as the builder. Each case created a disposable Btrfs container with a binary keyfile whose name contains a comma and spaces, rejected a wrong keyfile and password-only unlock, wrote data, unmounted, remounted read-only, rejected writes at both crypto and filesystem layers, and authenticated the header with stock cryptsetup without a mapper. The run took 138.06 seconds. The existing password matrix also passed for all three ciphers in 48.91 seconds.

Repeat the keyfile cases as the login user, without sudo:

```bash
LURKER_USERSPACE_DIR=/usr/lib/lurker/userspace ./scripts/test-linux-userspace.sh keyfile_only_userspace_workflows
```

Use the helper directory produced by the builder when testing a local helper build.
