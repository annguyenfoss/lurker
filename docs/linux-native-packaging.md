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
binary-package merge exercised the prepared guest's CLI routes. Gentoo's
installed desktop with its complete dependency set remains unqualified.

The six test packages were removed after verifying that the test mounts had
closed. Full build identities, case summaries, prior attempts, and remote
evidence paths are retained under `target/linux-native-installers/`.
