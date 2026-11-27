# DragonFly BSD fresh qualification — in progress

Host: `adjutant@dragonflybsd-core-01`. Baseline:
`8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`. These are fresh observations;
prior artifacts and reports are not fresh passes. Final candidate qualification,
additional credentials/ciphers, scratch devices and ordinary reboot persistence
remain pending.

## Host and artifacts

- Running base: DragonFly 6.5-DEVELOPMENT, `e17b9176-DEVELOPMENT`,
  stock `X86_64_GENERIC` kernel built November 26 and booted at
  `kern.boottime=1795733143`. Initial container receipts below used the prior
  `v6.5.0.2269.g9f0e62-DEVELOPMENT` kernel; they are not updated-base final passes.
- `pkg update -f` and `pkg upgrade -y` completed with exit 0: nine upgrades and
  129 ABI-matched reinstalls. Rust 1.98.1, Python 3.12.16 and Python 3.14.10.
  The previously incomplete Python 3.14 installation now imports its standard
  library. Package vulnerability notices remain recorded in the upgrade log.
- Unmodified stock source `e17b9176356e3afb3427d71c21108a8a849cd4d5` completed
  `buildworld`, `buildkernel`, `installkernel`, `installworld` and `upgrade`, all
  exit 0. Build workers: two, with disk guards. A verified boot/etc rollback
  archive and automatic world backup are retained. The original kernel directory
  was preserved. Installed kernel SHA-256:
  `dbc7827922fed75bf63ef0678fd5884f007c178ecc610d5a1ffe467d92ba3d62`.
- Fresh exact-baseline CLI/helper build completed in run
  `20261126T205225Z-98c4aa71`, with one Cargo worker. The remaining baseline workspace build was cancelled when the
  adapter batch was ready; no workspace pass is claimed. Final complete workspace
  validation remains pending. Flags: incremental disabled, development/test debug info 0.
- CLI SHA-256:
  `fdf41eee9f994aa6dfaf6aa73edac6e284752400c4739ef60ac3da0e826f30c7`.
  Helper SHA-256:
  `a375ac9d03c7e345b06e35c0a4aad4528d77fe34a3a872f65458f149ab799e37`.
- The private tcplay wrapper was freshly rebuilt from checksum-verified cached
  pinned sources. SHA-256:
  `a1e8b4d152c05ad36f948d785e849e5b1ec487f83c76e9345a7cb4d061697be7`.
- Runtime binaries are copied into the administrator-controlled private prefix
  `/opt/lurker-native-fresh-20261127/baseline`. Results are staged-product results;
  they do not establish installed-package or desktop qualification.

## Completed initial container assessment

The initial 20 cases selected all ten filesystems separately with LUKS1 and
VeraCrypt. AES/password UFS1 and HAMMER2 passed with both backends: four passes.
Each copied the downloaded Bitcoin, Ethereum and Transformer PDFs plus text as
uid 1300, closed normally, reopened and compared hashes, appended text and a new
file, and reopened/compared again. Ownership was uid/gid 1300 and mode 0700.
All four plaintext mounts were closed.

The other 16 selections were refused by the baseline adapter. ext4, NTFS and
exFAT are product gaps: fresh independent native controls formatted, mounted,
wrote, normally closed/reopened and verified the downloaded payload, including
an append and a second reopen. Their host-local FUSE mounts were closed.
Btrfs, XFS, APFS, UFS2 and ZFS require separate availability/applicability
reconciliation; no pass is claimed for them.

Initial native filesystem candidates and provenance (later host updates follow):

| Candidate | Version/license | Source and observation |
| --- | --- | --- |
| e2fsprogs + fuse-ext2 | 1.47.4 GPLv2+; 0.0.11 GPLv2 | https://github.com/tytso/e2fsprogs and https://github.com/alperakcan/fuse-ext2 — packaged native formatter/FUSE driver; payload lifecycle passed |
| NTFS-3G | 2022.10.3 GPLv2+ | https://github.com/tuxera/ntfs-3g — packaged native formatter/FUSE driver; payload lifecycle passed |
| exfat-utils + fuse-exfat | 1.4.0 GPLv2+ | https://github.com/relan/exfat — packaged native formatter/FUSE driver; payload lifecycle passed |
| libfsapfs | 20240429 LGPL3+ | https://github.com/libyal/libfsapfs — installed reader, no writable creation claim |
| libfsxfs | 20240501 LGPL3+ | https://github.com/libyal/libfsxfs — installed reader, no writable creation claim |

The stock `fuse.ko` loads on this running base. Mount names must fit DragonFly's
80-byte native fields. A discarded experimental identity control incorrectly
used a regular-image vnode as the native FUSE device source and caused a kernel
panic during later close. This is not evidence of a stock-helper failure: a
fresh ordinary failed-mount/close control remained healthy. The corrected native
identity control used a root-private alias of `/dev/fuse`, closed normally and
left zero source handles. Any product integration must preserve that device-vnode
contract and root-private mount identity.

## Evidence and next action

Private local evidence:
`target/goal-validation/native-fresh-20261127/hosts/dragonflybsd/`.
Remote evidence is persistent under
`/home/adjutant/.cache/lurker/native-fresh-20261127-dragonflybsd/evidence`, exposed
through `/tmp/lurker-native-fresh-20261127-dragonflybsd`.
`cases/baseline-cases.jsonl` names all 20 receipts; `cases/native-*/receipt.json`
contains the three native filesystem controls. Failed fixtures are retained.

## Native filesystem adapter and host prerequisite fixes

- `cf6841a` adds NTFS/exFAT native formatting and FUSE mounts. Each mount has a
  root-private alias of `/dev/fuse`, an exact native identity and normal close.
  NTFS geometry comes from key-free mapper status. Synthetic ownership is private
  to the invoking user.
- The packaged NTFS 2022.10.3 library rejected unsupported advisory locks on raw
  DragonFly character devices. A native host package, `fusefs-ntfs-2026.2.25_1`,
  was built from the current upstream stable archive and pinned BSD port patches.
  A narrow DragonFly compatibility patch tolerates only unsupported raw-device
  locks; regular-file locking remains effective. A held regular-image lock
  rejected mount, left no mount and preserved the image hash.
  Source: https://download.tuxera.com/opensource/ntfs-3g_ntfsprogs-2026.2.25.tgz
  Archive SHA-256:
  `7754f3b32e8baf9c472459b4e9c981e3ae0f5039107cdd8d8201aed0a949008a`.
- ext4 formatting requires explicit mapper block geometry. The ordinary native
  FUSE driver also failed flush on raw devices: tracing found a two-byte
  superblock write at offset 1036, rejected with EINVAL; device fsync returned 0.
  A host `fusefs-ext2-0.0.11_3` package enables the existing libext2fs aligned I/O
  path for DragonFly character devices. This preserves real flush/fsync and
  leaves regular-image behavior unchanged. Upstream source:
  https://github.com/alperakcan/fuse-ext2/commit/ae35afb9ab08d87c66c1e021df792b3a7c4308b0
- `1f627f9` adds ext4 formatting/mounting and initializes VeraCrypt ext4 root
  ownership through the shared journaled initialization mount. Targeted native
  core checks and 10 DragonFly unit tests passed (2 existing ignored tests).
- Full downloaded-payload Auto-open, append and second-reopen checks passed for
  NTFS/exFAT with both backends using passwords, and ext4 with both backends using
  binary keyfiles. Payload writes ran as uid 1300. These are development candidate
  receipts with recorded source input manifests and binary hashes; complete
  frozen-commit qualification remains pending.
- Filesystem tools are installed as host prerequisites outside Lurker's private
  prefix. Both original filesystem packages are preserved as rollback archives.
  Patch, source, package, binary and dependency identities are in `build/` and
  `inventory/`; no cipher, KDF or encryption default was weakened.
- Official sudo was installed for the desktop helper path. A temporary narrow
  sudoers policy permits only this round's administrator-controlled helper paths;
  its removal remains part of cleanup. Helper capabilities run as the login user
  through that policy; full frontend lifecycle validation remains pending.

## Initial applicability reconciliation

`applicability.json` now accounts for all 130 initial container cells: 10 passed,
45 unavailable through assessed ordinary native routes, 70 not applicable, and
5 blocked pending CryFS native-source work. Every cell carries its filesystem
role, source/evidence identities and eligibility or driver reason. These counts
are initial assessment; every final-candidate cell remains pending. Device
results are a separate dimension.

The five writable native filesystem rows are UFS1, HAMMER2, ext4, NTFS and exFAT.
Btrfs, XFS, APFS, UFS2 and ZFS have no qualified native writable creation route
on this host. Reader libraries, converters and unrelated package names do not
supply that route. LUKS2, gocryptfs and Cryptomator have no qualified ordinary
native engine/provider artifact; this is not a claim that a source port is
impossible. OS-specific facilities from the other BSDs and macOS are marked not
applicable with their owning-OS reason.

CryFS 1.0.3 source pin `5d0cc36785d93e12250696a3ffc928042f2e8d61`, its tag and
submodules were freshly verified. Native CMake configuration passed with official
DragonFly Boost, range-v3, spdlog and FUSE 2 packages. Source inspection found a
missing DragonFly guard for the native `sysconf` physical-memory API. A minimal
platform patch and the existing common durability patch are prepared for a
serialized diagnostic build after the stock base build. No CryFS runtime or
product pass is claimed.

An additional ext4/LUKS1/keyfile/Auto smoke passed with bounded payload workers,
an exact native session/mount match and independent ext4 extents-superblock
proof. Its plaintext root was uid/gid 1300 and mode 0700. All downloaded payload
hashes survived both normal reopens; its mount closed and only the successful
fixture was reclaimed. Receipt: `cases/bounded-runner-smoke-receipt.json`.

## Native scratch-device controls and identity guards

The new 1 GiB qcow2 is attached as libvirt `vda` at PCI `0:5.0`, requested
serial `lurker-fresh-dragonflybsd`:
`/srv/build/lurker-native-fresh-20261127/dragonflybsd/scratch.qcow2`.
After the ordinary updated-base reboot, native boot evidence linked that PCI
address to `virtio_pci1`, `vtblk0` and `/dev/vbd0`. The exact 1 GiB/512-byte
geometry, empty partition inventory and zero header/trailer samples were recorded
before the first write. The stock virtio driver does not expose its requested
serial in guest disk metadata; PCI topology and geometry provide the link.
No existing disk is a test target.

A native GPT data partition `/dev/vbd0s1` now has offset 1048576, size 1071644672
bytes and independently recorded DIOCGPART storage UUID bytes
`529f6ca8ebe9f1119752010000000000`. Native raw controls passed separately:

- LUKS1: approved AES-XTS/512-bit key, SHA-256 and 5000 ms native formatting;
  direct native cryptsetup open, UFS1 creation, uid 1300 payload write, normal
  close/reopen/hash, append/new file and second close/reopen/hash.
- VeraCrypt: freshly user-built pinned tcplay diagnostic with only the exact
  verified raw path added to its device filter; administrator-controlled staged
  executable, full 1022 MiB random initialization and successful raw-device fsync,
  fixed SHA-512/PIM 0/500000 profile, native mapping, UFS1 and the same complete
  downloaded-payload lifecycle. This diagnostic is not a product adapter pass.

Both controls checked real mounts and UFS1 superblocks. Their mounts and mappings
closed. Receipts: `device/native-luks1/receipt.json` and
`device/native-veracrypt/receipt.json`.

`cb5d22b` adds native GPT partition inspection and idle guards, while physical
creation remains disabled. It checks the canonical root-owned hardware node,
measured native geometry ABI, independent GPT row, stable storage UUID,
path/descriptor identity, and conservative whole-disk mount, swap, key-free
mapper dependencies and open raw descriptors. Native targeted core checks passed
14 tests; a separate administrator-controlled native test passed expected GPT
identity, held-descriptor refusal, idle recovery and system/whole-node rejection.
Sampled ciphertext header/trailer hashes stayed unchanged.

A filename-filtered `fstat` misses raw descriptors on this OS. A fresh positive
control established that a full numeric file-table scan exposes their device
cookies; the adapter uses that route, including native padding before the offset
colon. The first conservative parser refusal is retained with its fix and rerun.

A separate native directory-backend process probe passed precise PID/start
second+microsecond identity and live, exited-unreaped and reaped/missing states.
Current `kinfo_proc` is 1048 bytes; libc 0.2.191 describes a 992-byte older tail,
whose query failed with ENOMEM. The measured extension and raw primitive-only
field reads passed matching C/Rust offsets and widths. No whole Rust enum-bearing
structure is initialized from kernel bytes. Native `waitid(WNOWAIT)` proves the
unreaped child; `KERN_PROC_PID` returns explicit zero bytes for that zombie.
Directory-backend product integration is still pending.

## Initial physical product matrix

`6a4b88c` implements physical LUKS1/VeraCrypt creation and reopen using the
verified GPT partition, native geometry instead of character-node file length,
sector-aligned Auto detection and identity-checked raw flushes. LUKS1 retains its
journaled private metadata activation layer. The tcplay vnode protocol stays 2;
physical creation additionally requires the measured `physical-protocol` 1
extension before random initialization. An older wrapper was refused with
unchanged sampled ciphertext headers, and the qualified wrapper was restored.
Legacy LUKS1/UFS1 and VeraCrypt/FAT32 file payload regressions also passed.

API admission `34872dd` passed 13 native API tests. The frozen combined development
inputs have ID `b2833543ae057502ee3b3bd423153347788c2e22d23206acb630f3916ecd71a5`;
CLI SHA-256 `715035f34734b43b58a8c8286df7d83712d10d3dbdf36024c613e693270f2273`.
All ten explicit physical cases passed: LUKS1 and VeraCrypt each with UFS1,
HAMMER2, ext4, NTFS and exFAT. Cases alternated password/keyfile and explicit/Auto
open, used the full downloaded payload as uid 1300 and verified both normal
reopens, including append/new-file persistence. Native mount, superblock and GPT
identity proofs are in every receipt. All mounts and mappings closed.

`device/applicability.json` separately reconciles 130 device cells: 10 initial
passes, 45 ordinary-route unavailable, 70 not applicable and five CryFS cells
blocked pending native work. Directory rows refer to physical backing storage.
These development results do not replace final frozen-candidate validation or
broader cipher/credential, frontend and reboot checks.

## Current continuation

Native desktop validation from coordinator base `cca3c17` and a frozen six-file
overlay passed `cargo check --workspace`, the complete workspace test suite
(127 passed, five explicitly ignored), and all three product builds. One Cargo
worker, no incremental/debug data and disk guards were used. A first background
launcher inherited ignored SIGINT/SIGQUIT and invalidated a tool-timeout test;
resetting those dispositions allowed the complete suite to pass without a source
change. The original failure remains in the evidence.

All 16 DragonFly physical forms rendered at native scales and were reviewed.
Both opt-in actual GUI/helper lifecycle callbacks executed as uid 1300 on the
verified GPT scratch partition: LUKS1 and VeraCrypt each passed create, mount,
write, close, reopen and compare. The first private callback runner omitted the
ignored-test opt-in; that invalid receipt is retained separately and carries no
runtime credit. The corrected runner requires one executed pass and zero ignored
tests. A new trusted read-only identity/idle probe preceded these callbacks.
Helper SHA-256:
`20357c79765cbced2b503d5522744fdb3ee6dc836513b7e06faa6a260901981e`;
desktop SHA-256:
`35afa96e47eea64911feaa7a183e84a1abfa136f230a578c2823c76e3e87b3a8`.

The canonical native stager collected Rust notices, verified the pinned private
tcplay runtime and native library dependencies, and built a package selecting
LUKS1 and VeraCrypt. Source-manifest SHA-256:
`d8e350e9354ad21ea77dc7180b58400cbb2909ad80744ee8d20723aaaaf0f7c0`;
archive SHA-256:
`fc32381c4d87021cfc1fd572f11bc7829621450ce5e5b0135071a73219f9902f`.
All 847 manifested files were verified when copying into the root-owned candidate
prefix. This remains an explicitly recorded development overlay, not a final
frozen-commit pass. A real login-user winit/software X11 window also passed at 1280 × 860,
with 1,092 sampled colors, no loader overrides, and all owned children reaped.
Pre-existing unregistered zero-filled X11 tool files were preserved and replaced
with official native packages; missing ordinary cursor/keyboard libraries were
installed. CryFS compilation follows this package/callback/window gate. A bounded Go-FUSE transport assessment follows those
priorities. No custom kernel feature work has started.

Final frozen-commit workspace check/test/build, all cipher/credential and
conformance variations, ordinary mounted-vault reboot persistence, helper/Slint
frontend/package qualification, final 130-cell receipts and cleanup remain
pending. Initial assessment and native controls do not substitute for those
final product passes.
