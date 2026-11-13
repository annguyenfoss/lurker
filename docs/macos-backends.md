# macOS private encryption backends

`scripts/build-macos-backends.py` builds a pinned native VeraCrypt backend and
static wxWidgets under `target/`, without installing either application. Source
archives are SHA-256 checked. The build records target, compiler, SDK, FUSE
version, source URLs/hashes and staged artifact hashes in `provenance.json`.
Its text interface uses the existing VeraCrypt profiles and stdin credentials.
The shared `native/veracrypt/utf8-console.patch` preserves complete Unicode paths
in libc++ volume listings; its digest and patched input hashes are recorded and
checked on cache reuse. It changes no encryption or storage format.

Build on a Mac using the user's Rust and Python toolchains, Xcode command-line
tools, GNU-compatible make and pkg-config. Install FUSE-T separately, including
its development headers for building. The build never installs FUSE or changes
system permissions:

```bash
python scripts/build-macos-backends.py --jobs 4
python scripts/build-macos-dmg.py
```

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
dependency paths are checked before nested code is signed. The FUSE-T library is
an explicit external dependency; unrelated Homebrew/developer paths are not
allowed to survive packaging.

The private VeraCrypt executable alone receives the hardened-runtime library
validation exception required to load another developer's FUSE runtime. The
desktop, CLI and internal Rust helper retain library validation. This follows
Apple's entitlement contract:
https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.disable-library-validation

Apple encrypted disk images continue using built-in `hdiutil`/`diskutil` and need
no FUSE. Ad-hoc packages are development builds; public releases still require
Developer ID signing and notarization. Each release must qualify the exact
FUSE-T/OS combination and test the final package without a separate VeraCrypt
installation. Bundling alone does not prove native operation or durability.

Use only disposable `/tmp/lurker-*` files for native tests:

```bash
TMPDIR=/tmp cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1
```

Native development evidence on macOS 27.2.1 arm64 / SDK 27.2:

- The app/DMG builds, nested signatures and dependency checks pass; the packaged
  private VeraCrypt, gocryptfs and Java helper start without external Go/Java/
  VeraCrypt. The offline native rebuild reuses verified source/wx caches.
- Apple images pass the packaged CLI lifecycle, SQLite WAL and safety suite.
  Native Mac core unit tests cover raw-image detection and hardlink identities.
- VeraCrypt AES/Serpent/Twofish each reached application I/O through the packaged
  CLI. Subsequent tests found/fixed Unicode listing and hardlink safety bugs.
  The final FUSE-T 1.2.9 run still failed during native creation with an operation
  timeout. This combination remains unqualified; a successful small-image run
  does not establish reliable create/mount behavior across sizes or load.
- gocryptfs requires macFUSE, absent on that host. CryFS/FUSE-T cannot enable
  SQLite WAL; Cryptomator's experimental upstream FUSE-T provider also failed WAL.
  Cryptomator uses its macFUSE provider in production. These directory backends
  still need native application/lifecycle qualification with compatible macFUSE.

These are development results, not a full macOS support or public-release claim.
