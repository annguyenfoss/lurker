# Linux filesystem round results

The ten-filesystem Linux implementation and native qualification are complete on six independent guests. Results below describe small disposable containers with PDF and UTF-8 text, ordinary close/reopen, mutation and hash verification. They do not qualify crash recovery, durability, ACL fidelity, snapshots or production reliability of experimental drivers. No SQLite, GUI, physical-device, fault, kernel replacement or upstream source patch tests were run.

## Observed filesystem matrix

`RW4` means native LUKS2, LUKS1 and privileged VeraCrypt plus login-user VeraCrypt/LKL passed. `RW3` means the three privileged encryption routes passed; the installed LKL route does not implement that filesystem. `RO2` means independently formatted imports passed through LUKS1 and privileged VeraCrypt; default LUKS2 is blocked as described below. `RO3` means all three privileged read-only imports passed. These are observed routes, not interchangeable filesystem feature sets.

| Filesystem | Arch | Debian | Ubuntu | Fedora | openSUSE | Gentoo |
|---|---|---|---|---|---|---|
| ext4 | RW4 | RW4 | RW4 | RW4 | RW4 | RW4 |
| Btrfs | RW4 | RW4 | RW4 | RW4 | RW4 | RW4 |
| XFS | RW4 | RW4 | RW4 | RW4 | RW4 | RW4 |
| APFS | RW3 | RW3 | RW3 | RW3 | RW3 | RO3 |
| UFS1 | RO2 | RO2 | RO2 | RO2 | RO2 | RO2 |
| UFS2 | RW3 | RW3 | RW3 | RW3 | RW3 | RW3 |
| HAMMER2 | RO3 | RO3 | RW3 | RO3 | RO3 | RO3 |
| NTFS | RW4 | RW4 | RW4 | RW4 | RW4 | RW4 |
| exFAT | RW4 | RW4 | RW4 | RW4 | RW4 | RW4 |
| ZFS | RW3 | RW3 | RW3 | RW3 | ABI blocked | Kernel SDK missing |

The effective ten-filesystem pair results retain **324 passed and 18 failed** receipts after actual correction replays. Each host's three remaining failures are CryFS on exFAT, CryFS on UFS2 and the UFS1/default-LUKS2 read-only import. Unavailable create/write cells are documented below and are not counted as executed pairs.

| Host | Passed | Failed with recorded limit |
|---|---:|---:|
| Arch | 56 | 3 |
| Debian | 56 | 3 |
| Ubuntu | 59 | 3 |
| Fedora | 56 | 3 |
| openSUSE | 50 | 3 |
| Gentoo | 47 | 3 |

Directory encryption was tested separately with the encrypted source on each supported writable backing filesystem and plaintext mounts in a private directory on the host filesystem. gocryptfs and Cryptomator passed on every writable filesystem in the table. CryFS passed on ext4, Btrfs, XFS, NTFS, APFS, HAMMER2 and ZFS where writable; it failed on exFAT and UFS2 on every host. No directory backing cases were attempted for read-only UFS1 or Gentoo APFS. These cases do not assign an alternate inner filesystem to a directory engine.

## Identities and receipts

Remote jobs are terminal. Exact source archives were deployed and CLI/helper built independently on each guest with Rust 1.100.0 and the existing shared Cargo cache:

- Base: commit `6d10cfa`, `/tmp/lurker-filesystem-round-HOST-6d10cfa/`, `cases/matrix.json`.
- First corrections/supplements: `ec2add1`, `/tmp/lurker-filesystem-round-HOST-ec2add1/`, `native/matrix.json`, `user/matrix.json`.
- Final corrections/supplements: `c70f8a8d9a73c6861b779d692d6792a1e8f7c55e`, `/tmp/lurker-filesystem-round-HOST-c70f8a8/`, `native/matrix.json`, `ufs1-imports/imports.json` or `apfs-imports/imports.json`, terminal receipts and logs. HAMMER2 RO adapter/probe commits are `ca282e3` and `ee3ab7c`; those final jobs are `/tmp/lurker-filesystem-round-HOST-ee3ab7c/`, with payload receipts under `~/.cache/lurker/filesystem-round/hammer2-imports-ee3ab7c/imports.json` and installed prefix `/usr/local/lib/lurker-filesystem-round-ee3ab7c/`. Gentoo's earlier UFS1 receipt is `/tmp/lurker-filesystem-round-gentoo-ufs1-import/imports.json`.

`HOST` is `archlinux`, `debian`, `ubuntu`, `fedora`, `opensuse` or `gentoo`. Each payload receipt records source commit, CLI SHA-256, actions, payload hashes and mount cleanup; later receipts also record initial root uid/gid/mode. Final CLI/helper/harness hashes, administrator ownership, complete per-pair receipts, module state and reconciliation are collected locally under ignored `target/goal-validation/filesystem-round/linux/HOST-final-results.json`, `HOST-effective-pairs.json` and `HOST-final-reconciliation.json`. Archives, archive hashes, native imports, tool provenance, package/source build commands and full logs remain in that directory. Earlier failures remain visible and are superseded only by actual successful replays of the same pair.

The installed final prefix is administrator-owned `/usr/local/lib/lurker-filesystem-round-c70f8a8/`. Private privileged engines remain under administrator-owned `/usr/local/lib/lurker-qualification-2ebae8f/libexec/lurker/backends`. Login-user VeraCrypt explicitly uses the existing transport-4 helpers and Linux 6.12.118 LKL receipt under `~/.cache/lurker/final-14ede46/veracrypt-input`; the older transport-3 installed helpers were safely rejected and were not relabeled as passing. FAT32 is auxiliary historical regression evidence, excluded from the promised ten-filesystem totals; its base run retained 29 passes and 13 failures without further expansion.

## Installed providers and bounded unavailable routes

Guest kernels remained unchanged: Arch 7.2.15-arch1-1, Debian 6.12.118+deb13-cloud-amd64, Ubuntu 7.3.0-10-generic, Fedora 7.2.11-300.fc45.x86_64, openSUSE 6.12.0-160100.8-default and Gentoo 6.18.62. Native e2fsprogs, btrfs-progs, xfsprogs, exfatprogs and NTFS-3G/mkntfs were installed on all six.

- **APFS:** unmodified linux-apfs-rw v0.3.22, commit `80f5db04ecd3b240fee4c67829a5b5465f11f63f`, built against matching installed headers on five guests. Arch required matching headers from the official package archive; openSUSE required its exact `.8.1` SDK rather than the newer generic package selection. openSUSE explicitly loaded the unsupported module with the vendor flag; no global unsupported-module policy was changed. apfsprogs packages or unmodified v0.2.1 provide `mkapfs`. Native creation requires the experimental `readwrite` option and bypasses distribution `mount.apfs` helpers. Gentoo lacks a complete matching configured kernel build tree/export map. Unmodified apfs-fuse builds on Gentoo/openSUSE hit upstream C++ errors and were not patched. Gentoo instead built unmodified libfsapfs 20261030 and installed its reader under root-owned `/opt/lurker-libfsapfs-20261030`; `fsapfsmount` passed all three CLI RO imports. openSUSE's packaged reader also passed an independent raw RO control with caller ownership/0700 options.
- **UFS1:** native kernel reader only, variant `44bsd`; creation and writable unlock are refused. Fedora installed the exact kernel-modules-extra package. Debian and openSUSE built only the unmodified upstream v6.12.118 `fs/ufs` module against matching installed headers, without rebuilding/replacing their kernels. Upstream source commit is `b869a5313cbfe7102873d3de318ddb68675146d0`; openSUSE required `modprobe --ignore-install --allow-unsupported ufs` because its vendor install hook dropped the explicit flag. Other guests used shipped modules. All six failed the default LUKS2 import because the native `44bsd` UFS probe starts with a 512-byte block size while that mapper exposes 4096-byte logical sectors. LUKS1 and privileged VeraCrypt expose suitable geometry and passed. Defaults were not changed to force a compatibility pass.
- **UFS2:** unmodified realchonk/fuse-ufs 0.6.0 and portable kusumi/makefs built on all six; missing FUSE3/SSL/UUID SDK packages were installed. The trusted formatter is explicitly named `makefs-freebsd`, avoiding Debian/Ubuntu's incompatible NetBSD makefs output. Creation uses little-endian FreeBSD FFSv2, 32768-byte blocks and 4096-byte fragments and validates the new root through the same driver before publishing. FUSE support is experimental. Its missing hardlink support blocks CryFS publication; gocryptfs and Cryptomator passed.
- **HAMMER2:** Ubuntu's unmodified upstream Linux module and hammer2-utils built and passed three native plus three directory cases. The writable Linux port requires kernel 7.3; all five other unchanged guest kernels are below that floor. Unmodified kusumi/hammer2-fuse 0.5.0, commit `b92433c72dd0925e2ca21e8d06b3183ecb0559ca`, built independently on those five and supplies a read-only import route. An independent 512 MiB Ubuntu-native fixture passed raw reader control and the three encryption imports. Older util-linux lacks a HAMMER2 probe; Lurker uses a bounded eight-byte native magic read from the owned mapper before selecting the trusted reader. The initial import attempt exposed this detection gap and insufficient tmpfs space; closed failed fixtures were retained at home and only those failed pairs were replayed with the corrected adapter under a home-backed root. No kernel upgrade, compatibility patch or writable bridge was attempted.
- **ZFS:** Debian installed official contrib OpenZFS 2.3.9 DKMS; Ubuntu used packaged 2.4.4. Arch and Fedora built unmodified OpenZFS 2.4.4 against exact installed SDKs, installed administrator-controlled modules/tools under `/opt/lurker-openzfs-2.4.4`, and loaded them. openSUSE attempted the same unmodified release against its exact vendor SDK; configuration failed with `None of the expected lookup_bdev() interfaces were detected`. This is a concrete kernel API incompatibility, not package absence. Gentoo's installed kernel build links are dangling/incomplete with no matching `.config`/`Module.symvers`; its ordinary installation attempt is prerequisite-blocked. Neither case triggered source patches or kernel rebuilding. ZFS filesystem use does not add a ZFS-native encryption backend.

## Implementation and limits

Explicit formatting leaves the Btrfs default, compression and cipher/KDF profiles unchanged. New POSIX roots belong to the authorized caller with mode 0700; ext4 uses `root_owner` plus a private initialization mount because older e2fsprogs lacks `root_perms`. XFS uses its root prototype. NTFS-3G, exFAT and FAT views use caller uid/gid and umask 0077; these synthetic permissions do not imply POSIX ACL semantics. Existing roots are not changed during unlock. Private initialization checks the opened directory's actual mount ID before metadata changes and rechecks the visible mount before ordinary unmount.

Native APFS/HAMMER2/ZFS module-loading/signature prerequisites are checked before storage mutation. UFS variants use low-level blkid probing. Trusted native FUSE readers/writers use the exact mapper source and validated mount identity; their stdout is captured so formatter diagnostics cannot corrupt the helper JSON protocol. APFS forensic reads use explicit RO, caller ownership and umask options. Failed staged ciphertext remains recoverable while an associated mapper exists.

ZFS pools are restricted to one verified mapped leaf, a generated pool name and no persistent cachefile. Label topology is checked before import; import searches only the exact mapped device, uses the numeric pool identity and never force-imports. Post-import topology is verified. Export refuses extra root/child/snapshot/alias mounts rather than auto-unmounting unrecorded storage. No snapshots, native ZFS encryption, multi-device pools or broader dataset lifecycle are claimed.

CryFS exFAT fails with `publish new encrypted block: Operation not permitted`; its integrated atomic publication requires hardlinks. UFS2 reports the same publication failure; the current unmodified FUSE provider has no hardlink handler. UFS2 itself supports hardlinks, so this limit belongs to the selected provider and integrated publication path. Failed backing ciphertext and engine diagnostics are retained. The earlier APFS/HAMMER2 fusermount whitelist failures were resolved by the shared Linux private initialization path and separate plaintext mount root, then replayed successfully. The initial openSUSE password-quality and ext4 formatter failures were corrected without changing host password policy or encryption defaults.

## Verification and handoff

Final remote Arch core/CLI/helper tests: **108 passed, zero failed, 12 ignored**. Scoped `cargo check --locked -p lurker-core -p lurker-cli -p lurker-helper`, the meaningful OSC3008 PTY regression and Python compilation all pass. Native CLI/helper builds passed separately on all six. Full workspace/GUI compilation was not exercised: there is no warm Slint cache and the approved round is CLI/container-only. The common fixture is a valid one-page PDF 1.4 (SHA-256 `f62d9bb8cdf4722e68937ce0fd939201585a74e28ea839b3bc1441a2dd29597b`) plus UTF-8 text; read-only imports verify both payload and unchanged ciphertext.

Cleanup uses only ordinary owned-volume closes/unmounts. Closed passed scratch images are removed where needed for bounded disk usage; failed fixtures, receipts, hashes, installed packages/modules and shared caches remain. Final reconciliation records no owned mounts, crypto mappers, loop attachments or imported pools. No detached round coordinator remains active. Six-host ownership is released to the independent Linux reviewer; review should inspect actual per-pair receipts, provider build decisions and preserved upstream limits rather than repeat the complete matrix.

Primary provider sources:

https://github.com/linux-apfs/linux-apfs-rw

https://github.com/linux-apfs/apfsprogs

https://github.com/libyal/libfsapfs

https://github.com/realchonk/fuse-ufs

https://github.com/kusumi/makefs

https://github.com/jdmanring/linux_hammer2

https://github.com/kusumi/hammer2-utils

https://github.com/kusumi/hammer2-fuse

https://github.com/openzfs/zfs

## Independent review, 2026-11-25

Reviewed the Linux adapters through `ee3ab7c` and their unchanged native source
at the review head. No serious product defect required a patch. Reconstructed
the effective matrix directly from every base, supplement and final/import
receipt, then compared it with the published effective-pair index: **342
executed pairs, zero receipt inconsistencies**. Every passing receipt has the
required successful CLI actions, PDF/text hashes and ordinary close. Read-only
imports additionally verify both crypto/filesystem RO and unchanged ciphertext.

| Host | Container RW passed | Directory passed / limited | RO passed / limited |
|---|---:|---:|---:|
| Arch | 29 | 22 / 2 | 5 / 1 |
| Debian | 29 | 22 / 2 | 5 / 1 |
| Ubuntu | 32 | 25 / 2 | 2 / 1 |
| Fedora | 29 | 22 / 2 | 5 / 1 |
| openSUSE | 26 | 19 / 2 | 5 / 1 |
| Gentoo | 23 | 16 / 2 | 8 / 1 |
| Total | 168 | 126 / 12 | 30 / 6 |

These remain **324 passed, 18 limited failures**, excluding FAT32 and unavailable
or inapplicable pairs. The six UFS1/default-LUKS2 geometry failures and twelve
CryFS backing-store failures remain failures; review does not promote them.
The documented Gentoo SDK, openSUSE ZFS ABI and older-kernel HAMMER2 write
limits remain unchanged.

Live identity checks matched all **23 retained installed CLI paths** to their
receipt SHA-256 and administrator ownership. All **92 inspected native adapter
and harness source files** matched their exact Git inputs: `6d10cfa`, `ec2add1`,
`c70f8a8` and `ee3ab7c`. Earlier passing receipts retain those original identities.
The Ubuntu writable HAMMER2 route does not use the later reader-only artifact.

To cover later initialization changes without repeating the matrix, replayed
LUKS2/ext4 on each guest, LUKS2/HAMMER2 on Ubuntu, and LUKS2/UFS2 on Arch and
openSUSE using the existing `c70f8a8` artifacts: **9 passed, zero failed**.
Each completed create and three ordinary mount/close cycles with original,
appended and added-file hashes; all new roots were `1300:1300`, mode `0700`.
These replays supplement affected evidence and do not increase the distinct
pair totals above. No new build or broader workspace bootstrap was necessary.

Ignored local evidence is indexed by
`target/goal-validation/filesystem-round/linux/review-index.json`, with the
complete reconciled counts, original identities, input hashes and cleanup in
`review-receipt.json`. Per-host details are `HOST-review-identities.json`,
`HOST-review-reruns.json` and `HOST-review-cleanup.json`. Remote replay receipts
remain under `~/.cache/lurker/filesystem-round-review/`; closed successful review
images were removed after checking mount/loop inventory. Live final inventory
on all six guests contains **zero owned mounts, mappers, loop attachments,
imported pools or workers**. Original failed fixtures, installed providers and
shared caches remain preserved. All six hosts are released.
