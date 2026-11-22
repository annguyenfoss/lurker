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
FUSE-T 1.2.9 or later and its development headers exclusively. macFUSE is reserved for
gocryptfs/CryFS. Cryptomator requires FUSE-T's qualified FSKit transport on macOS
26 or later, enabled under System Settings > General > Login Items & Extensions
then File System Extensions. Packages require at least macOS 26; native dependency
deployment targets can raise the app minimum further.
The build never installs FUSE or changes system permissions:

```bash
python scripts/build-macos-backends.py --jobs 4
python scripts/build-macos-dmg.py
```

`--fuse-runtime fuse-t` is accepted explicitly; macFUSE selection is rejected.
The build records both FUSE-selection flags, runtime implementation/version and
FUSE API version. Verified source, patched inputs and build specifications keep
old native objects from being recertified under different inputs. The packager
rejects a macFUSE VeraCrypt manifest, while its generic dependency auditor still
supports macFUSE for the directory engines.

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

## Packaged lifecycle on Buran

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
