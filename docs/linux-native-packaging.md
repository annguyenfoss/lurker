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
`/usr/lib/lurker`. It does not grant administrator access or alter FUSE
permissions. These are local build artifacts. Sign packages through the
distribution's normal release process before publishing them to a repository.

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
