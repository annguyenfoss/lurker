# Fedora affected Linux contract 5 follow-up — e927e40

The bounded affected runtime checks pass on Fedora as login UID/GID 1300.
Runtime qualification used the native RPM's byte-verified payload in a new
administrator-controlled private prefix. A subsequent, separately authorized
qualification-only signing control passed normal RPM-database installation,
installed CLI/helper discovery, actual XDG/GIO desktop launch and uninstall under
the unchanged stock signature policy. Temporary trust and tools were removed.
This is not release signing or repository acceptance.

This follow-up is separate from `FEDORA_RESULTS.md` and the original 576505d /
67894f6 filesystem, physical-device and reboot receipts. Those results retain
their executed revisions, configured UFS prerequisites, failed first ZFS reboot
cohort and corrected native propagation reruns. No unchanged privileged matrix,
physical-device matrix or ordinary reboot was repeated for this batch.

## Frozen inputs and package

- Product source: `e927e40bb235815343302677531d48f1afa4f939`.
- Host: Fedora 45 Cloud, kernel `7.3.4-300.fc45.x86_64`, glibc 2.44,
  boot `cc9d2572-2a9c-424c-bd23-36cd2811e325`.
- Native managed Rust 1.100 workspace check, test and build passed; Rust results
  were 186 passed, 0 failed, 15 ignored. All 101 Python packaging checks passed.
  The selected ignored native desktop callback test subsequently passed too.
- Native RPM SHA-256:
  `1b87833d24aa20547564d136eeb1f28afbdaec76f3d715e5890b2cb4fdedb1d4`.
- Scoped installed prefix:
  `/usr/local/lib/lurker-native-fresh-20261127-fedora-package-e927e40`.
  All 1,085 manifest entries, including exact legal-notice symlinks, verify;
  executable files and ancestors are administrator controlled.

The ordinary `rpm -i --test` returned “does not verify: no signature” before
installation. The dependency-only transaction test passed. Verification policy
and package trust were preserved; the original 67894f6 private prefix was retained.
That original unsigned refusal remains evidence; only a separately identified
signed copy was used for the later normal installation control below.

The Rust frontends were built natively. The independently qualified crypto
protocol 4, restricted creator and external filesystem protocol 5 binaries were
reused compatibly, with their exact source, private zlib, license and static wx
provenance preserved. A separately identified derived SDK binds the actual
Fedora host FUSE provider through the frozen producer and stager. The reused
components retain their original build identities.

| Component | SHA-256 prefix; full hash is indexed |
| --- | --- |
| CLI | `6266f390a8ccab9b` |
| Helper | `170c1d20e9224915` |
| Desktop | `b43bf89dd9ae6c07` |
| Private crypto protocol 4 | `73f5e1cf90461b79` |
| Restricted creator | `7975cc4724d558fd` |
| External filesystem protocol 5 | `a5364138d90aabe0` |
| Native FUSE provider | `d6e5f6fefbdc4781` |

The filesystem driver is installed separately at `/usr/local/libexec/lurker-fs`,
with separate source/config/license records. Its in-process Linux filesystem
code uses native local I/O and host FUSE. It has no VM or network transport.
Actual Fedora versioned-loader/dependency checks passed for the three frontends,
two crypto executables, external filesystem driver and private zlib. FUSE resolves
from `/lib64/libfuse3.so.4`; zlib resolves from the new private prefix. The app
contains no filesystem driver or FUSE runtime. No administrator filesystem-driver
probe was executed; root readiness was also traced without backend execution.

## Affected native runtime results

| Check | Observation |
| --- | --- |
| Six CLI cipher/credential cases | AES/password/XFS, AES/keyfile/ext4, and Serpent/Twofish with password/keyfile on Btrfs all pass. Each uses Auto open, four downloaded inputs, append/new file, three opens, normal closes and five final hashes. All 42 CLI operations succeed. |
| Native filesystem binding | All six CLI cases bind each payload phase to the live host driver, UID, raw backing inode/descriptor, mount ID and independent native superblock. Initial root and payload ownership checks pass. |
| Strict helper and readiness | AES/password/ext4 completes the full downloaded-payload lifecycle through seven strict JSON actions. Keyfile coverage includes the live-close lifecycle and fault cases, with an expected wrong-key refusal. Five capability-role checks pass; only the explicit user readiness query probes backends. A root exec trace contains neither backend. |
| Three transient host FLUSH failures | XFS, Btrfs and ext4 produce application EIO 5. After disarming injection, both close attempts refuse; the `io-error` acknowledgement, original session, raw mount/inode and source lock remain. No automatic journal recovery is attempted. |
| Explicit manual recovery | All three pass separate operator-authorized test-only recovery after original image/key/session/ack/log archives are fully verified. A healthy write and independent reopen precede normal product close. This is not automatic product recovery. |
| Genuine absent acknowledgement | An owned filesystem-worker SIGKILL leaves no acknowledgement. Product journal recovery, normal close, reopen and four downloaded-input hashes pass; the source lock becomes available. |
| Native desktop callbacks | The committed actual winit test passes as UID 1300 through the installed helper, including create/mount/reopen/unmount and an unprivileged active row, without a source overlay. |
| Missing backend executable closes | CLI, helper and the actual desktop row all close normally while both `lurker-veracrypt` and external `lurker-fs` paths are temporarily absent. Sole-session guards and root-controlled backups preserve and restore exact inode, hash, UID/GID and mode. |

The actual expanded desktop row showed `btrfs` and `read/write · unprivileged`.
A native pointer click on **Unmount & lock**, while both paths were absent,
removed the row; the window then showed **0 mounted**. Raw XWD frames and derived
PNG inspection artifacts are retained. The fault injector is a separately built
private test interposer; the failure scope is host `fsync` EIO and an owned
filesystem-worker exit.

The original five bundled-driver comparisons remain historical failed/excluded
comparison evidence. They were not relabeled as this compliant external-driver
qualification. This batch establishes the specified affected regular-file cases
on XFS, ext4 and Btrfs; it makes no additional coverage claim for unprivileged
filesystems or physical devices.

## Evidence and cleanup

Private evidence root:
`target/goal-validation/native-fresh-20261127/hosts/fedora/build/unprivileged-e927/`.
The completion index contains 405 hashed references and explicit scope/gaps:

- `completion-evidence-index.json`, SHA-256
  `0f507e14ef97ed05dc293bab0979766402476441e7458039adba08b6eaef8a8e`.
  The previous index, SHA-256 `c3af820981a08733`, is retained under its full
  hash-named filename; earlier evidence was not relabeled.
- `native-e927-receipts-components-package.tar.gz`, SHA-256
  `7886941d4333fdb96f7e47e419ab22a57e4b365073f25fb0554d20e94850384a`:
  all 1,382 collected members, before/after hashes and complete gzip CRC verify.
- Six-case healthy archive: SHA-256
  `e0f9329de0d24ff1adb7860fd9aec75f2bef1eae335b669deb00e0a802799b7e`;
  70 members, 42 operations, common downloaded bytes, all final hashes and native
  driver identities are independently audited from retained data.
- Original negative archives: XFS `9856ad563610b8ba`,
  Btrfs `cc0772743764ea94`, ext4 `6c47beb8806d9871`; full hashes and member
  inventories are indexed. All original failed images/keys are retained locally.

The initial scoped installer assumed every manifest entry had a byte hash and
stopped before copying. Its corrected, separately named controller uses the
frozen stager's typed verification plus exact symlink/confinement checks. The
first pointer-control attempt found `xdotool` absent before any click; installing
the official native test tool allowed the unchanged controller to pass. Both
setup failures and the unsigned-RPM refusal remain in the evidence.

Final audit: no owned vault mounts, sessions, frontend/backend workers or backup
paths remain. Twelve retained closed encrypted sources have available source
locks and final hashes. The temporary callback bind was unmounted normally; the
reviewed root test controller was removed and the owned app/Xvfb processes stopped
after vault close. The intended new private prefix, external driver, test tools
and retained evidence remain installed. SELinux is Enforcing; the pre-existing
fixture alias remains private. Runtime sampling stayed above 18.75 GiB free,
with a 2 GiB guard; final free space was 18.76 GiB. No new qcow2 was needed.

Read-only final measurements confirm the earlier configured UFS1 module
`a13a90c94c41ca34` and three-patch UFS2 driver `237e0467fa4da053` are unchanged.
Their earlier observed passes remain distinct from stock Fedora limitations.
No UFS configuration work or new UFS tests occurred in this follow-up.

## Qualification-only normal RPM installation closeout

The disposable signing-only RSA3072 certificate was explicitly labeled
**DISPOSABLE QUALIFICATION ONLY**, with a one-day lifetime. Its sole full
fingerprint was `757b5c8c0acb8dce124415df88595d405d60a64a`; public certificate
SHA-256 is `6472fe732e43d9846cd5335f5f056617f5aff420be03221f93c35b15d6cb7f75`.
The secret stayed in a private 0700 GnuPG home and was never exported. Only a copy
of the original RPM was signed: SHA-256
`384de42deeee08c63059870983689387f575984eee0b569e803e0aa12ea0b538`.
The immutable header and uncompressed cpio remained identical to the unsigned
original; the complete cpio SHA-256 is
`abe72da7cc7faa786eb39ea22465bb62580f3d54497e567bbabfe4c523b67c27`.

Official `rpm-sign` and `desktop-file-utils` were the only two added packages;
their existing Fedora 45 signatures and transactions were inspected. The RPM
has no scriptlets or triggers, and all 1,738 candidate paths were collision-free.
The first controller had a syntax error before host operations. Private user
RPMDB initialization then failed with EACCES; root wrapper and direct `rpmdb`
attempts reported ENOENT. A retained syscall trace showed EACCES creating the
lock followed by ENOENT on a read-only fallback. No matching AVC was observed.
The private database had generic `var_lib_t`, while the stock database had
`rpm_var_lib_t`. An expressly approved type-only correction to the exact private
database inode allowed initialization and verification. The real RPMDB was not
relabeled; no policy module, boolean, enforcement or signature setting changed.

The root-private isolated database trusted only the pinned certificate and
verified the signed copy using native RPM tools with explicit `--dbpath` and
stock `all` / `0x20100` verification settings. After exact hash/fingerprint
checks, normal system import and `rpm -i --test` / `rpm -ivh` passed. RPMDB NEVRA,
dependencies, all 1,738 paths, empty `rpm -V`, and the three exact e927 frontend
ELF hashes verified. Installed CLI help and helper readiness passed; the helper
reported filesystem protocol 5 ready. Both prior private prefixes were unchanged.

`desktop-file-validate` accepted the installed entry. Actual
`gio launch /usr/share/applications/io.lurker.desktop.desktop` launched UID 1300
PID 58370 with window `0x200003` and exact installed desktop binary hash
`b43bf89dd9ae6c07`. The inspected frame showed the create view and **0 mounted**;
raw XWD SHA-256 is `52a5cf50126950cdae88a2b0488e7439bbf102cc5c7bbaea3d6a4cb6c6684336`.
This proves installed XDG entry/GIO launch, not interaction with a desktop-shell
panel. No additional storage or reboot sweep was run.

Normal uninstall removed every installed path. Only the full pinned temporary
fingerprint was erased; the original Fedora 45 certificate/export, 753-package
baseline and policy hashes were restored. The two added tools were removed
normally. After verified public evidence collection, the private agent and
secret-key home, owned root staging/database and its temporary label, and
Xauthority were removed. The original and signed RPMs, public certificate and
failures remain. SELinux is Enforcing, both prior prefixes are unchanged, and
final free space was 19,997,782,016 bytes.

Short per-run notes, exact controllers, commands and logs are under
`packaging-sign-plan/`. Public archive SHA-256 is
`2f373a703e6113bd934685b66a1c1eced060391cb2c926d9774f798698aa6274`;
all 117 retained member hashes verify and no secret-key home was collected.
The separate final-state/log archive is
`dafc8f5e6c67dd5cf7bbf64803dc28ffae28ba7cda60fec6f7edab5969c21810`.
Final receipt audit matches 110 of 112 command-log records: two intermediate
window-tree polling outputs were overwritten by the final same-label log.
Their original hashes remain, but those two stdout byte streams are unavailable.
The decisive final tree, PID, ELF, frame and completed cleanup receipts verify.
