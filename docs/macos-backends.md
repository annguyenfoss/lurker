# macOS private encryption backends

`scripts/build-macos-backends.py` builds a pinned native VeraCrypt backend and
static wxWidgets, without installing either application. Verified source and wx
caches default to `~/.cache/lurker/macos-backends`; staged artifacts stay under
`target/release/macos-backends`. Source
archives are SHA-256 checked. The build records target, compiler, SDK, FUSE
implementation/version, source URLs/hashes and staged artifact hashes in `provenance.json`.
The Mac engine is pinned to the signed upstream VeraCrypt 1.26.31 release
commit `7afc449f2bc69366e8b67b82ad66201325d318c3`. It includes Unix descriptor
inheritance hardening and system-tool wait/timeout fixes relevant to the previous
native hangs; native qualification below remains a separate requirement.
https://github.com/veracrypt/VeraCrypt/releases/tag/VeraCrypt_1.26.31

Its text interface uses the existing VeraCrypt profiles and stdin credentials.
The shared `native/veracrypt/utf8-console.patch` preserves complete Unicode paths
in libc++ volume listings; its digest and patched input hashes are recorded and
checked on cache reuse. It changes no encryption or storage format.

The macOS-only `native/veracrypt/macos-fuset-loopback.patch` selects FUSE-T's
`nobonjour` SMB mount option. Each auxiliary mount connects to its own loopback
port. The previous shared Bonjour discovery name could time out on repeated
create and unlock operations. FUSE-T 1.2.9 introduced this per-mount option.
https://github.com/macos-fuse-t/fuse-t/releases/tag/1.2.9

Build on a Mac using the user's Rust and Python toolchains, Xcode command-line
tools, GNU-compatible make and pkg-config. VeraCrypt uses separately installed
FUSE-T 1.2.9 or later and its development headers for SMB loopback. The full
bundle uses FUSE-T 1.2.9 because gocryptfs uses its versioned, signed helper.
gocryptfs, CryFS and Cryptomator mount through FUSE-T FSKit. Enable FUSE-T in
System Settings > General > Login Items & Extensions > File System Extensions.
FSKit requires macOS 26 or later. Native deployment targets may raise the
minimum macOS version recorded in the package.
The build never installs FUSE or changes system permissions:

```bash
python scripts/build-macos-backends.py --jobs 4
python scripts/build-macos-dmg.py
```

`--fuse-runtime fuse-t` is accepted explicitly. The build records both
FUSE-selection flags, runtime implementation/version and FUSE API version.
Verified source, patched inputs and build specifications keep old native
objects from being recertified under different inputs. Mach-O dependency
audits accept only the reviewed FUSE-T install names for external FUSE.

After building directory backends into `target/release/backends`, include them
explicitly with `--folder-backends gocryptfs cryfs cryptomator`. The packager
checks their manifests, includes the private Cryptomator Java image, audits its
native dependencies and signs its nested code. The Java image lives in sealed
Resources with a native launcher in Helpers; its source/notices are resources,
not nested executable code. Java executables receive the JIT
and native-library validation exceptions needed by that private runtime. This
does not qualify an incompatible host FUSE runtime; missing prerequisites must
still fail closed.

`--offline` requires previously downloaded and verified archives. Native sources
and objects are cached; changed compiler/SDK/options require a fresh cache
directory. A failed wxWidgets build is not treated as a completed dependency.

The DMG includes the CLI at `Lurker.app/Contents/MacOS/lurker`, the desktop,
internal helper, private backend, private non-system dylibs, notices and build
provenance. Backends are resolved relative to the running app/CLI; separately
installed VeraCrypt applications and PATH executables are not fallbacks. Missing private build
artifacts stop packaging. Architecture slices, deployment targets and rewritten
dependency paths are checked before nested code is signed. The selected FUSE library is
an explicit external dependency; its path, resolved target and ancestors must be
administrator-controlled. Unrelated Homebrew/developer paths are not
allowed to survive packaging.

The private VeraCrypt and CryFS executables receive the hardened-runtime library
validation exception required to load their separately installed FUSE runtime. The
desktop, CLI and internal Rust helper retain library validation. This follows
Apple's entitlement contract:
https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.disable-library-validation

Apple encrypted disk images use built-in `hdiutil`/`diskutil`. Ordinary
create, mount, close, and active-volume display work when VeraCrypt's FUSE-T
library is unavailable. Forced replacement requires discovery of all attached
containers. Ad-hoc packages are development builds. Public releases require
Developer ID signing and notarization. Each release must qualify the exact
FUSE/OS combination and test the final package without a separate VeraCrypt
installation. Bundling alone does not prove native operation or durability.

Use only disposable `/tmp/lurker-*` files for native tests:

```bash
TMPDIR=/tmp cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1
```

## FUSE-T package qualification

The development DMG from `4dcf8d68434b7856a1cd3bd13977098482628e23`
passed 356 CLI checks across 20 cases on Buran (macOS 27.2.1 arm64).
Apple disk images, VeraCrypt and all three directory backends passed.
The test confirmed 14 FUSE-T FSKit mount observations per directory
backend and covered SQLite WAL, read-only access, rapid close and
reopen, and exact file digests. Strict code signature checks passed,
and the dependency audit covered 23 Mach-O files. The tested DMG records
macOS 27.2 as its minimum and has SHA-256
`e4c5aeae7a6e7ec0bcb02d4859c480ceadf3666e376cc74c5a6f0ab8c48bfb9e`.
Workspace check and 104 workspace tests passed in run
`20261123T080933Z-cde84fab`.

## Earlier packaged lifecycle on Buran

On macOS 27.2.1 arm64 with SDK 27.2, the developer DMG from
`b2da478d6e86fd8ea733ecff331099995bfa7a6d` passed 320 CLI checks across
20 cases. The DMG SHA-256 was
`afe4f38c72e666832d5d19ac869d01165198d7d0c65bac6580577ca94779b19c`.
The standalone app passed the strict nested code signature and dependency
checks and required macOS 27.2. The release build reused the verified native
VeraCrypt and directory backend inputs.

VeraCrypt passed 192 checks across AES, Serpent and Twofish with both
password-only and binary-keyfile-only credentials and explicit and Auto
mounts. It used FUSE-T 1.2.9. Apple images, gocryptfs, CryFS and Cryptomator
each passed 32 checks. The directory backends used macFUSE for gocryptfs and
CryFS and FUSE-T FSKit for Cryptomator.

Every case wrote and synced three PDFs and a page of text, checked the four
files after close and reopen, then appended to the text and synced a fifth
file. All five SHA-256 digests matched after another close and reopen. Each
case completed the CLI checks, including SQLite WAL and busy close, and both
of its fixture mountpoints were clear afterward.
An owned Cryptomator JVM exit also passed all five fence and recovery checks.
All five Firefox profiles passed 75 checks. All five desktop callback tests
passed through the packaged helper. The VeraCrypt callback used a private
bundle layout so the test executable could find the packaged engine when
listing mounted volumes.
Receipts are under `target/goal-validation/ssh-matrix-20261122/macos-evidence/b2da478/`.

## Packaged FSKit close regression

The earlier rapid-close suite encountered one Cryptomator busy refusal and
then closed the same vault normally. A control using the native `/sbin/umount`
returned `Resource busy` on four of 30 immediate calls. Each call preserved
the mounted vault and the next ordinary close succeeded.

The `c997d6a06973fcc61fc292c51c540d360ed75534` developer DMG passed 32
Cryptomator CLI checks across the explicit and Auto payload lifecycles,
35 rapid close cycles, five JVM crash and fence checks, 15 Firefox profile
checks, and the desktop callback through the installed helper. The controlled
transient case closed after its consumer exited and reopened with the synced
file and SQLite data intact. A consumer that kept the volume open still
received a busy refusal and preserved its mount. All test sessions closed.

The DMG SHA-256 was
`8e61e679620e8e6d91ef8efe329f92e3b3d15f9c4950076e6e404559067db2f9`.
All four signed native backend executables matched the earlier DMG byte for
byte. Workspace verification passed 103 tests with nine ignored. The 17
packaging checks and the opt-in plaintext Apple image test also passed.
The tests did not request a reboot. This package was validated after the
operator's separately requested restart, with an unchanged boot identity
throughout the storage tests. Receipts are under
`target/goal-validation/ssh-matrix-20261122/macos-evidence/c997d6a/`.

The same `c997d6a` DMG then passed all 320 CLI checks in 20 profiles on
Buran. Each profile recovered the three PDFs, the appended text and the
second file after a close and reopen. The exact package has five desktop
callbacks and 75 Firefox checks, including the Cryptomator checks already
run on that DMG. Buran retained its boot identity and the tests left no
active mounts, journals, attached images, volumes or helper processes.
The replay receipts are in the `full-replay/` subdirectory.

## CryFS and gocryptfs FUSE-T FSKit experiment

Buran ran the experiment on macOS 27.2.1 arm64 with FUSE-T 1.2.9 and SQLite
3.53.1. CryFS 1.0.3 passed WAL, a FULL commit, second-connection reads,
checkpointing, and file and database persistence on FSKit. The same CryFS
binary mounted through NFS with default options and SQLite returned
`delete` for `PRAGMA journal_mode=WAL`. gocryptfs v2.7.0 used an opt-in
Go-FUSE transport. Its first FSKit build reproduced a rename I/O error
because the helper sent the compatibility request after extended rename
negotiation. The corrected build passed both rename layouts and the direct
file and SQLite workload.
[CryFS experiment](../native/macos/README.md) ·
[gocryptfs experiment](../native/macos/GOCRYPTFS_FUSE_T.md)

The first CLI replay also exposed transient busy errors during CryFS
initialization and immediate close. The verified FUSE-T FSKit retry now
performs one ordinary unmount retry after rechecking the recorded source
and mount identities. The patched CLI built from `9f80e1483937a3740e630147501fa03cdf961f73`
with SHA-256 `67b09c7a418c2e213befa278fdd88f9ced6c8a08656a01add722c37041e00d78`.
On Buran, workspace check passed and workspace tests reported 104 passed
and nine ignored in run `20261123T074557Z-38db421e`.

| Backend | Explicit CLI | Auto CLI | Experimental engine SHA-256 |
| --- | ---: | ---: | --- |
| CryFS | 33 passed | 33 passed | `06328620bcc9289cd4bf5285489c95bdcf69fe5ab249cc66cc3a97919c331890` |
| gocryptfs | 33 passed | 33 passed | `8fb29f2cc8460308beacb610474a060c666d123916a420dd1fd2e7e7085512e5` |

All four CLI cases verified the actual FSKit mount rows, WAL and integrity,
all five payload hashes over repeated reopens, busy-close refusal, six rapid
close cycles, and read-only access. The packaged macFUSE controls at
`c997d6a` each passed 21 checks across the same two backends and detection
modes. The harness passed its 27 Python tests on Buran.

A separate coordinated test held both FSKit volumes open together, verified
distinct session identities and data, closed CryFS while gocryptfs stayed
mounted, and verified both vaults again after reopening. The final audit
found no owned mounts, helpers, active journals, or FSKit session records
and verified the same boot identity. Receipts are under
`target/goal-validation/macos-folder-fuse-t/`, including `final-audit.json`
and `coexistence.json`.

This verification covers direct experimental engines and the native CLI.
The packaged CLI and desktop control results above predate the FUSE-T migration.
[Evaluation plan](../DIRECTIVES/PLAN/MACOS_FOLDER_FUSE_T.md)

## Earlier macOS 27.2.1 arm64 results

The `8aac707` artifact is a functional baseline. Later fault testing exposed
plaintext fallthrough into a writable mount directory after unexpected detach;
underlay fencing and fresh native qualification are required before release.

The fenced `294b2b83bbb797886a22f871ad8759a08f787f2e` DMG now passes **182 CLI
checks** (the baseline coverage below plus 26 Apple-image checks) and five
packaged Cryptomator crash/recovery checks. Its SHA-256 is
`7647f3b977ee68e6c63dceaac6c237ff0067a1597c32310add4174d9061098f8`.
One VeraCrypt Serpent/keyfile reopen timed out with no remaining mount; that
suite passed on retry. Both attempts remain in
`dist/macos/294b2b8/native/`; this is an observed transient limitation. No owned
package mounts/processes remained after qualification. Packaged fast-close and
desktop callback checks remain pending. See `docs/folder-supervision.md` for the
fence recovery contract; losing its guardian never enables a path-based chmod.

- Full-scope candidate `8aac70784f3a4742cffacced4bbe3bd48841ead0` packages Apple,
  VeraCrypt, gocryptfs, CryFS and Cryptomator. The DMG and extracted standalone app
  pass strict nested-signature/dependency audits. This artifact requires macOS
  27.2 because of native dependency targets; FSKit's feature floor of 26 does
  not establish compatibility with macOS 26.
- The exact packaged CLI passes 156 checks: VeraCrypt 78 (AES/Serpent/Twofish,
  password-only and binary-keyfile-only, explicit/Auto), gocryptfs/CryFS 52
  (each explicit/Auto), and Cryptomator FSKit 26 (explicit/Auto). Checks include
  SQLite WAL, fsync, persistence, busy/alias rejection and read-only enforcement.
- Cryptomator also passes ten immediate mount/close cycles alternating read-only
  and writable modes (11 checks including create). A headless Apple desktop
  callback lifecycle passes with the same packaged helper. Owned mounts and
  backend processes close normally; VeraCrypt leaves no SMB transport service.
- VeraCrypt 1.26.31 uses FUSE-T 1.2.9 exclusively; gocryptfs/CryFS use macFUSE
  5.5.0; Cryptomator uses FUSE-T FSKit with a private Java runtime. Tests use only
  system PATH directories and no separately installed VeraCrypt application.
- Artifact, source and binary hashes and case reports are retained under
  `dist/macos/8aac707/`. The earlier `d330b20` checkpoint remains separate.
  These results supersede the old VeraCrypt creation timeouts and Cryptomator
  SMB locking failure for the qualified combinations, without enabling fallback.

The package is ad-hoc signed, not notarized. These are native development results,
not a security audit, arbitrary OS-version qualification or full shutdown/fault
coverage. Apple images also retain their earlier packaged CLI/WAL proof.
See `DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md` for the complete matrix and limits.

## Optional inner-filesystem development tools

VeraCrypt file containers now have native development CLI evidence for explicit
`--filesystem ext4` and `--filesystem xfs`, using separately staged, unmodified
filesystem drivers with the existing FUSE-T encryption engine. Current release
packages do not bundle these optional tools. See `docs/macos-filesystems.md`
for pins, build/stage instructions, dependencies and limits, and
`DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RETEST.md` for exact CLI payload receipts.
