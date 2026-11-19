# Cross-platform implementation status

Active completion pass, 2026-11-14. The earlier frozen candidate was
`a16cd18b8462293c670e66803fc4e37fc4a9c59f`; its results below remain historical.
The current integration is not yet frozen or fully qualified. Scope/profiles:
`DIRECTIVES/LLM/CIPHER.md`. Public-release qualification remains separate.

## Non-macOS KVM qualification resumed, 2026-11-20

The frozen product source for this round is
`2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1`. Harness and documentation changes
have separate revision identities.

| Target | Implemented formats | Qualification at this checkpoint |
| --- | --- | --- |
| Arch, Debian, Ubuntu, Fedora, openSUSE | LUKS2, LUKS1, VeraCrypt (kernel and userspace), gocryptfs, CryFS, Cryptomator | Each package passed 117 privileged and 78 login-user lifecycle checks. Workspace, desktop and extended fault checks are separate. |
| Gentoo | Same Linux formats and routes | Package staged and native lifecycle checks started. |
| FreeBSD | GELI, VeraCrypt, PEFS, gocryptfs | Earlier package results are below. Current package build resumed after a damaged compiler cache was identified. |
| OpenBSD | softraid CRYPTO, RAID1C, VeraCrypt | Earlier CRYPTO and VeraCrypt results are below. Current RAID1C qualification and workspace rebuild are in progress. The isolated OpenBSD fault clone has booted and is reachable. |
| NetBSD | CGD | Earlier package results are below. Current workspace and package rebuild are in progress. |
| DragonFly BSD | LUKS1 | Earlier package results are below. Current package and desktop qualification are in progress. The vendor tool and patch provenance are recorded in `docs/encryption-methods-matrix.md`. |
| macOS | Apple encrypted disk images, VeraCrypt, gocryptfs, CryFS, Cryptomator | Historical `294b2b8` DMG passed 182 CLI checks and five Cryptomator crash/recovery checks. The retained initial VeraCrypt timeout and remaining desktop checks are described below. This round excludes macOS implementation and test runs. |

Linux check details are in `docs/linux-package-testing.md`. The OpenBSD fault
copy receipt, guarded network identity and runner's 31 passing unit tests are
retained under `/mnt/virt/lurker-fault-ob20261120a/`.

## Resumed checkpoint

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

## Current completion pass

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

- Finish Mac packaged fast-close/desktop callback checks and characterize the retained VeraCrypt reopen timeout. The rebuilt five-backend `294b2b8` DMG passes 182 CLI and five crash/recovery checks; macOS FUSE remains separately installed.
- Finish final packaged native tests at one frozen source across all six Linux distributions and four BSDs. Minimal pinned wxBase/private ELF payloads now pass dependency audits on the six Linux distributions; installed developer tools do not establish clean-machine independence.
- Repeat BSD VeraCrypt generation/recovery proofs against final packages. Current native generation guards pass, including FreeBSD daemon-death recovery. OpenBSD can return ENXIO for a configured vnode after its FUSE daemon dies, making source identity unverifiable. Production preserves that session rather than guessing or force-detaching it.
- The DragonFly IDE/UEFI clone adapter is implemented and its isolated clone is ready; power-loss testing awaits the current package. OpenBSD FFS still requires checking after the characterized power cut; no automatic repair is performed.
- Native BSD support remains file-container oriented. Full-home/session automation, clean-machine/public-release qualification, signing/notarization and broader fault/security review remain separate gates.

Earlier Linux archive: `target/linux-package-a16cd18b84/package/package.tar.gz`.
Earlier Mac developer DMG: `dist/macos/a16cd18/Lurker-0.1.0-macos-arm64.dmg`.
Both have retained manifests, source identities and SHA-256 proofs; the DMG is
ad-hoc signed rather than notarized.
