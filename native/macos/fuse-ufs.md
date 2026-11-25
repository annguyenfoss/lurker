# Optional macOS UFS2 reader build

Upstream: https://github.com/realchonk/fuse-ufs

The tested source is `ee5c4a447da98220a11edb69a225d51379afc4fb` (0.6.0,
BSD-2-Clause). Apply `fuse-ufs-darwin-foreground.patch` to that checkout, then
build as the login user with the real macFUSE SDK:

```sh
git apply /path/to/fuse-ufs-darwin-foreground.patch
PKG_CONFIG_PATH=/usr/local/lib/pkgconfig cargo build --release --locked -p fuse-ufs
```

The patch removes the unavailable Darwin `nix::unistd::daemon` import/call and
requires foreground mode (`-f`). It does not change filesystem parsing or add
writable macOS support. The patched source builds on Buran; native mounting
still needs qualification. The initial fuser 0.16/libfuse2 mount attempt fails
with `ENOTTY` against macFUSE 5.5.0, so build success is not a mount-support claim.

macFUSE is an optional filesystem-driver prerequisite, installed separately.
The existing VeraCrypt encryption transport continues to use FUSE-T.
