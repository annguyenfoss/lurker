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

Build on a Mac using the user's Rust and Python toolchains, Xcode command-line
tools, GNU-compatible make and pkg-config. VeraCrypt uses separately installed
FUSE-T and its development headers exclusively. macFUSE is reserved for
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
provenance. Backends are resolved relative to the running app/CLI before the
compatibility lookup for separately installed VeraCrypt. Missing private build
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

Apple encrypted disk images continue using built-in `hdiutil`/`diskutil` and need
no FUSE. Ad-hoc packages are development builds; public releases still require
Developer ID signing and notarization. Each release must qualify the exact
FUSE/OS combination and test the final package without a separate VeraCrypt
installation. Bundling alone does not prove native operation or durability.

Use only disposable `/tmp/lurker-*` files for native tests:

```bash
TMPDIR=/tmp cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1
```

Native developer qualification, macOS 27.2.1 arm64 / SDK 27.2:

The `8aac707` artifact is a functional baseline. Later fault testing exposed
plaintext fallthrough into a writable mount directory after unexpected detach;
underlay fencing and fresh native qualification are required before release.

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
