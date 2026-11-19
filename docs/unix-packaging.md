# Unix staging and package inputs

From a clean checkout at an exact commit, `scripts/build-unix.py` builds both
products, builds missing private engines, and stages the applicable locked scope:

```bash
python scripts/build-unix.py --source-commit "$(git rev-parse HEAD)" \
  --java-home "$JAVA_HOME" --output target/packages/lurker-native
```

Run as the login user with the intended toolchains already selected. Linux needs
the JDK/Maven for its private Cryptomator runtime; BSD native-only builds do not.
The builder never installs prerequisites or runs storage tests. `--cli-only`
explicitly requests a CLI-only development artifact. `--select BACKEND` limits a
development build; omitting it selects every applicable locked backend.

Pass existing engine outputs with repeated `--backend KIND=DIRECTORY` to reuse
verified native builds, for example `--backend veracrypt-userspace=/path/to/output`.
The stager still verifies their pins, patches, notices, file hashes and native
dependencies. Cached engine recipes have separate input hashes; unrelated Rust
changes reuse their native objects. Rust has its own reusable cache, or respects
an explicit `CARGO_TARGET_DIR`. A cache lock prevents overlapping orchestrator
builds; reconcile a recorded owner before removing a stale lock.

Each invocation retains stage logs and `result.json` beneath
`~/.cache/lurker/build-unix/`, printing only stage exits and result locations.
`--cache-root`, `--jobs` and `--stage-timeout` bound build resources. The remote
runner can supply `--source-manifest` for its verified deployed Git archive.
Successful packaging is build evidence; native lifecycle qualification remains
a separate required step.

See [Linux native packages](linux-native-packaging.md) for the distribution
wrappers and installed-artifact results.

The BSD package managers use native package formats. See
[native BSD packages](bsd-packaging.md) for the verified archive wrapper,
dependency declarations and installer commands.

## Debian and Ubuntu packages

Build the full Linux staging archive on the intended Debian or Ubuntu guest.
Then run the native wrapper as the login user, supplying the archive checksum
from the build receipt:

```bash
python scripts/build-linux-package.py \
  --archive target/packages/lurker-native/package.tar.gz \
  --sha256 FULL_ARCHIVE_SHA256 --source-commit FULL_SOURCE_SHA \
  --version 0.1.0 --output target/packages/lurker_0.1.0_amd64.deb
```

The wrapper requires `dpkg-deb`, checks every staged file and relative
symlink, and reports the native artifact digest. Install the resulting package
using `apt install ./lurker_0.1.0_amd64.deb` on a supported guest. The two public
programs have `/usr/bin` entry points and the desktop menu has a Lurker
launcher. The helper, engines and license inventory live under
`/usr/lib/lurker`. All installed payload bytes come from the supplied staging
archive. The archive remains a separate distribution format.

The package declares kernel-tool, FUSE 2/3, native display and Fontconfig
dependencies for current Debian and Ubuntu releases. Mount authorization and
the kernel's device support depend on the host. Installation neither grants
passwordless elevation nor loads kernel modules.

## Portable Unix archives

`scripts/stage-unix.py` stages Linux, FreeBSD, DragonFly BSD, NetBSD and OpenBSD artifacts. It does not build, install, elevate, run a payload, qualify encryption operations or change host permissions. macOS uses `scripts/build-macos-dmg.py` instead.

The package layout is:

```text
bin/lurker
bin/lurker-helper
bin/lurker-desktop                         # only with --desktop
libexec/lurker/backends/lurker-veracrypt-create
libexec/lurker/backends/lurker-veracrypt
libexec/lurker/backends/lurker-veracrypt-bsd   # BSD console engine
libexec/lurker/backends/lurker-fs
libexec/lurker/backends/gocryptfs
libexec/lurker/backends/cryfs
libexec/lurker/backends/cryptomator/        # launcher, private runtime, modules, sources/notices
share/lurker/licenses/
share/lurker/provenance/
share/lurker/manifest.json
```

Only explicitly selected applicable backends ship. Selection does not claim runtime qualification. Native LUKS/GELI/CGD/softraid/PEFS facilities remain host dependencies; private third-party runtimes must be supplied for selected VeraCrypt, gocryptfs, CryFS and Cryptomator operations. Missing required tools, runtimes, notices, provenance or shared libraries block packaging. CryFS 1.x specifically needs FUSE 2 (`libfuse.so.2` plus `fusermount`); Cryptomator and the Linux VeraCrypt userspace stack need FUSE 3 / `fusermount3`. gocryptfs accepts either Linux mount helper and uses `mount_fusefs` on FreeBSD. These are recorded per backend, rather than treating the two FUSE ABIs as interchangeable.

The Linux desktop requires a working X11 or Wayland session and at least one
readable font indexed by Fontconfig. The package embeds its UI fonts, and the
renderer also initializes Fontconfig's system collection. On minimal guests,
install a distro font package or configure Fontconfig to expose an existing
font. This requirement is recorded in `desktop_runtime.system_fonts` in
`manifest.json` and applies only to the desktop.

FreeBSD gocryptfs runs as the login user. The prepared test guest required loaded
`fusefs`, access to `/dev/fuse`, and `vfs.usermount=1` in addition to
`mount_fusefs`. An administrator must provide the intended mount/device access
policy; the package does not change it. Native qualification temporarily enabled
the required access and restored the original settings afterwards.

Build the Rust products in a clean checkout at one exact commit, then record their identities. Use the user's managed toolchains. `--source-commit` requires the full 40-character SHA. A deployed Git archive uses its verified `--source-manifest source-manifest.json` instead of Git metadata. Build outputs should be outside the immutable deployed source tree.

```bash
cargo build --release -p lurker-cli -p lurker-helper
python scripts/stage-unix.py record --kind binaries \
  --directory target/release --source-commit "$COMMIT" \
  --output target/rust-binaries.json
```

Record mode is a build-caller attestation linking hashes to the verified clean source tree; it cannot retrospectively prove how an arbitrary supplied binary was compiled. The release build runner must record immediately after its successful build and retain build logs. The stager verifies this identity and all native builder metadata, and records the final payload digests.

Backend build entry points (no installation):

- Linux VeraCrypt creator and optional userspace/LKL stack: `scripts/build-linux-userspace.py`; `--only-creator` omits the userspace stack.
- gocryptfs and stable CryFS: `scripts/build-folder-backends.py`.
- Cryptomator and reduced private JRE: `scripts/build-cryptomator.py`.
- BSD VeraCrypt: `scripts/build-bsd-backends.py` produces `lurker-veracrypt-bsd`, private `lib/`, notices and `bsd-veracrypt-source.json`.

Artifact arguments are repeated `--backend KIND=DIRECTORY`. Supported kinds are `veracrypt-create`, `veracrypt-userspace`, `veracrypt-bsd`, `gocryptfs`, `cryfs`, `cryptomator`. The userspace payload includes its creator; do not supply the creator twice. Folder builder aggregate directories are accepted: only the named executable, its manifest and its notices are copied. Full userspace/BSD VeraCrypt and Cryptomator preserve nested assets and internal relative symlinks.

The creator's `creator-source.json` binds its binary, pinned revision and current source patch. Folder builder `gocryptfs.json` / `cryfs.json` bind revision, platform and executable checksum. Required folder patches also bind to exact selected source bytes through the builder's `PATCHES` inventory; unpatched or stale CryFS durability builds are rejected even when the upstream revision matches. Unpatched gocryptfs manifests may omit an empty patch map. Cryptomator's `manifest.json` binds the complete runtime/module tree to its dependency lock. The stager also compares its packaged `Main.java`, module descriptor and Maven project against the exact selected Lurker commit; a new helper cannot silently reuse an older private image. README-only changes do not require rebuilding Java. Full Linux userspace payloads additionally require a file envelope because the older `sources.json` lists source pins without output hashes:

```bash
python scripts/stage-unix.py record --kind veracrypt-userspace \
  --directory target/release/userspace --source-commit "$COMMIT" \
  --output target/release/userspace/artifact-manifest.json
```

BSD VeraCrypt uses its builder's `bsd-veracrypt-source.json`, which binds the pinned upstream archives, native platform, executable and private-library hashes. The stager preserves `lib/` beside `lurker-veracrypt-bsd`, copies notices/provenance, and leaves build logs and unrelated aggregate-directory executables out. Native helper protocol and actual operations need separate qualification.

A Linux example stages the privileged VeraCrypt route and native LUKS support:

```bash
mkdir -m 700 target/packages
python scripts/stage-unix.py stage --binaries target/release \
  --binary-manifest target/rust-binaries.json --source-commit "$COMMIT" \
  --select luks --select veracrypt \
  --backend veracrypt-create=target/release/userspace \
  --output target/packages/lurker-linux
```

The output parent must belong to the caller and have no group/world write permission. Existing output files, directories (including empty ones), and symlinks are never replaced. The tool constructs a completed private backing tree adjacent to the requested output, then publishes the **output prefix as one atomic symlink**. Keep its adjacent hidden backing directory. `PREFIX/package.tar.gz` contains ordinary `lurker/bin`, `lurker/libexec` and `lurker/share` directories, without the archive itself; extract this archive for an actual root-owned installation. Archive owners are normalized to uid/gid 0. `PREFIX/package.sha256` and the final JSON stdout record bind the archive bytes. A failed staging operation removes only its own unfinished backing tree.

All input roots must be real directories. External, broken or looping source symlinks, traversal paths, special files, setuid/setgid payloads and colliding destinations are rejected. Internal JRE/library symlinks are retained with relative targets. Elevated operations must still canonicalize and validate the installed executable and ancestor ownership; a user-owned staging prefix does not become trusted for elevation merely because the stager created it.

Dependency audit uses the native `readelf`/`llvm-readelf` metadata reader, never `ldd` or an executable's version flag. It checks native ELF machine type, interpreter, RPATH/RUNPATH and recursively resolves linked libraries. Current-architecture ELF resources embedded in JARs are included. Missing libraries and developer/relative library search paths fail. Supplied private libraries remain private; host dependencies are explicitly recorded with resolved paths and hashes. The generated manifest deliberately says `self_contained: false`: packaging cannot supply kernel crypto, mount authorization, FUSE devices or PEFS kernel modules. The private JRE has one explicit loader qualification: its Java launcher/JLI loads `runtime/lib/server/libjvm.so` before the known java.base libraries (`libjava`, `libzip`, `libnet`, `libverify`, `libjimage`) need that SONAME. This relationship is accepted only with the verified Cryptomator manifest, native launcher/JLI/JVM, `-server KNOWN` configuration and matching JVM SONAME. Such edges are labeled `preloaded-by-private-java-launcher`; no arbitrary whole-prefix library search is allowed. Native packaged Java/Cryptomator tests must qualify the loader behavior. Metadata inspection does not prove every other `dlopen` path, plugin, kernel feature or mount permission; native packaged-artifact tests remain required.

Run staging unit tests on a test host:

```bash
python -m unittest discover -s scripts/tests -p test_stage_unix.py
```

Tests cover external symlinks, traversal, special/setuid files, internal runtime links, source/binary identity, missing runtimes, dependency failures and competing publication. They execute no encryption operations.

## Native package download sizes

These sizes are for the digest-verified native installer files built on KVM.
The Linux artifacts use `c21e0cc` and the BSD artifacts use `ca3c845`.
Distribution prerequisites are separate downloads.

| Package | Size |
| --- | ---: |
| Arch Linux pacman | 79.5 MiB |
| Debian DEB | 69.4 MiB |
| Ubuntu DEB | 71.3 MiB |
| Fedora RPM | 71.4 MiB |
| openSUSE RPM | 71.4 MiB |
| Gentoo Portage GPKG | 79.5 MiB |
| FreeBSD PKG | 17.2 MiB |
| OpenBSD TGZ | 21.5 MiB |
| NetBSD TGZ | 22.3 MiB |
| DragonFly BSD PKG | 12.2 MiB |

Exact sizes and digests are in
`target/goal-validation/native-package-download-sizes.json`.

## Vendored Rust source archive

The native Unix builder collects the Rust dependency inventory from the
Cargo build log for the packaged products. It ships the deposit under
`share/lurker/licenses/rust/`, with license declarations and the notice
files found in each checksum-verified crate. The binary provenance binds
the inventory and individual files to the build. Build-time crates are
included. For a manually staged Cargo build, pass its JSON build log to
`stage-unix.py record --kind binaries --cargo-log` and keep the resulting
`rust-notices/` directory next to the binary manifest. Use Cargo's
`--message-format=json-render-diagnostics` when making that log.

On Arch KVM, the combined package build recorded 372 compiled dependency
crates and 647 notice files. The CLI-only build recorded 22 crates and 46
notice files. Fifteen crates in the desktop build supplied license
declarations without separate notice files in their registry archives.
Their declarations remain visible in the inventory for source review.
Both builder and staging suites passed, for 58 tests total. Evidence is
under `target/goal-validation/rust-notices/`.

To prepare the Rust portion of the release sources, extract a Git archive
of the exact product commit into a new directory. From that directory,
generate the vendor tree and its local Cargo configuration.

```sh
mkdir .cargo
cargo vendor --locked --versioned-dirs vendor > .cargo/config.toml
```

Archive the whole directory. Unpack it for verification and use an empty
`CARGO_HOME` with `cargo check --workspace --locked --offline` and
`cargo test --workspace --locked --offline`. Keep the lockfile, Cargo
configuration and original vendored license files in the archive.

Arch KVM assembled the exact `8bb689c` project sources with all 588
registry crates in `Cargo.lock`. Each vendored package checksum matched
the lockfile. Cargo resolved all 592 packages from the unpacked archive
with an empty Cargo home. Workspace check passed, and offline tests as the
build login user reported 136 passed, zero failed and 13 ignored. Earlier
network-namespace test attempts hit the host identity and tool-discovery
assumptions in the test fixtures. The receipts and archive are under
`target/goal-validation/rust-corresponding-source/`. Archive SHA-256 is
`0b240ba625b2a65eff78ca467420e1a07096d119e047cc17d639e93b37a5a30e`.

Native and JVM engines and their dependencies have separate source inputs.

https://doc.rust-lang.org/cargo/commands/cargo-vendor.html

## Collected native and JVM source inputs

The `8bb689c` Linux package audit verified 131 matching private backend
entries across the six native packages. A local staging inventory now
records digest-verified source archives for VeraCrypt, LKL, its Linux
stable patch, both wxWidgets builds, patchelf, CryFS and gocryptfs. It
also holds source ZIPs and module declarations for all nine linked Go
modules. See `target/goal-validation/collected-native-sources/receipt.json`.

The FreeBSD `ca3c845` VeraCrypt payload includes FreeBSD Ports builds of
libfuse 2.9.9 and libiconv 1.18. Both upstream tarballs matched `distinfo`
at ports commit `9b1792b1d9fdc91f491da6648a888173b85bab04`. The exact port
recipes and patches were archived from the same commit. The input files
and hashes are under `target/goal-validation/freebsd-port-sources/`.

The private Cryptomator runtime comes from Temurin 25.0.5+8. Its official
Linux x64 release metadata names the JDK tree at
`7ec19fb1eda7e42498b7ee4ebde29f77aa961a1c` and the Temurin build tree at
`1cf43d7a3e77886bfea7d550a902e151a87d724d`. Debian KVM archived both Git
objects and verified the commit identifiers. The JDK archive includes
the HotSpot native sources and Java class libraries. The release metadata,
archives and digests are under `target/goal-validation/temurin-source/`.

The `8bb689c` Cryptomator dependency lock covers 27 Maven artifacts. The
source bundle contains 26 source JARs and 42 published POMs, including 15
parent or imported BOM POMs. Each download matched its available Maven
Central checksum and has a recorded SHA-256. The shipped Guava
`listenablefuture` conflict marker contains only metadata. Its published
and embedded POMs are retained. The bundle and per-coordinate receipt
are under `target/goal-validation/maven-source-8bb689c/`.

The CryFS bundle contains all four locked dependency archives, the six
matching Conan recipe exports and the locked B2 and CMake build inputs.
The CMake executable matched the qualified producer. A cold offline
rebuild still needs Conan's Python dependencies and the native compiler
and FUSE development tools. See
`target/goal-validation/cryfs-conan-sources-8bb689c/`.

The remaining source assembly includes the relocated Arch library sources
and recipes. See the original package audit under
`target/goal-validation/linux-source-readiness-8bb689c/`.

https://github.com/adoptium/temurin25-binaries/releases/tag/jdk-25.0.5%2B8

https://github.com/freebsd/freebsd-ports/tree/9b1792b1d9fdc91f491da6648a888173b85bab04

## Release maintenance and signatures

Use the installed package's `share/lurker/manifest.json` and neighboring
license and provenance files to identify the shipped dependencies. The builder
pins VeraCrypt, LKL and wxWidgets in `scripts/build-linux-userspace.py` and
`scripts/build-bsd-backends.py`, and gocryptfs and CryFS in
`scripts/build-folder-backends.py`. The Cryptomator dependencies are in
`native/folder/cryptomator/dependencies.lock.json`. Record the build JDK
distribution and version as well as the private runtime manifest.

When an upstream release or advisory affects a shipped component, identify
the affected builds from their installed manifests. Review the new source,
checksums, local patches, transitive dependencies and notices before updating
the pins. Rebuild affected packages at one source commit and repeat their
native lifecycle and relevant fault checks. Supply the corresponding source
for the released binaries and dependencies, including Lurker's patches and
build inputs and the source and notices required by each component's license.
Keep the source and binary receipts together.

After applying any package-manager or repository signatures, place the
final native packages, portable archives and corresponding-source archives
in one release directory. Assemble `SHA256SUMS` from the reviewed artifact
digests. Use one lowercase SHA-256 digest, two spaces and a filename per
line. Filenames use ASCII letters, digits, dots, underscores, plus signs
and hyphens. Use the complete fingerprint of the publisher's actual signing
key or subkey.

```sh
python3 scripts/release-signatures.py sign /path/to/release \
    --signer "$RELEASE_SIGNING_SUBKEY_FINGERPRINT"
```

The signing command checks each artifact against the approved inventory
before publishing `SHA256SUMS.sig`. Prepare replacements in a fresh release
directory. `gpg` uses the signing host's existing key store and agent.

Verify using a current publisher keyring and signing fingerprint acquired
through an independent trusted channel. The verifier authenticates the exact
inventory before checking the downloaded contents. With no `--artifact`
arguments it checks every listed file. Native package-manager verification
also follows each distribution's trust rules. Keep the verification keyring
current because GnuPG's `gpgv` trusts the keyring for key status.

```sh
python3 scripts/release-signatures.py verify /path/to/downloads \
    --signer "$RELEASE_SIGNING_SUBKEY_FINGERPRINT" \
    --keyring /path/to/trusted-publisher.gpg --artifact lurker_0.1.0_amd64.deb
```

An Arch KVM test signed the coordinated `a60e4bc` native package checksum
with a disposable key. Both verification steps passed. A modified checksum
file failed signature verification and a modified artifact failed the
checksum check. Evidence is under `target/goal-validation/release-signature/`.

On Arch KVM, the signing tool from `382743d` signed a test inventory with
the exact `8bb689c` pacman package, its portable archive, and the archived
Lurker Git source tree. Verification passed for the full inventory and for
an isolated single-package download. Native `gpgv` accepted the detached
signature. Modified copies of the package and inventory were each refused.
All 255 script tests passed. The signing subkey was disposable, and its
private staging directory was removed. Receipts are under
`target/goal-validation/release-signatures-test/`.

Public signatures require the publisher's real release key.

https://www.gnupg.org/documentation/manuals/gnupg/Operational-GPG-Commands.html

https://www.gnupg.org/documentation/manuals/gnupg/gpgv.html

### gocryptfs shipped dependency check

The packaged gocryptfs v2.7.0 binaries from Arch `a60e4bc` and FreeBSD
`2fda5f7` were extracted from their hash-verified packages and inspected on
the Gentoo KVM guest with `govulncheck` v1.10.0 in binary mode. Both embed Go
1.27.4 and `golang.org/x/crypto` v0.58.0. The Go advisory database returned
one module-level match, GO-2026-5932, for the `openpgp` package family.
The scans found no matching package or symbol.

The Linux and FreeBSD dependency graphs were checked separately from pinned
source `756cd453d7afe69fd2a00a7592fd2d9869a6f756`, including the packaged
FreeBSD patch. Both include `golang.org/x/crypto/scrypt` and exclude the
affected `openpgp` packages. The module-level finding therefore does not
apply to either inspected executable. This check is scoped to the two artifact
digests and Go database snapshot dated 2026-11-11 02:51:11 UTC. Binary build
metadata, scanner output and package graphs are retained in
`target/goal-validation/go-runtime-review/`.

https://go.dev/doc/security/vuln/

https://pkg.go.dev/vuln/GO-2026-5932

### Rust workspace dependency check

On Arch KVM, the exact `a5a1857` Cargo lockfile was audited with
`cargo-audit` 0.23.0 from a signed Arch Linux Archive package. Both the
baseline and update used RustSec advisory database commit
`074ea9a7ed619f511b5b7374c7a97e98d1e98be4` and disabled database refresh and
yank checking. The baseline had 593 packages, six vulnerability records,
three unsoundness notices, and nine unmaintained notices.

| Class | Locked package update | RustSec IDs |
|---|---|---|
| Vulnerability | webbrowser 1.2.1 → 1.2.2 | RUSTSEC-2026-0257 |
| Vulnerability | pulldown-cmark 0.13.3 → 0.13.6 | RUSTSEC-2026-0414, RUSTSEC-2026-0433 |
| Vulnerability | crossbeam-epoch 0.9.18 → 0.9.20 | RUSTSEC-2026-0204 |
| Vulnerability | quick-xml 0.39.2 → 0.41.0 through wayland-scanner 0.31.10 → 0.31.11 | RUSTSEC-2026-0194, RUSTSEC-2026-0195 |
| Unsound | anyhow 1.0.102 → 1.0.103 | RUSTSEC-2026-0190 |
| Unsound | event-listener 5.4.1 → 5.4.2 | RUSTSEC-2026-0221 |
| Unsound | memmap2 0.9.10 → 0.9.11 | RUSTSEC-2026-0186 |

The Linux dependency graph includes webbrowser through Slint's winit backend
and pulldown-cmark through Slint core. Its crossbeam-epoch path goes through
the Slint compiler, and its affected quick-xml path goes through the
Wayland scanner proc macro. The graph identifies dependency selection.
Finding reachability requires separate testing.

The updated lock has 592 packages, zero vulnerability and unsoundness records
at the same database revision, and the same nine unmaintained notices:
bincode, chrono, paste, pin-utils, rustybuzz, ttf-parser, zbus_names, zvariant,
and zvariant_derive. These results cover Cargo dependencies. The packaged
native and JVM dependencies have separate provenance and maintenance checks.

The exact `ca3c845` commit passed offline `cargo check --workspace --locked`
and `cargo test --workspace --locked` on Arch KVM with Cargo/Rust 1.100.0.
Run `20261119T201634Z-4b1cae2e` reported 135 passed, zero failed and 13
ignored. The updated lockfile SHA-256 is
`e0a352930ba76478e4a263d696c6acbdef783d6872d0868eabeeb282f830f546`.
The audit JSON, tool signature, dependency graph, and workspace receipts are
in `target/goal-validation/linux-runtime-audit-a5a1857/`.

https://github.com/RustSec/advisory-db/tree/074ea9a7ed619f511b5b7374c7a97e98d1e98be4
