# Optional macOS UFS2 reader build

Upstream: https://github.com/realchonk/fuse-ufs

The tested source is `ee5c4a447da98220a11edb69a225d51379afc4fb` (0.6.0,
BSD-2-Clause). Apply `fuse-ufs-darwin-foreground.patch` to that checkout. Copy
the locked `fuser` 0.16.0 crate into a private build directory and apply
`fuser-0.16-macos-libfuse3.patch` to that copy. The crate archive SHA-256 is
`0bb29a3ae32279fe3e79a958fe01899f5fb23eadccee919cf88e145b54ed9367`.
Do not modify the shared Cargo registry. Build as the login user with the
genuine macFUSE SDK:

```sh
git apply /path/to/fuse-ufs-darwin-foreground.patch
PKG_CONFIG_PATH=/usr/local/lib/pkgconfig \
PKG_CONFIG_LIBDIR=/usr/local/lib/pkgconfig \
cargo --config 'patch.crates-io.fuser.path="/absolute/private/fuser-0.16.0"' \
  build --offline --release -p fuse-ufs
```

The patch removes the unavailable Darwin `nix::unistd::daemon` import/call and
requires foreground mode (`-f`). It does not change filesystem parsing or add
writable macOS support. The fuser patch selects its existing libfuse3 mounting
backend on Darwin while retaining the macFUSE protocol layout. macFUSE 5.3+
disables the old `fuse_mount_compat25` entry point used by fuser's default
Darwin libfuse2 backend; its failure can report stale `ENOTTY`. See
https://github.com/cberner/fuser/issues/752 and the installed 5.5.0 library source:
https://raw.githubusercontent.com/macfuse/library/e9c0433a0ef4694802474c82f47ca024a572153b/lib/helper.c

Review the lockfile change: only fuser's source identity should change. Add
`--locked` for subsequent builds. Verify `otool -L` reports
`/usr/local/lib/libfuse3.4.dylib`. On Buran with macFUSE 5.5.0, the patched reader
mounted a populated disposable UFS2 image read-only, returned its expected
payload hash, and exited successfully after ordinary `diskutil unmount`.
Lurker's separate CLI import test covers the encrypted-container lifecycle.

macFUSE is an optional filesystem-driver prerequisite, installed separately.
The existing VeraCrypt encryption transport continues to use FUSE-T.
