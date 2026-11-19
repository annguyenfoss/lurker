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

After applying any package-manager or repository signatures, generate a
detached signature for the final portable archives and native package hashes
with the publisher's release key. From the release directory on a signing
host, enumerate the exact files to publish:

```sh
sha256sum lurker_0.1.0_amd64.deb lurker-openbsd-0.1.0.tgz > SHA256SUMS
gpg --local-user "$RELEASE_KEY_FINGERPRINT" --detach-sign \
    --output SHA256SUMS.sig SHA256SUMS
```

Verify the signature with an independently trusted publisher keyring before
checking the downloaded artifacts. This example uses GnuPG and GNU coreutils.
Repository and native package-manager trust continue to follow each
distribution's own verification rules.

```sh
gpgv --keyring /absolute/path/to/trusted-publisher.gpg SHA256SUMS.sig SHA256SUMS &&
    sha256sum --strict -c SHA256SUMS
```

An Arch KVM test signed the coordinated `a60e4bc` native package checksum
with a disposable key. Both verification steps passed. A modified checksum
file failed signature verification and a modified artifact failed the
checksum check. Evidence is under `target/goal-validation/release-signature/`.
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
