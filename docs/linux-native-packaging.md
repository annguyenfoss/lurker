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
