# Fedora affected Linux contract 5 follow-up — e927e40

The bounded affected runtime checks pass on Fedora as login UID/GID 1300.
Ordinary installation of the unsigned native RPM remains blocked by the stock
signature policy. Runtime qualification used its byte-verified payload in a new
administrator-controlled private prefix. This does not establish an RPM-database
installation, global launcher/menu integration or uninstall result.

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
The completion index contains 136 hashed references and explicit scope/gaps:

- `completion-evidence-index.json`, SHA-256
  `c3af820981a08733edbd8bc185775da96fafe2bedd305da817ae9ffb3a5dcc10`.
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
