# Cross-platform implementation status

Active non-macOS qualification, 2026-11-20. The coordinated native package
results and remaining release gates apply to this round. Earlier checkpoints
retain their original dates and source revisions. The approved profiles are
in `DIRECTIVES/LLM/ENCRYPTION.md`.

## DragonFly VeraCrypt candidate, 2026-11-23

Qualification started on `dragonflybsd-core-01` with DragonFly 6.4.2 x86-64.
Stock `tcplay` 3.3 created, opened, wrote and reopened disposable AES/SHA512-VC
images with a password and a binary keyfile. Its root version probe changed
`kern.cryptodevallowsoft` from `0` to `1`. The test restored `0`. The private
userspace wrapper left the value at `0` through its six cipher and credential
round trips.

The initial offline CLI package `46e13649a466c56da85728fbd616a1164195a515`
(`3a9f3e34c9bf018b30fea7336d2661e01dd291245d92a71486f59b7bb241276b`)
installed and verified its package files. Its 512-byte FAT32 cluster layout
returned different payload bytes after an ordinary close and reopen in all
six cipher and credential cases. A matched stock `tcplay` control reproduced
the mismatch with and without a synchronous mount. Plain FAT32 retained the
expected bytes in both modes. The stock `tcplay` 4 KiB cluster controls both
preserved the four test payload files across close and reopen. The candidate
now requires FAT32 clusters of at least 4 KiB and a 4 KiB-aligned data region.

On DragonFly 6.4.2, both plain FAT32 and a stock `tcplay` FAT32 mapping accepted
file `fsync`, rejected directory `fsync` with `EINVAL`, and failed SQLite WAL
and rollback-journal mode with `SQLITE_IOERR_LOCK`. A native linear mapper
created with `--readonly` accepted a direct write and changed the backing
image digest. The candidate rejects VeraCrypt read-only requests before vnode
attachment. The application and power-loss gates remain open and release
support remains planned. Test receipts and disposable fixture locations are
under `target/dragonfly-tcplay/evidence/`.

The aligned candidate `5786867a17252ea95059c3922944de8a8ac9c773`
passed `cargo check --workspace --locked` and 96 workspace tests with four
ignored on `dragonflybsd-core-01` in run `20261123T121207Z-42cce614`. The
packaging checks passed 50 tests. The fault harness passed 36 tests after
supplying the guest's separately packaged Python SQLite extension.

Its CLI and desktop package SHA-256 was
`95e384feb463fb9387da5ea1a757c7e11b29a116221aeb7e602bd21559aecc10`.
An isolated DragonFly clone installed it offline and verified its package
inventory and base-library resolution. The clone stopped returning SSH
banners during the aligned runtime suite, which ended without a complete
receipt. On the named host, the separately staged CLI from the same source
completed the AES/password case and verified all five payload hashes across
explicit and automatic opens. Directory `fsync` still failed. Those two
guests gave no complete aligned matrix receipt.

A separate isolated guest booted the stock 6.4.2 kernel from a hash-verified
clean archive. It rebuilt the wrapper offline from `5786867a1725` and matched
the installed wrapper SHA-256. That binary rejected upstream Argon2id,
BLAKE2s, Camellia, PIM 501, SHA-256, Whirlpool, an AES–Twofish cascade, and an
80-byte password. The upstream AES/SHA-512/PIM 0 control authenticated and
mapped at byte offset 131072 with a 66846720-byte payload. Each test retained
its ciphertext hash and restored the vnode, mapper and mount inventories.
`kern.cryptodevallowsoft` stayed at `0`. Build and rejection logs are in
`target/dragonfly-tcplay/evidence/archive-recovery-wrapper-*.log`.

The exact CLI and desktop package was recovered from a read-only copy of the
stalled clone's HAMMER2 disk and installed offline on the clean UFS-backed
guest. `pkg check -s` and `pkg check -d` passed, and the installed product and
wrapper hashes matched. The first replay used the old 64 MiB fixtures and
received the expected 260 MiB minimum error in every case.

At the corrected 260 MiB size, the installed CLI passed the lifecycle for all
six cipher and credential pairs. The 121 recorded steps covered explicit and
Auto open, wrong credentials, the 64/65-byte password boundary, alias and
busy-close rejection, close by mountpoint and source, and five payload hashes
per pair across close, append, and reopen. The vnode, mapper, mount and
session inventories returned to baseline with `kern.cryptodevallowsoft=0`.
All six directory `fsync` checks still failed. The raw receipt is
`target/dragonfly-tcplay/evidence/archive-recovery-installed-aligned-matrix.json`.
The named host's follow-up suite remained inaccessible over SSH.

The installed package and Linux VeraCrypt 1.26.31 exchanged writable FAT32
containers in both directions for AES, Serpent, and Twofish. Linux verified
the DragonFly marker in each upstream-created image. For each Lurker-created
image, Linux verified all five post-append payload hashes, wrote a marker,
and returned the image. The installed CLI reopened each through Auto,
verified the Linux marker and closed with the same ciphertext hash Linux
recorded. The Linux console SHA-256 is
`ed01a2348e21d3b760fe35564338a98160db427e610379088f715c90f7a37eaf`.
The three receipts are `archive-linux-forward.log`,
`archive-linux-upstream-retry.log`, and `archive-linux-reverse.log` under
`target/dragonfly-tcplay/evidence/`. Windows is unverified.

OpenBSD 8.0 VeraCrypt 1.26.31 and the installed DragonFly CLI exchanged AES
containers in both directions. OpenBSD created a FAT32 volume with 4 KiB
clusters and a 565248-byte data offset, then verified DragonFly's marker.
OpenBSD also verified all five payload hashes in a DragonFly-created image,
wrote a marker, and returned it. DragonFly reopened it through Auto, verified
the marker, and matched the ciphertext digest recorded by OpenBSD. Both
native inventories returned to baseline. The OpenBSD console SHA-256 is
`2429fb0658331b953de7abe27920480f6d7ddfe32931ccd8aff81c9016e46272`.
Receipts are `openbsd-aligned-origin.log` and `archive-openbsd-*.log` under
`target/dragonfly-tcplay/evidence/`.

FreeBSD 15.1-RELEASE-p7 and macOS 27.2.1 passed the same two-way AES exchange
with the installed DragonFly package. Each VeraCrypt 1.26.31 console created
an aligned FAT32 image with 4 KiB clusters and verified DragonFly's marker.
Each verified all five payload hashes in a Lurker-created image, wrote a
marker, and sent back the resulting ciphertext. The installed CLI verified
both markers on Auto reopen and matched both returned ciphertext digests.
The native mount and device inventories returned to baseline. The FreeBSD
console SHA-256 is
`92edaf1363133aab8844ddd2c01656174967e260cd046569bd2dcbac64eec8b4`.
The macOS console SHA-256 is
`c5b2420f651e48d59a9bc602076a6e2aaee99e62bb025ef1b800e2992b9e466f`.
Receipts are under `target/dragonfly-tcplay/evidence/` in the
`freebsd-aligned-origin*.log`, `macos-aligned-origin.log`,
`archive-freebsd-*.log`, and `archive-macos-*.log` files.

The installed CLI rejected the eight incompatible inputs above and a
compatible AES header with unqualified FAT32 geometry. Each of the nine
fixtures was tested with explicit VeraCrypt and Auto open. All 18 rejections
preserved the source digests and restored the native inventories.
Opening without a credential or controlling terminal also failed in both
modes with the source digest and native inventory intact. The receipt is
`target/dragonfly-tcplay/evidence/archive-missing-credential-retry.log`.

The installed helper ran as login UID 1300 through scoped sudo authorization.
It created an invoking-user-owned image, refused read-only before attachment,
and exposed the active FAT32 mount with mode `0700` in the authorized volume
list. An open working directory blocked close. After releasing it, the helper
closed the mount, reopened it through Auto, and verified the written payload
hash. The temporary sudo rule was removed and native inventories returned to
baseline. An ASCII file `fsync` succeeded. Directory `fsync` returned `EINVAL`
and SQLite 3.54.1 returned `SQLITE_IOERR_LOCK` in both WAL and rollback-journal
mode. The required power-loss workload could not be armed. The receipts are
`archive-installed-import-rejections.log` and `archive-installed-login-helper.log`
under `target/dragonfly-tcplay/evidence/`.

The standard power-loss worker on the UFS-backed clone reached the same
directory `fsync` failure after writing and syncing its file and renaming it.
The worker exited without a readiness record. The harness uses 260 MiB for
DragonFly VeraCrypt to satisfy the aligned FAT32 minimum. All 36 harness
unit checks passed on the guest. The receipts are `archive-powerloss-arming.log`
and `archive-powerloss-worker-failure.log` under `target/dragonfly-tcplay/evidence/`.

The installed Slint desktop opened a 1280 × 860 software-rendered X11 window
as UID 1300 using the verified Xvfb archive. The first probe lacked `xdpyinfo`
and stopped before launch. The retry with the offline X11 test tools captured
the window and reaped its processes. The receipt is
`target/dragonfly-tcplay/evidence/archive-installed-desktop-window.json`.

The native desktop callback test recovered from a read-only copy of the
primary guest ran as UID 1300 against the same installed package helper.
It exercised the Slint create and manage callbacks, authorized list and
mount, a synced 1 MiB write, close, reopen, and payload verification. The
test passed and cleanup removed the temporary authorization and native
storage resources. The test executable SHA-256 is
`7f1b75cae2435267f32ddd97529df1d12011562d6af99e3b0589eac016434a54`.
The receipts are `archive-desktop-callback*.log` under
`target/dragonfly-tcplay/evidence/`. The same read-only copy held a partial
primary CLI matrix receipt with 12 recorded steps in its AES/password case.
The complete installed matrix remains the one from the UFS clone.

Package removal deleted the candidate CLI, helper, desktop, wrapper and
manifest. The other package rows, the guest's original package database
digest, the sample ciphertext and the native storage inventories matched
their pre-uninstall values. Reinstalling the same offline archive restored
the previous package inventory and passed file and dependency checks. The
receipt is `target/dragonfly-tcplay/evidence/archive-package-uninstall.log`.

The isolated UFS-backed guest performed an ordinary shutdown and reboot
with a candidate vault mounted. The guest reported a new boot time and
idle vnode and mapper inventories. The same installed CLI reopened the
vault through Auto, verified the fsynced file's SHA-256, and closed it
cleanly. This receipt covers ordinary file persistence. SQLite durability
remains blocked by the FAT32 locking failure. Receipts are
`target/dragonfly-tcplay/evidence/archive-ordinary-*.log`.

In the separate busy-shutdown case, a process held its working directory
inside the vault and a close attempt returned `Device busy`. Before the
ordinary reboot, the workload synced a file, replaced another file by
rename, and renamed the enclosing directory. After the boot ID changed,
the installed CLI reopened through Auto and verified both recorded SHA-256
values and the renamed paths. Final storage inventories were clean.
The receipts are `archive-busy-shutdown-*.log` under
`target/dragonfly-tcplay/evidence/`.

The installed CLI opened AES and Serpent vaults concurrently. While both
were mounted, a replacement at the AES source path caused close by source
and by mountpoint to refuse cleanup. The vnode display followed the renamed
file and retained its device and inode. Both sessions remained usable. After
restoring the path, closing AES left Serpent mounted, and both vaults later
reopened with matching marker hashes. The original fixture digests and final
native inventories matched baseline. The receipt is
`target/dragonfly-tcplay/evidence/archive-concurrent-replacement-normalized.log`.

An independent linear mapper survived a normal VeraCrypt close. When the
recorded VeraCrypt mapper was removed outside Lurker, close preserved the
journal. Reusing its name for a linear mapper with a different UUID and
backing file caused both close entry points to refuse cleanup. The foreign
mapping and ciphertext digest were preserved. Restoring the authenticated
mapping allowed Lurker to finish cleanup after the native unmount. The
receipt is `target/dragonfly-tcplay/evidence/archive-foreign-mapper-reused.log`.

All 14 installed CLI interruption cases passed on the isolated DragonFly
6.4.2 UFS guest. The debugger stopped the hash-verified CLI after staging,
vnode attachment, mapper activation, format, mount, unmount, and mapper
removal, once each for `SIGINT` and `SIGKILL`. The harness checked each
stop against the actual native devices and journal before sending the signal.
Interrupted creates preserved the destination inode and bytes. Recovery
closed the owned state, reopens matched the original payload hash, and the
final inventories matched baseline.

The CLI SHA-256 is
`65f25054610b3309f8308bcae350f7834b9b26a98fbcb6f2c57d4f04c35c51ae`.
The executable-relative stop offsets
were `0x5df10`, `0x6a370`, `0x68be0`, `0x5a820`, `0x50bf3`, `0x5aa4d`,
and `0x5ad1a` in phase order. The receipt records the resolved addresses and
observed journal and device state. See
`target/dragonfly-tcplay/evidence/archive-interruptions.json`.

LUKS1 Auto detection on the same installed CLI passed all 13 conformance
checks with stock `cryptsetup` 1.1.2. Final mounts, mappings and vnode
inventories were clean. The receipt is
`target/dragonfly-tcplay/evidence/archive-luks1-auto-regression.json`.

The installed CLI opened a fully allocated 260 MiB image for a bounded FAT32
capacity test. After syncing a 269623296-byte filler, one 4 KiB cluster
remained. A 1 MiB append returned `ENOSPC`. The earlier file hash and the
append target's original prefix survived a close and reopen. Removing the
filler made room for a new synced file that also survived reopen. The final
native inventory matched baseline. The receipt is
`target/dragonfly-tcplay/evidence/archive-inner-capacity.log`.

A separate 360 MiB UFS fixture had 194928640 bytes available when a 260 MiB
VeraCrypt create with `--force` failed. The existing destination retained its
inode and bytes, an unrelated committed file survived, and the staging and
session inventories returned to baseline. Freeing the filler allowed fully
allocated creation and a synced payload survived close and reopen. A write
that would grow the FAT32 file past 4294967295 bytes returned `EFBIG` and
preserved the payload. The receipt is
`target/dragonfly-tcplay/evidence/archive-outer-capacity.log`.

A separate UFS-backed clone with an 8 GiB fixture disk installed the same
offline package and created a fully allocated 4.5 GiB AES image. FAT32 wrote
and synced a 4294967295-byte file. A one-byte append returned `EFBIG`. The
installed CLI closed the vault, reopened through Auto, and read the entire
file back with the same size and SHA-256
`c3fd0d0de3685ca66b9380b150335a4f2ba4b09153f3a25df3206abe0f5603c3`.
The container closed and the dedicated UFS fixture unmounted cleanly.
The receipts are `size-clone-{offline-install,fixture,max-file,cleanup}.log`
under `target/dragonfly-tcplay/evidence/`.

## Measurable completion criteria for the active implementation round

The target is one verified final candidate covering the 16 approved non-macOS
storage routes across Linux and the four BSDs. The Linux kernel and login-user
VeraCrypt routes have separate qualification. The following gates define the
finish line. Record each gate as passed, failed, blocked, or unverified with its
artifact hash and native KVM receipt. Counts describe the existing suites.
Additional regressions required by a fix join the affected gate.

Fault operations may fail explicitly as specified by the recovery contract.
For a full-store append, record any temporary read failure at capacity and
require exact committed bytes after recovery. A failed close must preserve
the backend's nonzero status and recovery evidence. A write or `fsync` that
acknowledges data subsequently lost remains a failed preservation check.

| Gate | Required evidence |
| --- | --- |
| Native installers | Ten installable packages, matching source and dependency manifests, offline runtime, and verified removal |
| Installed storage lifecycle | At least 1,655 existing checks passed, zero failures, across the ten stock guests |
| Linux device creation | All 54 format/cipher profiles passed on fixture-owned loop devices |
| BSD capacity handling | All 81 existing full-storage checks passed, including SQLite full responses and old/new committed data after reopen |
| Directory append preservation | All six full-store and one-free-block cases passed across gocryptfs, CryFS, and Cryptomator, with reported storage failure and original bytes preserved after reopen; a failed close must retain its real failure and recovery evidence |
| Desktop | Ten X11 windows, six Linux Wayland windows, and nine BSD storage callbacks passed through the installed helper |
| Recovery and application behavior | Every applicable case in the plan's Required checks has a receipt, with zero unresolved data-preservation or lifecycle failures across the 16 routes |
| Implementation and packaging regressions | Required workspace checks and targeted native/module/packaging tests passed for changed components, with skipped cases explained |
| Handoff | Committed source, artifact/test index, explicit remaining issues, and restored original guest and storage inventories |

Use the verified `14ede46` artifacts as the starting baseline. Build each
affected component once for a batch of compatible fixes, then reuse those
outputs for the targeted regressions and affected installer matrix. Carry
forward an existing receipt when the relevant implementation, dependencies,
build inputs, and installed artifact are unchanged. Investigate failures
against a named gate. Repeat a build or test only for a changed input, failed
case, or unresolved concern affecting that gate. New optional qualification
work belongs in a later round unless it is required by the approved plan.

Compare OpenBSD VeraCrypt with the unmodified upstream app at the pinned
version on the same guest. Run equivalent lifecycle and persistence checks.
Require Lurker to match the operations upstream passes. A limitation
reproduced in an equivalent upstream workflow can be recorded as shared.
Record the inner filesystem for each result.

The round is complete when every gate passes. An upstream or host defect that
prevents a gate from passing remains an explicit blocker with a reproducer and
the affected route. Reporting that blocker does not establish completion.
Publisher-controlled signing and public release need their own authorized
release operation and are tracked separately from implementation completion.

## Native SSH lifecycle round, 2026-11-22

Each OS exercised the same three PDFs and text page. Fixtures were closed
and reopened before the append and second text file. Verification compared
all five SHA-256 digests after another close and reopen, or after an orderly
reboot with the test vaults still mounted. The package identities differ by
OS and are recorded with each receipt.

| Target | Native payload result | Remaining limitation |
| --- | --- | --- |
| Linux, six distributions | The updated `9ff2b5b` Debian and `16aa1f4` Arch, Fedora, Ubuntu, openSUSE and Gentoo installers passed 1,170 lifecycle checks, 54 loop-device profiles, six Cryptomator mounted reboots, and all 12 X11 and Wayland probes. All six directory append cases passed on Debian. | The 42 mounted reboot cases across all seven routes retain their `14ede46` artifact identities. |
| FreeBSD | The exact `16aa1f4` installer passed 260 checks across 20 variants, all 11 mounted reboot fixtures, four installed-helper callbacks, and X11. | gocryptfs requires explicit journal reconciliation after a reboot with an active mount. Legacy journals with a changed device number require manual recovery. |
| NetBSD | The exact `16aa1f4` TGZ passed 26 CGD lifecycle checks, nine capacity checks, two mounted reboot fixtures, the login-user desktop callback, and X11. | The package was installed under a private destination. The earlier abrupt power-loss WAL result remains open. |
| DragonFly BSD | The exact `16aa1f4` installer passed 117 LUKS1 lifecycle checks, nine capacity checks, X11, and the login-user installed-helper callback. All six mounted cipher and credential profiles reopened after an orderly reboot. | Earlier abrupt power-loss durability remains open. |
| OpenBSD | The `999eced` TGZ passed 212 checks across 16 CLI variants, 27 capacity checks, three login-user installed-helper callbacks, and X11. Both mounted VeraCrypt FFS vaults reopened writable after an ordinary reboot without a shutdown hook, matching the outcome of upstream VeraCrypt's default FAT vault. CRYPTO and RAID1C passed the mounted reboot on the prior installer. | A busy FFS mount can prevent an orderly close. |
| macOS | The exact `c997d6a` DMG passed 320 checks across all 20 CLI profiles, 35 Cryptomator rapid closes, busy refusal, JVM crash recovery, five desktop callbacks, and 75 Firefox checks. | The macOS tests issued no reboot. The final DMG was verified after Buran's separately requested restart. |

All ten KVM guests ran workspace checks and passed 1,234 Rust tests on
`c997d6a`. Buran passed 103 workspace tests at the same revision. Packaged
lifecycle receipts retain their exact source and artifact hashes.

The updated non-macOS installers passed 1,785 lifecycle checks, ten X11
probes, six Wayland probes, and nine BSD installed-helper callbacks.

Receipts and handoffs are in `target/goal-validation/ssh-matrix-20261122/`.
See `docs/linux-validation.md`, `docs/backend-testing.md`,
`docs/macos-backends.md` and `docs/folder-supervision.md` for exact artifacts,
scope and recovery behavior. These results do not close the release gates above.

## Supported routes and power-cut results

The packaged-vault lifecycles in the following sections verify creation,
unlock, writes, clean close, and reopen. Power-cut results below cover the
specific guest, package revision, and workload recorded in
`docs/vm-powerloss-testing.md`. The cut workload includes a synced file,
a committed SQLite FULL/WAL row, and an uncommitted row.

| OS | Routes with packaged lifecycles | Abrupt power-cut evidence |
| --- | --- | --- |
| Linux | LUKS2, LUKS1, privileged VeraCrypt | All three routes recovered the file and committed row on Arch. |
| Linux | Login-user VeraCrypt | Recovered both on Arch with the `1e04b85` full package after fixing the LKL write barrier. |
| Linux | gocryptfs, CryFS, Cryptomator | All three recovered both on Arch. CryFS required the atomic client-state fix. |
| FreeBSD | GELI | The matched `ca3c845` sparse fixture lost acknowledged data. The fully allocated control and a newly created `ae53afb` image with native preallocation each passed all five checks. |
| FreeBSD | VeraCrypt, PEFS, gocryptfs | All three recovered the file and committed row. VeraCrypt needed the ciphertext flush fix. gocryptfs required explicit stale-session reconciliation. |
| OpenBSD | softraid CRYPTO, RAID1C, VeraCrypt | All three needed native FFS preen on separate ciphertext copies before writable reopen. With synchronous FFS, each copy then recovered the file and committed row. |
| NetBSD | CGD | Ordinary application sync left the journal's WAL inode stale and lost the committed SQLite row, including with synchronous WAPBL and writable replay. An explicit WAL range flush persisted the new inode in the journal. Writable replay recovered the file and row on a ciphertext copy. Read-only reopen exposed the stale inode. |
| DragonFly BSD | LUKS1 | Ordinary application sync lost acknowledged writes on sparse and fully allocated images. A single backing-file fsync preserved the file and committed row through read-only UFS in a timing-matched control. Inner UFS fsync ends before issuing a mapper flush. Normal writable reopen refused the unclean filesystem. |
| macOS | Apple encrypted disk images, VeraCrypt, gocryptfs, CryFS, Cryptomator | Historical package results only. This round excluded macOS. |

## DragonFly guarded LUKS1 activation

`e98bcc6` feeds a checked, root-private LUKS1 metadata copy to native
`cryptsetup` through the stock `dm_target_linear` module. The mapping
reads and writes the original ciphertext payload at its existing offsets.
On DragonFly 6.4.2 KVM, workspace check passed and unit tests reported
92 passed and three ignored. The development CLI passed 91 real lifecycle
checks across AES, Serpent and Twofish with passwords and binary keyfiles.
It also reopened an earlier AES vault through Auto in read-only mode.

The native CLI package at `dfc7fdc` verified all 49 installed payload files,
passed 13 installed AES lifecycle checks, and restored the prior package
inventory on removal. The guarded package reopened an established AES
vault after a cut with synchronous inner and outer UFS. Its recovered
ciphertext matched the pre-workload digest and its inner filesystem
was empty. The fully allocated control produced the same result.

The full native CLI and desktop package at `dfc7fdc` matched 645 payload
entries and passed installed checksums on a stock-image KVM clone. The
installed desktop rendered a native X11 window and passed the login-user
storage lifecycle through its helper. The installed CLI passed all 13
AES checks. A separate static source review confirmed the metadata
isolation for the approved profiles and identified further read-only and
interruption checks. Ordinary application durability remains open.
See `docs/backend-testing.md`, `docs/bsd-packaging.md` and
`docs/vm-powerloss-testing.md`.

`8f07cae` checks each native vnode during interrupted activation recovery.
The root-only KVM test found a private metadata vnode beyond a gap in
the device units and confirmed that close preserved unrelated attachments.
`dc458b6` gives DragonFly's `luksFormat` and `luksOpen` five seconds for
cleanup after sending SIGINT on timeout. A native subprocess regression
verified the signal handler and the forced-stop fallback. DragonFly
workspace check passed, with 93 tests passing and four ignored. At the
same revision, core checks and tests passed on FreeBSD, NetBSD and
OpenBSD.

`8143d6c` journals the payload vnode's devfs identity before native
attach. The active-attachment recovery test also verified that reused
unit numbers cannot authorize detach of a later attachment. The exact
native CLI package passed 91 installed lifecycle checks across all
three LUKS1 ciphers, passwords and binary keyfiles, including Auto.

`685f428` extends the same DragonFly reservation to the private LUKS1
metadata vnode. Recovery checks the recorded devfs generation and backing
inode. The native regression recovered the reserved attachment while
preserving another vnode attached to the same file, including on retry.
Older journals retain their validated inode discovery. The DragonFly
workspace passed 94 tests with four ignored. The exact native CLI
package passed all 39 installed AES Auto, Serpent, and Twofish lifecycle
checks, including file and SQLite persistence after reopen. The
alternate package was removed and the native inventories matched their
baselines. Evidence is under
`target/goal-validation/dragonfly-metadata-reservation/`.

## Native BSD container staging

`e607cc7` creates the staging directory privately and anchors image
creation, publication, and cleanup to opened directory descriptors.
Native tools still receive the staged filename after identity checks.
The regression verifies that a moved stage cannot redirect publication
or cleanup into the directory now at its former pathname. NetBSD's
regression also verifies sidecar rollback when the image name is taken.

| KVM target | Workspace tests passed | Ignored |
| --- | ---: | ---: |
| FreeBSD | 113 | 10 |
| NetBSD | 97 | 4 |
| OpenBSD | 112 | 17 |
| DragonFly BSD | 94 | 4 |

All four locked workspace checks passed. The OpenBSD run used the
previously qualified no-debug workspace cache and data limit after
retiring the cold-cache attempt. Logs and source digests are under
`target/goal-validation/bsd-pinned-staging/`.

All four installed native packages at the same revision verified create,
write, close, and reopen. These checks used owned fixtures and restored
the native inventories after removal.

| KVM target | Installed staging qualification |
| --- | --- |
| FreeBSD | 143 GELI and VeraCrypt cases across explicit, Auto and binary-keyfile routes. Each fixture removed its staging directory. |
| NetBSD | 26 explicit and Auto CGD cases. A sidecar-only collision preserved the existing file and left no staged image. Both successful publishes produced mode 0600 images and sidecars. |
| OpenBSD | 13 CRYPTO and 30 RAID1C cases, including invoking-user checks. VeraCrypt also passed login-user writes, SQLite reopen and safe read-only refusal. |
| DragonFly BSD | 26 AES Auto cases across a root-owned and a login-owned destination. The login-owned fixture published the image as UID 1300 with mode 0600. |

`21337f5` cleans up its staging files after publication is refused. The
KVM regressions preserved the existing destination and removed the
owned stage, including after CGD sidecar rollback. All four locked BSD
workspace checks passed again. The exact DragonFly package passed 13
installed AES Auto checks and left no staging directory behind. Logs
and the package receipt are under `target/goal-validation/bsd-publish-cleanup/`.

`b68c8b1` records the requested destination of interrupted BSD file
creates. Explicit unmount can find the session by that path while it
is still absent, verify the staged image and native devices, and close
the session. The command reports where the unpublished image remains.
The native NetBSD and OpenBSD tests used disposable mapped images,
injected a filesystem initialization failure, and verified close by
the requested path and restoration of the prior device inventories.
Locked workspace checks on all four BSDs passed with 416 tests and 35
ignored. Logs are under `target/goal-validation/bsd-create-recovery/`.

`69299a9` syncs the destination directory after publishing the CGD
parameter name and before linking the image. A failed barrier or image
link rolls back the owned sidecar. The NetBSD workspace passed 98 tests
with four ignored, including an injected barrier failure and an image
name collision. A cut between the two name publications can leave only
the sidecar. Evidence is under
`target/goal-validation/netbsd-cgd-publication-barrier/`.

The installed `69299a9` package on stock NetBSD 11.1 passed 26 explicit
and Auto CGD lifecycle checks and seven checks on a vault created after
recovering an interrupted create. Recovery rejected a replaced staged
inode. Both sidecar-only and late image collisions preserved the
preexisting file. A native trace recorded the sidecar link, a successful
directory fsync, and the image link in that order. Evidence is under
`target/netbsd-cgd-destination/`.

The DragonFly BSD CLI package at the same revision passed 39 installed
LUKS1 checks across AES Auto, Serpent and Twofish, including file and SQLite
readback after reopen. Package removal restored the native inventories
and left no staged directories. Receipts are under
`target/goal-validation/bsd-create-recovery/dragonfly-native/`.

`de4345c` records intent before each native CGD or vnode detach on
NetBSD. A retry can clear an old device record when stock NetBSD confirms
that device is inactive. The native tests exercised both detach steps,
required the recorded intent, and preserved live attachments until
ordinary close. NetBSD workspace checks passed with 98 tests and five
ignored. Core checks also passed on FreeBSD, OpenBSD, and DragonFly BSD.
Evidence is under `target/goal-validation/netbsd-native-detach-retry/`.

The exact installed `de4345c` package passed 26 CGD checks on stock
NetBSD 11.1. With `.cgdparams` absent, mount failed with the native
inventories unchanged. Restoring a byte-identical backup recovered the
file and SQLite data on reopen. The package and test guest were removed.
Evidence is under `target/netbsd-cgd-detach-intent/`.

OpenBSD `db993eb` records CRYPTO detach intent before closing the
softraid disk or vnode. It also records the vnode release before Auto
falls back to VeraCrypt. `6fe1f26` records the RAID1C array detach and
each member detach separately. Retry checks the recorded source
identities and native disk or vnode state. Native regressions on
OpenBSD 8.0 recovered completed detaches, required a saved intent,
and restored the device, mount, and registry inventories.
The final workspace passed 112 tests with 19 ignored. Results are under
`target/goal-validation/openbsd-native-detach-retry/` and
`target/goal-validation/openbsd-raid1c-detach-retry/`.

The exact installed `6fe1f26` package passed 13 CRYPTO, 13 Auto VeraCrypt,
and seven normal RAID1C lifecycle checks on stock OpenBSD 8.0. Each route
verified file and SQLite persistence after close and reopen. Package
removal restored the original mount, device, session, and package
inventories. Evidence is under `target/openbsd-crypto-detach-26/`.

The same installer passed nine RAID1C descriptor recovery checks. Auto
refused an owned vault with the descriptor absent and preserved both
member digests. Explicit selection of both members recovered the synced
file and SQLite row. A byte-identical descriptor restore enabled Auto
to recover both again. Evidence is under
`target/openbsd-raid1c-descriptor-recovery-26/`.

FreeBSD `0dd174c` records the GELI and md detach steps before issuing
their native commands. A retry validates the source identity and uses the
GEOM and md inventories to clear a completed step. The native test also
verified that cleanup preserves a different file attached to a reused md
unit. The FreeBSD workspace passed 113 tests with 11 ignored. Evidence
is under `target/goal-validation/freebsd-native-detach-retry/`.

The exact installed `0dd174c` package passed 26 GELI checks on stock
FreeBSD 15.1, split between explicit and Auto operation. Both routes
verified file fsync and SQLite FULL/WAL data after reopen. Package
removal restored the native and package inventories. Evidence is under
`target/freebsd-geli-detach/`.

## Browser profiles on native mounts

OpenBSD's exact `e607cc7` package passed 43 checks with Firefox ESR
153.4.0 running offline as UID 1000. CRYPTO, AES VeraCrypt and RAID1C
each stored and recovered profile data across normal vault close and
reopen. Harness revisions were `b9117eb` and `5ea6e3f`. Evidence is
under `target/openbsd-firefox-bed2448/`.

FreeBSD's exact `e607cc7` package passed 30 checks across GELI and AES
VeraCrypt. Firefox ESR 153.7.0 ran as UID 1300 with loopback access.
Both profiles recovered their data after close and reopen. The first
browser launch crashed with no available fonts. The replay passed with
signed native DejaVu font packages installed in the disposable guest.
Evidence is under `target/freebsd-browser-profile/`.

FreeBSD 15.1 also passed 30 checks across PEFS and gocryptfs using the
full native `ca3c845` package and harness `b9117eb`. Firefox ESR 153.7.0
ran as UID 1300 on a loopback-only guest. Fresh browser processes
recovered both profiles after vault close and reopen. Evidence is under
`target/freebsd-directory-browser/`.

The exact `e607cc7` package also passed 15 checks each on stock NetBSD
11.1 CGD and DragonFly 6.4.2 AES LUKS1. Firefox ran as UID 1300 with
loopback-only access and both vaults preserved their profile data across
close and reopen. NetBSD used official SHA-512-verified package inputs.
DragonFly used matching official DPorts catalogs and checksum-verified
archives. Receipts are under `target/netbsd-firefox-e607cc7/replay/` and
`target/dragonfly-firefox-e607cc7/`.

The same isolated browser environment at `b9117eb` passed 15 checks
on the installed Debian `14197ef` LUKS2 package. Firefox ESR 153.7.0
ran as UID 1300 in a loopback-only namespace and recovered the profile
in a fresh browser process. Evidence is under
`target/browser-profile-qualification/debian-luks2-b9117eb/`.

## Workspace verification after the OpenBSD and Rust dependency updates

All ten non-macOS KVM guests ran locked workspace checks and tests from
`ca3c8456e507102a1c6e2b9194981afbd48df8db`. The totals were 1,220 passed,
zero failed, and 112 ignored.

| Guest | Passed | Ignored | Remote run |
| --- | ---: | ---: | --- |
| Arch Linux | 135 | 13 | `20261119T201634Z-4b1cae2e` |
| Debian, Ubuntu, Fedora, openSUSE, Gentoo | 135 each | 13 each | `20261119T201722Z-02bef4e2` |
| FreeBSD | 112 | 10 | `20261119T202105Z-c5d427b1` |
| OpenBSD | 111 | 17 | `20261119T201835Z-1a695499` |
| NetBSD | 95 | 4 | `20261119T201722Z-d83260c0` |
| DragonFly BSD | 92 | 3 | `20261119T201722Z-311c61a7` |

The first FreeBSD attempt, `20261119T201722Z-d64aec4f`, stopped before
compilation because the cache had 3,693 MiB free against a 4,096 MiB
threshold. The retry used a 3,000 MiB threshold after a prior workspace run
with the same no-debug build profile used 1,938 MiB. Per-guest stages and
log digests are in
`target/goal-validation/secure-synchronous-ca3c845/workspace-summary.json`.

After the DragonFly changes, NetBSD passed workspace check and reported
95 passed and four ignored at `dfc7fdc`. OpenBSD passed its locked
workspace checks with 111 passed and 17 ignored. Its initial desktop test
build aborted in the default debug profile. The confirmation used the
existing no-debug, single-job OpenBSD profile and exited successfully.
FreeBSD's `ae53afb` allocation work passed with 112 tests and ten ignored.
The receipts are under `target/goal-validation/current-bsd-workspace/` and
`target/goal-validation/freebsd-geli-allocation/`.

The native installers from the same `ca3c845` revision passed 1,642 real
CLI lifecycle checks. Each target also rendered an installed X11 window.

| Target | CLI checks | Additional installed evidence |
| --- | ---: | --- |
| Arch, Debian, Ubuntu, Fedora, openSUSE, Gentoo | 195 each, 1,170 total | 117 privileged and 78 login-user checks per guest ran offline. |
| FreeBSD | 208 | Four desktop storage lifecycles and VeraCrypt daemon recovery. |
| OpenBSD | 147 | Three desktop storage lifecycles and synchronous FFS confirmed at runtime. |
| NetBSD | 26 | Explicit and Auto CGD, with 13 checks each. |
| DragonFly BSD | 91 | Desktop LUKS1 lifecycle and eight UFS exhaustion checks on an isolated archive clone. |

The base DragonFly VM built the package binaries. Its package database
required installation in a prepared archive clone. Linux removed the test
packages and verified that all owned mounts, loops and processes were gone.
Whole-guest mount inventory equality remained unverified on Linux. The
artifact digests and lifecycle receipts are in
`target/goal-validation/secure-synchronous-ca3c845/native-package-summary.json`.

## Clean-machine installation and offline runtime

Fresh KVM guests used vendor installation images and local native
prerequisites. Each ran the actual installed CLI and desktop package
without an external network route. Together they passed 1,551 CLI
lifecycle checks and ten X11 window probes.

| Guest | Product source | Installed lifecycle checks | Desktop |
| --- | --- | ---: | --- |
| Arch cloud image | `1e04b85` | 195 across all approved Linux routes | X11 passed |
| Debian 13 cloud image | `1e04b85` | 195 across all approved Linux routes | X11 passed |
| Ubuntu 26.10 cloud image | `1e04b85` | 195 across all approved Linux routes | X11 passed |
| Fedora 45 cloud image | `1e04b85` | 195 across all approved Linux routes, SELinux enforcing | X11 passed |
| openSUSE Leap 16.1 cloud image | `1e04b85` | 195 across all approved Linux routes | X11 passed |
| Gentoo cloud image | `1e04b85` | 195 across all approved Linux routes | X11 passed |
| FreeBSD 15.1 cloud image | `ca3c845` | 208 across GELI, VeraCrypt, PEFS and gocryptfs | X11 passed |
| OpenBSD 8.0 stock install | `ca3c845` | 69 across CRYPTO, AES VeraCrypt and RAID1C | X11 passed |
| NetBSD 11.1 stock image | `ca3c845` | 13 CGD checks, including read-only Auto reopen | X11 passed |
| DragonFly BSD 6.4.2 stock install | `ca3c845` | 91 LUKS1 checks plus eight UFS exhaustion checks | X11 passed |

OpenBSD's native package manager rejects the unsigned developer archive
under its stock policy. The clean replay used its scoped unsigned-package
option after verifying the exact archive digest. The platform packaging
guides record source image identities, dependency receipts and cleanup.
The aggregate is in `target/goal-validation/clean-install-matrix-summary.json`.

## Qualification after the Linux runtime and Unix packaging changes

All six native Linux packages at `c21e0cc` passed 1,170 offline lifecycle
checks and six installed X11 probes. Arch and Gentoo also passed 26
VeraCrypt checks from an inherited root-only working directory. Gentoo's
initial CryFS close error is retained with its complete passing retry.

All six packages rebuilt at `8bb689c`. The installed gocryptfs, CryFS,
and Cryptomator routes passed 234 focused offline checks, and all six
installed X11 probes passed. Each package preserved all 131 private
backend entries from `c21e0cc`. Cleanup restored the package inventories
and cleared the owned test resources. Exact artifacts and receipts are in
`target/linux-coordinated-c21e0cc/` and `target/linux-coordinated-8bb689c/`.

The six native installers at `14197ef` verified 899 installed entries
per guest, including a matching catalog of 372 locked Rust crates and
647 notice files. All 131 private backend files matched `8bb689c`. The
installed gocryptfs suites passed 78 checks covering create, unlock,
SQLite writes, close and reopen. All six installed X11 windows rendered
at 1280 × 860. Cleanup restored the package and loop inventories. On
Gentoo, the transient `/run/user/1300` mount expired during the replay.
The first manifest comparison omitted the executable flag. The corrected
comparison passed against the same package bytes. See
`target/linux-coordinated-14197ef/matrix.json` for both attempts.

NetBSD 11.1 built the optimized CLI, helper and desktop from `14197ef`.
The native package matched all 645 staged payload files. The Cargo
catalog covered 369 built dependencies and 638 notice files, checked
against the locked archives. The installed CLI passed all 13 CGD
lifecycle checks, including SQLite writes and reopen. The alternate
installation was removed and the original guest inventory matched.
Evidence is in `target/netbsd-rust-notices-14197ef/qualification-summary.json`.

## Stock non-macOS installer refresh at `14ede46`

Disposable stock KVM guests for all ten targets installed native packages
from `14ede46fb401578f0e8a4ee9b51e72ce17c0ad04`. The suites created and
unlocked encrypted storage, wrote files and SQLite data, closed the vaults,
then reopened them to verify read-back.

| Target | Installed storage checks | Desktop |
| --- | ---: | --- |
| Arch, Debian, Ubuntu, Fedora, openSUSE, Gentoo | 195 each, 1,170 total | Six X11 passes |
| FreeBSD | 208 across GELI, VeraCrypt, PEFS and gocryptfs | X11 and four helper callback lifecycles |
| OpenBSD | 160 across CRYPTO, RAID1C and VeraCrypt | X11 and three helper callback lifecycles |
| NetBSD | 26 explicit and Auto CGD | X11 and CGD helper callback |
| DragonFly BSD | 91 LUKS1, plus nine UFS full-storage checks | X11 and LUKS1 helper callback |

The six prepared Linux guests also passed 1,170 installed checks using the
same package hashes. Every Linux installer verified 899 payload entries.
The disposable guests and networks were retired. The original ten guest
definitions and backing disks matched their baselines.

Sources, artifact hashes, per-guest receipts and retry details are indexed
in `target/goal-validation/frozen-14ede46/matrix.json`, with package notes
in `docs/linux-native-packaging.md` and `docs/bsd-packaging.md`. Power-loss
and publisher release requirements are tracked in the sections below.

The same six Linux artifacts passed the installed Wayland probe on stock
guests. The original OpenBSD package omitted the Rust notice catalog.
The corrected package preserved its product binaries and repeated all
160 storage checks, X11 and three desktop helper lifecycles. The shared
native wrapper now requires build-bound Rust notices.

The exact vendored Rust source passed an offline workspace build and a
portable LUKS2 rebuild with 13 stock Arch lifecycle checks. Arch verified
a signature fixture covering that source archive and all ten native
installer candidates, including the corrected OpenBSD package. The
current receipts are indexed in
`target/goal-validation/release-provenance-14ede46/qualification-index.json`.

Two offline builds on each platform reproduced the shipped Cryptomator
runtime on Debian and the shipped gocryptfs executable on FreeBSD. The
rebuilt Cryptomator runtime passed 13 stock Debian checks. The FreeBSD engine
passed 26 stock explicit and Auto checks. Source and runtime receipts are
listed in `docs/unix-packaging.md`.

### Device creation and full storage

The six stock Linux guests passed all 54 device-create profiles using
the same native installers. Each guest tested AES, Serpent, and Twofish
with LUKS2, LUKS1, and VeraCrypt on fixture-owned file-backed loop devices.
Each profile synced a file, verified it after a read-only reopen, rejected
a write, and detached cleanly. The original guests and disks matched their
baselines. See `target/goal-validation/linux-stock-block-14ede46-matrix.json`.

The BSD full-storage batches also used the existing `14ede46` installers.
The tested filesystems returned `ENOSPC`, and SQLite returned `SQLITE_FULL`.
The suites verified committed data at capacity, then made new commits after
freeing space and checked the data after clean close and reopen.

| Stock guest | Full-storage checks | Evidence |
| --- | ---: | --- |
| FreeBSD | 36 across GELI, AES VeraCrypt, PEFS, and gocryptfs | `target/freebsd-file-full-storage-14ede46/receipt.json` and `target/freebsd-directory-full-storage-14ede46/receipt.json` |
| OpenBSD | 27 across CRYPTO, RAID1C, and AES VeraCrypt | `target/openbsd-enospc-14ede46/summary.json` |
| NetBSD | 9 for CGD | `target/netbsd-cgd-full-storage-14ede46/qualification-summary.json` |
| DragonFly BSD | 9 for AES LUKS1, also reported above | `target/goal-validation/frozen-14ede46/dragonfly/` |

The exact DragonFly package also passed orderly shutdown on a fresh stock
guest. Native `rc.shutdown` stopped the SQLite writer and closed the vault.
After reboot, the same vault recovered the synced file and committed SQLite
row. See `target/dragonfly-shutdown-14ede46/receipt.json`.

Cryptomator on Debian could acknowledge a short ciphertext append at `fsync`
and then fail to read the previously committed prefix. An isolated CryptoFS
2.12.0 comparison surfaced the write failure at `fsync` but still lost the
readable prefix. The candidate was held back. See `docs/folder-supervision.md`.

## Coordinated non-macOS native qualification, 2026-11-20

All ten KVM targets built and exercised native packages from
`a60e4bc69a3b78b8dd6e2297c2300a38f4508251`. Each installed package
passed real encrypted create, unlock, write, close, reopen and read-back.

| Target | Native installed CLI checks | Desktop and additional evidence |
| --- | ---: | --- |
| Arch, Debian, Ubuntu, Fedora, openSUSE, Gentoo | 195 each, 1,170 total | Actual X11 window on each, with Gentoo Portage dependencies resolved. Four Debian Firefox profiles passed. |
| FreeBSD | 208 | All four desktop backend lifecycles, X11, VeraCrypt generation recovery. |
| OpenBSD | 147 | All three desktop backend lifecycles, X11, raw-image privacy and WAL reopen. |
| NetBSD | 26 | CGD desktop, X11, legacy-image compatibility, eight full-storage checks on the byte-matched portable package. |
| DragonFly BSD | 91 | Desktop, X11, eight full-storage checks and verified LUKS1 headers. |

All six Linux guests ran the 117 privileged and 78 login-user cases in
isolated network namespaces across the normal and follow-up replays. Arch
also installed the exact package in a loopback-only namespace. The
replays used prepared guests and their existing native runtime prerequisites.

The verified Debian `14197ef` package separately passed 45 installed
checks across AES LUKS2, LUKS1, and privileged VeraCrypt. Firefox ESR
ran as the login user in a loopback-only namespace and verified its
profile after each vault was closed and reopened.

Exact artifacts and run receipts are recorded in `docs/linux-native-packaging.md`
and `docs/bsd-packaging.md`. Power-cut evidence has its own revision and fixture
identities in `docs/vm-powerloss-testing.md`.

## Earlier non-macOS KVM package round

The baseline product source for this round was
`2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1`. Platform fixes, native installers
and harness changes retain separate revision identities.

| Target | Implemented formats | Qualification at this checkpoint |
| --- | --- | --- |
| Arch, Debian, Ubuntu, Fedora, openSUSE, Gentoo | LUKS2, LUKS1, VeraCrypt (kernel and userspace), gocryptfs, CryFS, Cryptomator | Each baseline package passed 117 privileged and 78 login-user lifecycle checks. Six native installers passed 650 additional checks. Debian passed six desktop and four Firefox profile lifecycles. Arch power-cut checks passed across the seven routes after the LKL and CryFS fixes, with exact revisions recorded separately. |
| FreeBSD | GELI, VeraCrypt, PEFS, gocryptfs | Rebuilt `029cf99` passed 208 CLI checks and four desktop lifecycles after fixing gocryptfs's SQLite checkpoint failure. GELI reopened after a cut but failed file and database recovery. |
| OpenBSD | softraid CRYPTO, RAID1C, VeraCrypt | Rebuilt `264cd6e` passed 147 CLI checks, all three desktop lifecycles and an X11 window. The VeraCrypt fix passed login-user WAL persistence and denied direct access to the private image. FFS refused writable RAID1C reopen after a cut at `2ebae8f`. |
| NetBSD | CGD | `24dcc6e` passed 26 CLI checks, desktop and older-image compatibility with WAPBL. Two journaled power cuts recovered the fsynced file and lost the committed SQLite transaction. |
| DragonFly BSD | LUKS1 | `2ebae8f` passed 91 CLI checks, desktop lifecycle/window and eight UFS exhaustion checks. The fault clone stopped at an outer UFS check before encrypted recovery. |
| macOS | Apple encrypted disk images, VeraCrypt, gocryptfs, CryFS, Cryptomator | Historical `294b2b8` DMG passed 182 CLI checks and five Cryptomator crash/recovery checks. The retained initial VeraCrypt timeout and remaining desktop checks are described below. This round excludes macOS implementation and test runs. |

See `docs/backend-testing.md`, `docs/linux-package-testing.md`,
`docs/linux-native-packaging.md`, `docs/bsd-packaging.md` and
`docs/vm-powerloss-testing.md` for exact evidence.

## Earlier macOS and BSD checkpoint

The paused WIP was committed/pushed, then implementation resumed. Darwin's
absent-ACL handling and discarded capability output are fixed. Native Mac fence
tests now pass 21/21, including inherited-descriptor guardian SIGKILL; the fenced
Java image passes its integration tests and helper negotiation passes. Explicit/
Auto CLI lifecycles pass 26/26. Supervised JVM crash, plaintext write denial,
replacement refusal, explicit recovery, committed file/SQLite WAL integrity and
normal close pass five checks. These results retain their individual revisions
in `docs/folder-supervision.md`. Rebuilt Mac package `294b2b8` now passes 182 CLI
checks and five packaged crash/recovery checks. One VeraCrypt Serpent/keyfile
reopen timed out without leaving a mount; the retained failed attempt passed on
retry. This remains an observed transient limitation, not an erased failure.
RAID1C power-loss setup/reopening now supplies both recorded members; runner unit
tests pass 31/31. Actual RAID1C power-loss and final cross-host packages remain pending.

The earlier full-scope Mac `8aac707` package passed 156 normal CLI checks, 11
rapid-close checks and an Apple desktop lifecycle, but crash testing exposed
writable plaintext underlay after automatic detach. It is not release-qualified.
OpenBSD desktop code generation failed with OOM/ENOSPC. On resumption OpenBSD,
NetBSD and DragonFly were stopped on root-filesystem inconsistencies. Explicitly
authorized repairs without backups completed; read-only follow-up checks passed
and all three booted with SSH available. OpenBSD's missing X11 files were restored
from signed official sets, preserving 52 existing configurations and boot controls.
Its workspace rebuild now raises only the build process's 1.5 GiB data limit to
4 GiB; root has roughly 4 GiB headroom. Final Linux/BSD package source is frozen
at `cc87b54ef6`; Mac artifact source remains independently recorded as `294b2b8`.

## Earlier implementation checkpoint

- macOS policy: VeraCrypt uses FUSE-T only; gocryptfs/CryFS use macFUSE;
  Cryptomator uses FUSE-T's signed FSKit extension (macOS 26+). The user enabled
  that extension on Buran. SMB/NFS failed SQLite requirements and are not fallbacks.
- The packaged `d330b20` Mac checkpoint passed VeraCrypt 78/78, gocryptfs/CryFS
  52/52 and an Apple-image desktop callback lifecycle. It omits Cryptomator and
  is not the final full-scope package. The production FSKit helper is committed;
  `8aac707` fixes the observed mounted-ACK/readiness race. Full-package tests follow.
- OpenBSD RAID1C now has command/helper/desktop support, native two-member and
  explicit degraded recovery, descriptor-last publication and identity guards.
  Development CLI checks passed 19/19 plus 11/11 owner/returning-peer checks.
  Native duplicate-role stale peers are refused unchanged, not auto-rebuilt.
- New Linux/BSD filesystems and images receive the invoking user's private
  ownership. Real normal-user GUI/helper lifecycles passed for NetBSD CGD and
  Arch LUKS2/gocryptfs. Native FreeBSD/DragonFly desktop builds passed after the
  upstream rustix 1.1.6 fix; the fontless zoom fixture is fixed and passes.
  Actual native X11 windows pass on FreeBSD, NetBSD and DragonFly; OpenBSD remains pending.
- Linux loop leases, persistent directory-backend exit results and Cryptomator
  close acknowledgements are implemented. Mounted CryFS ENOSPC/EDQUOT write
  recovery passed. A close while still full may abort upstream; Lurker reports
  failure and retains exact exit evidence instead of falsely claiming success.

Evidence: `docs/macos-backends.md`, `docs/folder-supervision.md`,
`docs/backend-testing.md`, `docs/remote-testing.md`. Current native tests retain
their exact source/binary identities; they are not relabeled as a final matrix.

## Architecture delivered

- Shared command model/platform allowlist, directory lifecycle and BSD container lifecycle with small native adapters; common VeraCrypt profile and Unicode patch.
- Private VeraCrypt engines, pinned Go/CryFS builds and private Cryptomator/JRE; administrator-controlled elevated executables, identity-checked sessions, atomic publication and busy-safe teardown.
- Desktop backend choices/native defaults, directory forms, supported credentials and Linux-only device/privilege/keyslot controls over the shared helper/core.
- Exact-commit remote jobs, separate compiler/native caches, bounded JSON reports and verified Unix/macOS staging.

## Earlier frozen build/unit matrix

All eleven hosts used the same final candidate. Ignored native tests do not count
as runtime qualification; the packaged CLI suites below are separate.

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| macOS workspace, including desktop | 62 | 0 | 8 |
| Debian workspace, including desktop | 87 | 0 | 12 |
| Arch / Ubuntu / Fedora / openSUSE / Gentoo core, each | 60 | 0 | 12 |
| FreeBSD core | 46 | 0 | 5 |
| OpenBSD core | 43 | 0 | 9 |
| NetBSD core | 36 | 0 | 2 |
| DragonFly core | 34 | 0 | 1 |

Total: **608 passed, 0 failed, 97 ignored**. Exact jobs/counts:
`docs/remote-testing.md`, `target/remote-test/final-matrix-result.json` (ignored).
Desktop form tests cover six OS models and multiple zoom levels; native BSD GUI
runtime qualification remains separate. Linux desktop release builds also pass.

## Packaged native CLI evidence

`76f05fb` is the immutable baseline; `a16cd18` includes the Linux publication
guard, desktop forms and FreeBSD process-exit fix. Baseline results retain their
original provenance; they are not relabeled as a full final-artifact rerun.

| Target / revision | Passed checks |
| --- | --- |
| Arch / baseline | 195 lifecycle; 8 cross-mode; 9 disposable loop device profiles; 30 directory daemon-crash checks |
| Arch / final | 8 cross-mode; 24 forced-replacement checks across privileged LUKS2/LUKS1/VeraCrypt and userspace VeraCrypt |
| FreeBSD / baseline | 104: VeraCrypt all three ciphers plus Auto AES; GELI/PEFS explicit and Auto |
| FreeBSD / final | 26: gocryptfs explicit and Auto; fixes the retained-session failure seen in the baseline |
| OpenBSD / baseline | 78: VeraCrypt all three ciphers plus Auto AES; softraid CRYPTO explicit and Auto |
| NetBSD / baseline | 26: CGD explicit and Auto |
| DragonFly / baseline | 52: LUKS1 all three ciphers plus Auto AES |
| macOS / final | 26: Apple images explicit and Auto |

All listed lifecycle cases pass without skips. Unsupported OpenBSD read-only
requests exercise safe rejection. BSD native adapter/profile sources are
unchanged between revisions; the FreeBSD delta affects portable directory
process checks. PTY harness `8bf9f0e` is recorded separately from runtime artifacts.

Evidence: `docs/backend-testing.md`, `docs/linux-userspace.md`,
`target/linux-package-{76f05fbd21,a16cd18b84}/qualification/`, and
`dist/macos/a16cd18/apple-native/`. Failure evidence is retained. Owned runtime
mounts/processes are reconciled; temporary FreeBSD FUSE access is restored.

## Abrupt shutdown characterization

At baseline `76f05fb`, isolated copies of Arch LUKS2, FreeBSD GELI and NetBSD CGD
pass five recovery checks each after actual KVM poweroff/restart: durable file
and committed SQLite WAL contents survive, integrity passes, uncommitted
transactions stay invisible, and normal CLI reopen/close succeeds without repair.

OpenBSD softraid's inner FFS **refuses writable reopening after the power cut**
and requests a filesystem check. No repair, forced mount or writable fallback
was attempted. This records a recovery limitation; it does not establish loss
of the acknowledged data. Its normal lifecycle passes separately.

Original guests remain running; fault tests verify their domain definitions and
backing disk paths remain unchanged. Ordinary fixtures use private
`/tmp` directories; shutdown fixtures use persistent guest caches inside isolated
owned clones. Evidence: `docs/vm-powerloss-testing.md`,
`target/vm-powerloss/summary.json`, `/mnt/virt/lurker-fault-*`.

## Remaining qualification work and limits

- On Arch, the exact `14ede46` CryFS package returned `ENOSPC` and
  `EDQUOT` to the application. Freeing space allowed clean close and
  reopen. Closing while storage remained exhausted reported an unclean
  exit for pending directory metadata. Reopen recovered the committed file
  and SQLite row. See `docs/folder-supervision.md`.
- On OpenBSD, the installed `e607cc7` package passed 24 full-storage
  checks across CRYPTO, RAID1C, and VeraCrypt. All three preserved
  committed data at `ENOSPC` and accepted new writes after freeing
  space. On Debian `14197ef` LUKS1, a 16 MiB transaction exited with
  `SIGBUS` in SQLite 3.46.1 after Btrfs returned `ENOSPC`. The isolated
  writer check records the signal separately. File and database recovery
  after freeing space and reopening passed. See `docs/backend-testing.md`.
- FreeBSD's installed `e607cc7` AES VeraCrypt package passed nine
  full-storage checks. UFS returned `ENOSPC`, SQLite returned
  `SQLITE_FULL`, and file and SQLite data written before and after
  freeing space survived reopen. Evidence is under
  `target/freebsd-veracrypt-full-storage/`.
- The same FreeBSD release passed 16 GELI metadata backup and restoration
  checks. An owned copy refused to open with its metadata missing.
  Native restore returned the copy to its original digest. File and
  SQLite data survived reopen, including new writes made after restore.
  Evidence is under `target/freebsd-geli-metadata/`.
- FreeBSD's sparse GELI trial lost acknowledged writes. The fully
  allocated control and the `ae53afb` production UFS create recovered
  them. NetBSD CGD still lost acknowledged writes after abrupt cuts
  under ordinary application sync. FreeBSD VeraCrypt at `2fda5f7`
  recovered the acknowledged file and committed SQLite row and closed
  normally after a repeat cut. FreeBSD PEFS and gocryptfs also passed the
  recovery workload, with explicit dead-session reconciliation needed
  for gocryptfs.
- OpenBSD RAID1C at `2ebae8f` and CRYPTO and VeraCrypt at `4189679`
  recovered acknowledged files and SQLite transactions from read-only FFS
  mounts of disposable ciphertext copies after cuts. Normal writable CLI
  reopen still failed during mount on all three routes. Fresh synchronous
  FFS controls and the default `ca3c845` package on all three routes passed
  explicit preen and writable CLI recovery on copies. Conventional FFS
  controls reported duplicate blocks requiring manual intervention.
- DragonFly's freshly published LUKS1 fixture was missing from a recovered
  copy of the HAMMER2 backing disk. Previously published vaults unlocked
  after repeat cuts on HAMMER2 and synchronous UFS backing disks. Both
  retained the pre-workload ciphertext digest and lost the acknowledged
  file and SQLite data. The backing UFS required preen on a separate copy.
- OpenBSD may return ENXIO for a configured vnode after its VeraCrypt FUSE
  daemon dies, making source identity unverifiable. Production preserves that
  session. The final native generation and alias checks passed for OpenBSD;
  FreeBSD also passed its daemon-death recovery case.
- The opt-in `session-close` hook passed 82 checks across Debian, FreeBSD and
  NetBSD. Arch also stopped its systemd consumer, ran the hook, shut down
  cleanly, rebooted and recovered the file and SQLite data. Separate native
  `rc.shutdown` tests recovered both after reboot on FreeBSD GELI, NetBSD
  CGD, DragonFly LUKS1, and all three OpenBSD storage routes. Publisher
  signatures and release security qualification still require evidence.
- The DragonFly 6.4.2 base `cryptsetup` and reviewed maintenance branch
  lack the upstream LUKS1 reader's structural keyslot checks. The guarded
  adapter above is the candidate protection for these builds. Release
  qualification requires independent review. See
  `docs/encryption-methods-matrix.md`.
- The `ca3c845` private LKL Btrfs tree matches Linux `v6.12`. The builder
  now pins `v6.12.118` and rechecks its applied sources. A rebuilt helper
  passed 39 Arch login-user checks. The full `1e04b85` Arch archive passed
  all five VeraCrypt recovery checks after a separate power cut. Native
  packages from `1e04b85` passed 1,170 offline lifecycle checks and six X11
  window probes on the prepared Linux guests with the same updated engine.
- macOS remains outside the current round. Its historical `294b2b8` DMG passed
  182 CLI and five crash/recovery checks. Fast-close and final desktop checks
  and the retained VeraCrypt reopen timeout remain part of Mac qualification.

Earlier Linux archive: `target/linux-package-a16cd18b84/package/package.tar.gz`.
Earlier Mac developer DMG: `dist/macos/a16cd18/Lurker-0.1.0-macos-arm64.dmg`.
Both have retained manifests, source identities and SHA-256 proofs; the DMG is
ad-hoc signed rather than notarized.
