# Cross-platform implementation status

Active non-macOS qualification, 2026-11-20. The coordinated native package
results and remaining release gates apply to this round. Earlier checkpoints
retain their original dates and source revisions. The approved profiles are
in `DIRECTIVES/LLM/CIPHER.md`.

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

- FreeBSD GELI and NetBSD CGD lost acknowledged writes after abrupt power
  cuts. FreeBSD VeraCrypt at `2fda5f7` recovered the acknowledged file and
  committed SQLite row and closed normally after a repeat cut. FreeBSD PEFS
  and gocryptfs also passed the recovery workload, with explicit dead-session
  reconciliation needed for gocryptfs.
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
