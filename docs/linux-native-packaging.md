# Linux native packages

[Debian and Ubuntu packages](unix-packaging.md#debian-and-ubuntu-packages) and
portable archives use the same verified staging payload. Build the staging
archive on the intended distribution guest, then supply its SHA-256 and product
commit to the native wrapper.

## Fedora and openSUSE Leap

Install `rpm-build` on the build guest. As the login user, run:

```bash
python scripts/build-linux-package.py \
  --archive target/packages/lurker-native/package.tar.gz \
  --sha256 FULL_ARCHIVE_SHA256 --source-commit FULL_SOURCE_SHA \
  --version 0.1.0 --output target/packages/lurker-0.1.0-1.x86_64.rpm
```

The wrapper chooses distribution-specific runtime dependency names. RPM
packaging preserves the private payload byte-for-byte and uses explicit
dependencies so its private libraries do not become public system providers.
The build receipt records the RPM's own digest. Use the native package manager
to verify and install the package.

The RPM includes public CLI and desktop entries and the private helper under
`/usr/lib/lurker`. Administrator authorization and FUSE device access follow
the host policy. Sign release packages through the distribution's normal
process before publishing them to a repository.

## Arch Linux

Use the native `makepkg` tooling on Arch. Run the same wrapper as the login
user and select a `.pkg.tar.zst` output:

```bash
python scripts/build-linux-package.py \
  --archive target/packages/lurker-native/package.tar.gz \
  --sha256 FULL_ARCHIVE_SHA256 --source-commit FULL_SOURCE_SHA \
  --version 0.1.0 --output target/packages/lurker-0.1.0-1-x86_64.pkg.tar.zst
```

After verifying the emitted package digest, use `pacman -U` on the resulting
package to install it and resolve its Arch runtime dependencies. The wrapper
disables payload stripping and preserves the private engines and manifest.

## Gentoo Portage

Use the same wrapper on the Gentoo build guest. Select a dotted numeric
version and an output file ending in `.gentoo.tar.gz`:

```bash
python scripts/build-linux-package.py \
  --archive target/packages/lurker-native/package.tar.gz \
  --sha256 FULL_ARCHIVE_SHA256 --source-commit FULL_SOURCE_SHA \
  --version 0.1.0 --output target/packages/lurker-0.1.0.gentoo.tar.gz
```

The bundle contains a local Portage repository and its sole distfile. Extract
the verified bundle under an administrator-controlled directory. Register its
`repository` directory as `lurker-local` in Portage's repository configuration,
with `gentoo` as its master. Copy the contents of its `distfiles` directory
into Portage's configured `DISTDIR`. Permit the emitted architecture keyword
for `app-crypt/lurker::lurker-local` in `package.accept_keywords`.

Run `emerge --ask app-crypt/lurker::lurker-local` to install the ebuild. It
uses the local payload and validates the included BLAKE2b and SHA-512
manifest. Portage can also create a native binary package through its
`buildpkg` workflow. Lurker's recipe requests the indicated X11 and Wayland
flags on `x11-libs/libxkbcommon` because the desktop includes both display
routes.

## Installed package qualification

Six KVM guests installed their own native packages with exact product source
`2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1`. Each test verified the installed
payload against its staging manifest. The helper and private engines were
installed within an administrator-controlled prefix. The passing lifecycle
cases exercise creation, unlock, application writes with SQLite WAL, close,
reopen, read-back, credential rejection, and read-only handling.

| Guest | Installed format | Package SHA-256 prefix | Privileged checks | Login-user checks |
| --- | --- | --- | ---: | ---: |
| Debian | dpkg | `ab0a5fd1f61e699d` | 117 | 78 |
| Ubuntu | dpkg | `50c9b3038dea2eb1` | 39 | 52 |
| Fedora | RPM | `0cd1580cf43268d5` | 39 | 52 |
| openSUSE Leap | RPM | `f96631b845265698` | 39 | 52 |
| Arch | pacman | `b7c50ebbd9df32e7` | 39 | 52 |
| Gentoo | Portage gpkg | `c69117001881f459` | 39 | 52 |

Debian additionally opened an X11 desktop window through the installed
`/usr/bin/lurker-desktop` entry and exercised all three container ciphers. The
other installer tests covered AES for LUKS1, LUKS2, and VeraCrypt, plus the
fixed profiles of gocryptfs, CryFS, and Cryptomator. VeraCrypt used both the
kernel and userspace routes. The other five privileged suites ran in isolated
network namespaces. Passing login-user suites used the ordinary SSH sessions.
No separate VeraCrypt or Java executable was on the test PATH. The prepared
Gentoo guest had Go installed.

The earlier login-user retries used an unsuitable runtime directory or a
root-launched harness environment. Ubuntu's AppArmor policy specifically
denied the test's custom FUSE mount path. Ordinary login sessions passed with
the existing guest policies, and those initial results remain in the reports.
Gentoo built a local gpkg from the generated ebuild. Portage's dependency
preview identified required X11 and Wayland USE settings, and the local
binary-package merge exercised the prepared guest's CLI routes. That run
left Gentoo's installed desktop with its complete dependency set unqualified.

The six test packages were removed after verifying that the test mounts had
closed. Full build identities, case summaries, prior attempts, and remote
evidence paths are retained under `target/linux-native-installers/`.

## Coordinated native replay at a60e4bc

Each of the six Linux KVM guests built a native package from exact source
`a60e4bc69a3b78b8dd6e2297c2300a38f4508251`, with the VeraCrypt userspace
flush and CryFS local-state fixes. The build recipes verified the reusable
engine inputs and audited the staged runtime dependencies on each guest.
The install tests verified the package and installed file digests before
running the CLI suites and desktop probes.

| Guest | Installed package SHA-256 prefix | Privileged | Login user | Installed X11 window |
| --- | --- | ---: | ---: | --- |
| Arch | `843800e0d61d2443` | 117 | 78 | Passed |
| Debian | `4255150bd3f28d08` | 117 | 78 | Passed |
| Ubuntu | `da19e8de1f34fa07` | 117 | 78 | Passed |
| Fedora | `35dfc5a3a11b5c2c` | 117 | 78 | Passed |
| openSUSE Leap | `0d3d2163a8cdb3f4` | 117 | 78 | Passed |
| Gentoo | `dd32a9fbd2d066fe` | 117 | 78 | Passed |

The 1,170 passing checks covered AES, Serpent and Twofish in the container
routes, both VeraCrypt privilege modes, and the gocryptfs, CryFS and
Cryptomator fixed profiles. They exercised creation, unlock, file sync and
SQLite WAL, close, reopen, read-back, credential rejection and read-only
handling. Privileged suites ran in isolated network namespaces. Login-user
suites used ordinary SSH sessions on the prepared guests. Gentoo's local
Portage GPKG resolved its full desktop dependencies and ran the installed
launcher as UID 1300 under the official Xvfb package. Go was present on that
guest.

Debian also passed 60 additional checks using Firefox ESR 153.7.0 with
fresh profiles on VeraCrypt userspace, gocryptfs, CryFS and Cryptomator.
Each profile wrote data through the installed backend and read the same
data after reopening. Arch passed 26 additional VeraCrypt binary-keyfile,
eight cross-mode and ten loop-lease checks. All six workspace replays had
133 passes and 13 ignored tests per guest.

The initial Debian and Ubuntu test installs encountered an APT local-path
error in no-download mode. The retry ran a dependency preflight and installed
the verified local artifact with dpkg. Fedora's initial local RPM preflight
required a signature. The test retry checked the exact local artifact's
SHA-256 and used RPM's unsigned local install mode with dependency checks.
The native packages were removed after verifying the owned fixtures were
closed.

The Debian, Ubuntu, Fedora and openSUSE build run was
`20261119T184136Z-8ae08b9d`. Gentoo used `20261119T184400Z-b06b1894`.
Build receipts, all-case lifecycle summaries, native installer logs and
window results are indexed under `target/linux-coordinated-a60e4bc/`
and `target/linux-matrix-a60e4bc/arch/`.

### Arch offline install and lifecycle

An isolated Arch KVM clone installed the same native `a60e4bc` package
with `pacman -U` and native dependency checks. Both installs and all
runtime commands used a network namespace with loopback as its only
interface. The package runner found no separate VeraCrypt, gocryptfs,
CryFS, Cryptomator or Java runtime.

The offline replay passed all 117 privileged and 78 login-user checks.
The first login-user launcher passed root's runtime directory to the
login user and was rejected before creating a container. The repeat
supplied an owned private directory and used the same package. Both runs
removed the package and restored the mount and loop-device inventories.
The package SHA-256 is
`843800e0d61d24436e8e27c57216e01479bee467e789ea1f8196277cd3aaa289`.
Evidence is under `target/goal-validation/arch-offline-a60e4bc/`.

### Offline login-user replay on the other Linux guests

Debian, Ubuntu, Fedora, openSUSE and Gentoo each passed all 78 login-user
checks with the same `a60e4bc` native package digests listed above. Each
run installed the verified local package with native dependency checks,
then ran the ordinary user as UID 1300 in a network namespace containing
only loopback and loopback routes. The harness used an owned mode-0700
runtime directory. The five passes cover 390 checks across userspace
VeraCrypt with AES, Serpent and Twofish, and the fixed gocryptfs, CryFS and
Cryptomator profiles. Together with Arch, the offline login-user matrix
has 468 passes and zero failures or skips in the completed runs.

Ubuntu's first attempt used a runtime path outside the distribution's
AppArmor FUSE mount policy. The retry used a fresh private directory
under `/tmp`, which the policy permits. Gentoo's first install needed
its temporary Portage build directory created. The retry installed the
verified GPKG from the local package cache with invocation-scoped Portage
settings. The prepared Gentoo guest had Go on PATH. All five guests
reported an empty inventory for separate VeraCrypt, gocryptfs, CryFS,
Cryptomator and Java programs on the standard path.

The post-run audits verified the test packages and runtime directories
were removed, mount and loop inventories were restored, and no processes
remained in the qualification namespaces. Exact package receipts, per-case
reports, early failures and post-run audits are indexed in
`target/goal-validation/linux-offline-a60e4bc/offline-user-matrix.json`.

## Coordinated native replay at ca3c845

The six Linux KVM guests built and installed packages from
`ca3c8456e507102a1c6e2b9194981afbd48df8db`, including the RustSec dependency
updates. Each packaged private backend inventory matched all 131 entries
from its `a60e4bc` baseline. The installed manifest matched the native build
receipt, and the test verified all 250 installed entries on each guest.

| Guest | Installed package SHA-256 prefix | Privileged | Login user | Installed X11 window |
| --- | --- | ---: | ---: | --- |
| Arch | `7dba0cf603d9c162b` | 117 | 78 | Passed |
| Debian | `09066fd7d66a35bc7` | 117 | 78 | Passed |
| Ubuntu | `ff0c7eefa4b599212` | 117 | 78 | Passed |
| Fedora | `f306f3e83b1320bb` | 117 | 78 | Passed |
| openSUSE Leap | `a1eac2a6f6b88bec` | 117 | 78 | Passed |
| Gentoo GPKG | `e4538e75e53fb383` | 117 | 78 | Passed |

All 1,170 lifecycle checks and six 1280×860 X11 window checks passed. Each
test installed the package and exercised the native routes in a network
namespace with loopback as its only interface. The login-user suites
and private X11 probes ran as UID 1300. The lifecycle checks exercised
creation, unlock, file and directory sync, SQLite WAL, close, reopen,
read-back, incorrect credentials and read-only handling. They covered AES,
Serpent and Twofish in both VeraCrypt routes and in LUKS1 and LUKS2, plus
the fixed gocryptfs, CryFS and Cryptomator profiles. The exact workspace
suite passed 135 tests with 13 ignored on each guest.

The initial cleanup compared whole-guest mount and loop inventories
and reported a difference. A follow-up checked all 15 owned fixture roots
per guest and found zero owned mounts, loop devices, or live processes
in the qualification namespaces. It removed the private runtime directories
and test packages. All six native package inventories matched the baseline.
Arch also removed all 30 temporary signed X11 dependencies. Gentoo used an
invocation-scoped local Portage repository and installed the locally verified
GPKG. Its recipe archive SHA-256 is
`035682ebe27018c90e694d816004e310fd13b60031e0f129c17b400de4a53f95`.

The native wrapper runs were `20261119T202302Z-74e64899` and
`20261119T202448Z-19b8ff07`. An initial wrapper guard expected too many
private files and stopped after the portable builds. The corrected wrapper
compared the complete 131-file inventories, using the existing verified
portable artifacts. Exact native archives, installed digests, per-case
receipts and post-run audits are indexed in
`target/linux-coordinated-ca3c845/matrix.json`.

## Coordinated native replay with the LKL stable update

The six KVM guests rebuilt their native packages from
`1e04b8523bb6d39ca73e1c9e09d36b02f3f8932e` in run
`20261119T210239Z-0ecd5fc7`. The cold Arch build verified 87,060 LKL
source inputs and applied the pinned Linux 6.12.118 stable patch before
compiling. All six packages used the resulting `lurker-fs` SHA-256
`34eccca4ab65712f4e14e80023f09b0b4bc42518a8046ece175968a8d99c0ebe`.
The private FUSE closure matched the previous qualified build byte for
byte. Both VeraCrypt executables also reproduced their prior hashes in
the cold build. The native package manifests retain the stable patch
identity and verify all 251 installed entries per guest.

The helper and its private FUSE dependency closure require glibc symbols
through `GLIBC_2.38`. The host glibc versions were 2.40 on openSUSE,
2.41 on Debian, 2.43 on Gentoo, and 2.44 on Arch, Fedora, and Ubuntu.
The exact helper loaded and returned protocol `lurker-userspace-4` on
every guest using its packaged private library search path.

| Guest | Installed package SHA-256 prefix | Privileged | Login user | Installed X11 window |
| --- | --- | ---: | ---: | --- |
| Arch | `bbd412b644533aa8` | 117 | 78 | Passed |
| Debian | `e62ad6ce010abc1c` | 117 | 78 | Passed |
| Ubuntu | `cb8cef3c6b6d885c` | 117 | 78 | Passed |
| Fedora | `0131b9e145d403e2` | 117 | 78 | Passed |
| openSUSE Leap | `0c163cfb6a9fba48` | 117 | 78 | Passed |
| Gentoo GPKG | `00cf19c6d7fe6069` | 117 | 78 | Passed |

All 1,170 lifecycle checks passed with zero failures or skips. The package
install and lifecycle cases ran offline in private network namespaces.
The workload checked creation, file and directory sync, SQLite WAL, normal
close and reopen, credential rejection, and read-only handling. It covered
AES, Serpent, and Twofish on LUKS1, LUKS2, and both VeraCrypt routes, plus
the fixed gocryptfs, CryFS, and Cryptomator profiles. The 78 login-user
checks and the 1280×860 X11 smoke test on each guest ran as UID 1300.

The cleanup checked all 15 owned fixture roots per guest and found zero
owned mounts, loop devices, or remaining namespace processes. All native
package inventories matched their pre-test snapshots after removal,
including Arch's 30 temporary signed X11 dependencies. The only whole-guest
mount difference was the disappearance of the temporary
`/run/user/1300` mount. The loop-device inventories matched.

Exact native archives, the Gentoo recipe and installed GPKG, all six ELF
reports, and the post-run cleanup records are indexed in
`target/linux-coordinated-1e04b85/matrix.json`. The Arch full portable
archive SHA-256 is
`55bec5eed39951786cfc6f693d1aea40b0809afa27a8b80f71b443458a2267ce`.

## Clean Debian installation and offline runtime

A dedicated KVM guest booted from a verified copy of Debian's published
`debian-13-genericcloud-amd64-20261103-2651.qcow2` image. It ran kernel
`6.12.115+deb13.1-cloud-amd64`. The source image matched Debian's SHA-512
listing. The fresh guest exposed no separate VeraCrypt, gocryptfs, CryFS,
Java or Cryptomator executables. The isolated network had no default routes.

The Debian `1e04b85` installer SHA-256 was
`e62ad6ce010abc1ceea1152addb9588be68b24e02b5a5b8c448311cbb5967fd0`.
APT resolved 84 native prerequisites, including the X11 test tools, against
the clean package inventory on a separate prepared guest. The clean guest
verified the downloaded files and used `dpkg` to install the native package
and prerequisites offline after APT's local-file install returned a pathname
error. The original APT reports are retained.

The installed package passed all 117 privileged and 78 login-user checks in
a loopback-only namespace, plus a 1280×860 X11 window as UID 1300. The X11
probe needed a retry with its requested output under the test user's temporary
directory. Mount and loop inventories matched before and after, and removal
left the package database consistent. The installed artifact, image checksum
match and runtime receipts are under
`target/goal-validation/debian-clean-1e04b85/`.

https://cloud.debian.org/images/cloud/trixie/20261103-2651/SHA512SUMS

## Clean Arch installation and offline runtime

A new KVM guest used an untouched copy of
`Arch-Linux-x86_64-cloudimg-20261101.620452.qcow2`. Its checksum and
detached signature verified against the upstream Arch-boxes release key.
The guest ran kernel `7.2.11-arch1-1` with glibc 2.44. It had no separately
installed VeraCrypt, gocryptfs, CryFS, Java, or Cryptomator executables.

The guest verified 54 official prerequisite package signatures and
installed the prerequisites offline with `pacman`. The exact `1e04b85`
developer package SHA-256 was
`bbd412b644533aa81f6609714c07b67a5f0f1fea0678ccbde7bead35fc62f486`.
Its local install verified the archive digest and used a package-specific
unsigned policy for that invocation.

All 117 privileged and 78 login-user lifecycle checks passed in a
loopback-only namespace. The installed desktop rendered a 1280×860 X11
window. Cleanup found no fixture mounts, loop devices, or private backend
processes. The whole-guest inventory included one newly active system
`binfmt_misc` mount. The package, guest, and isolated network were removed.
Receipts are in `target/goal-validation/arch-clean-1e04b85/`.

https://geo.mirror.pkgbuild.com/images/v20261101.620452/

https://github.com/archlinux/arch-boxes

## Clean Ubuntu installation and offline runtime

A separate Ubuntu 26.10 KVM guest booted from a copy-on-write overlay
over a verified copy of Canonical's `release-20261105` amd64 cloud image.
The source image SHA-256
`9ef2243a4319e43164ab11d61e324efa0dd19ed850979d10c3a6e44feb5684c5`
matched Canonical's signed checksum file. The isolated bridge had no
default route, and the first-boot serial console supplied the guest SSH
host key. The guest ran kernel `7.3.0-10-generic` and glibc 2.44.

The Ubuntu `1e04b85` .deb had SHA-256
`cb8cef3c6b6d885ce0f663dcc61555b86275974a7b6270e157e8c59c7330a78c`.
APT resolved 59 native dependencies, including the X11 smoke-test tools,
against the pristine guest package inventory. The archive signature
checks bound the selected package indexes and the 59 archives to Ubuntu's
signed SHA-512 metadata. The clean guest verified the archives and
installed the dependencies and .deb together with dpkg in a loopback-only
namespace. All 251 installed entries matched the artifact manifest.
The guest exposed no separate VeraCrypt, gocryptfs, CryFS, Java, or
Cryptomator executables on the standard search path.

The installed package passed 117 privileged and 78 login-user checks
with zero failures or skips. The workload covered normal file and SQLite
writes, close, reopen, and read-back across all approved Linux profiles.
The X11 probe passed at 1280×860 as UID 1300 after a retry used the
harness's private output directory under `/tmp`.

The mount and loop inventories matched. Cleanup removed the tested package
and left no processes in the qualification namespaces. The guest halted
and its transient KVM domain and bridge were retired. The source image
retained its original checksum and file metadata. The signed checksums,
package index receipts, native install log, and runtime evidence are under
`target/goal-validation/ubuntu-clean-26.10/`.

https://cloud-images.ubuntu.com/releases/stonking/release-20261105/SHA256SUMS

## Clean Fedora installation and offline runtime

A new Fedora 45 Cloud KVM guest booted from an independent copy of
`Fedora-Cloud-Base-Generic-45-1.4.x86_64.qcow2`. The upstream Fedora 45
OpenPGP signature verified the stock image SHA-256
`50bdc5e70f6a24f540932d15411c1c6ae9fcfa40fb990d876d83c5cc823b2ad0`.
The running kernel was `7.2.11-300.fc45.x86_64` and SELinux was enforcing.

DNF resolved the prerequisites from local copies of the Fedora and Fedora
Updates metadata on the clean guest. The guest checked all 56 selected RPM
digests and Fedora 45 package signatures before installing them offline.
The exact Lurker `1e04b85` RPM had SHA-256
`0131b9e145d403e248883f8bbc15e948ba42b8edbc5b8bdd6638be5f711a078f`.
The developer RPM install used its verified digest, and all 251 installed
payload entries matched the manifest. The native process environment had
no separate VeraCrypt, gocryptfs, CryFS, Java, Cryptomator, or Go executable
on PATH.

The installed package passed 117 privileged and 78 login-user lifecycle
checks with zero failures or skips. The privileged tests used a loopback-only
network namespace. The ordinary login user ran on the private KVM bridge
without IPv4 or IPv6 default routes and rendered a 1280×860 X11 window.
There were no remaining fixture mounts, loop devices, or private backend
processes before removal of the tested RPM.

The guest closed its filesystems and halted. The owned transient domain
and isolated bridge were removed, the stock image still matched its
checksum, and the original Fedora guest retained its definition and disk
sources. Package signatures, artifact receipts, logs, and the native result
are under `target/goal-validation/fedora-clean-1e04b85/`.

https://dl.fedoraproject.org/pub/fedora/linux/releases/45/Cloud/x86_64/images/Fedora-Cloud-45-1.4-x86_64-CHECKSUM

https://fedoraproject.org/security/

## Clean openSUSE Leap installation and offline runtime

The stock Leap 16.1 Build3.13 cloud image matched openSUSE's published
SHA-256 and SHA-512. A dedicated KVM copy installed the exact `1e04b85`
RPM, SHA-256
`0c163cfb6a9fba486a5db6f624f35098a2ad50147812000d4db5fd6b69d9a8af`,
with 73 checksum- and signature-verified openSUSE prerequisite RPMs.

Installation and lifecycle testing ran in loopback-only network namespaces.
All 117 privileged and 78 login-user checks passed. The installed X11
window probe passed after provisioning the separately packaged signed Xvfb
test dependency. Package removal left no fixture mounts or loop devices.
The owned guest and bridge were removed, and the original domain and
image were preserved. Receipts are under
`target/goal-validation/opensuse-clean-1e04b85/`.

https://download.opensuse.org/download/distribution/openSUSE-current/appliances/Leap-16.1-Minimal-VM.x86_64-Cloud-Build3.13.qcow2.mirrorlist
