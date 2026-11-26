# Native macFUSE ext4 backing filesystem

The preferred ext4 container adapter uses FUSE-T FSKit. Its current transport
cannot provide either hardlinks or exclusive rename for CryFS block publication.
A separately installed macFUSE build of the same host driver provides an eligible
alternative backing filesystem for directory vaults, without a network mount.

Use fuse-ext2 revision `ae35afb9ab08d87c66c1e021df792b3a7c4308b0`:
https://github.com/alperakcan/fuse-ext2

Apply `fuse-ext2-macfuse-xattr.patch` only to the macFUSE variant. Its SDK uses a
five-argument getxattr callback; reject nonzero positions explicitly. Keep the
four-argument FUSE-T build separate.

Build as the login user with the separately installed macFUSE SDK and e2fsprogs:

```sh
./configure --prefix=/absolute/private/stage \
  CPPFLAGS="$(pkg-config --cflags fuse) -I/opt/homebrew/opt/e2fsprogs/include -D_FILE_OFFSET_BITS=64" \
  LDFLAGS="-L/usr/local/lib -Wl,-rpath,/usr/local/lib -L/opt/homebrew/opt/e2fsprogs/lib" \
  LIBS="$(pkg-config --libs-only-l fuse)"
make -C fuse-ext2 -j1 fuse-ext2
codesign --force --sign - fuse-ext2/fuse-ext2
otool -L fuse-ext2/fuse-ext2
```

Build the CLI target only; the historical optional Objective-C installer is not
needed. Install the reviewed executable separately as
`/usr/local/libexec/lurker-filesystems/fuse-ext2-macfuse`, owned by root:wheel,
mode 0755. Never execute the user-writable build with sudo or bundle the driver
with Lurker. The macFUSE kernel extension remains a host prerequisite.

For a newly created disposable filesystem, e2fsprogs can initialize the root's
login-user ownership with `-E root_owner=UID:GID` and a private empty `-d` seed.
Mount with `fuse-ext2-macfuse IMAGE MOUNTPOINT -o rw+,debug`; the debug option
keeps this development driver in the foreground. Normal unmount must precede
closing its image. Do not change ownership of an existing filesystem on open.

The fresh native round passed gocryptfs, CryFS and Cryptomator payload lifecycles
on this backing route. This does not establish journal replay, ACL or complete
ext4 feature qualification; the exact host, binaries and limits are recorded in
`DIRECTIVES/PLAN/NATIVE_FRESH_20261127/MACOS_RESULTS.md`.
