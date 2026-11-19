# Cross-platform implementation status

Active non-macOS qualification, 2026-11-20. The coordinated native package
results and remaining release gates apply to this round. Earlier checkpoints
retain their original dates and source revisions. The approved profiles are
in `DIRECTIVES/LLM/CIPHER.md`.

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
  reopen still failed during mount on all three routes.
- DragonFly's published LUKS1 fixture was missing from a recovered copy of
  the separate HAMMER2 backing disk. Native BSD adapters remain file-backed
  and perform no automatic filesystem repair.
- OpenBSD may return ENXIO for a configured vnode after its VeraCrypt FUSE
  daemon dies, making source identity unverifiable. Production preserves that
  session. The final native generation and alias checks passed for OpenBSD;
  FreeBSD also passed its daemon-death recovery case.
- The opt-in `session-close` hook passed 82 checks across Debian, FreeBSD and
  NetBSD. Arch also stopped its systemd consumer, ran the hook, shut down
  cleanly, rebooted and recovered the file and SQLite data. Broader shutdown
  integration, clean-machine qualification, offline BSD coverage, publisher
  signatures and security review still require evidence.
- macOS remains outside the current round. Its historical `294b2b8` DMG passed
  182 CLI and five crash/recovery checks. Fast-close and final desktop checks
  and the retained VeraCrypt reopen timeout remain part of Mac qualification.

Earlier Linux archive: `target/linux-package-a16cd18b84/package/package.tar.gz`.
Earlier Mac developer DMG: `dist/macos/a16cd18/Lurker-0.1.0-macos-arm64.dmg`.
Both have retained manifests, source identities and SHA-256 proofs; the DMG is
ad-hoc signed rather than notarized.
