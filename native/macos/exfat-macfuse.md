# Native macFUSE exFAT backing filesystem

Apple's exFAT FSKit route does not provide the new-file exclusive publication
primitive required by CryFS. A separately installed native macFUSE exFAT driver
qualifies as an alternative backing filesystem for directory vaults. The
container's selected filesystem and the directory vault's backing filesystem
remain separate concepts. Lurker does not select or bundle this host driver.

Use the mature GPL-2.0-or-later Free exFAT implementation, v1.4.0, revision
`0372fdeab2b878736fa20c9c510a0766a7996cdc`:
https://github.com/relan/exfat
https://github.com/relan/exfat/releases/tag/v1.4.0

Apply `exfat-macfuse-exclusive.patch`. It implements the macFUSE 2 `renamex`
callback for ordinary rename and Darwin `RENAME_EXCL`. The upstream driver
always invokes `fuse_main` with `-s`, so its destination check and namespace
mutation run on one dispatcher. Unsupported swap flags are refused. This patch
must not be applied to a multithreaded dispatcher without a namespace lock.
The upstream file and directory fsync callback flushes nodes and filesystem
metadata, then invokes `exfat_fsync` on the backing device/file.

Build as the login user with macFUSE's separately installed FUSE 2 SDK. When
FUSE 3 is also installed, use a private pkg-config directory containing only
macFUSE's `fuse.pc`; do not accidentally select FUSE-T or FUSE 3:

```sh
git clone --branch v1.4.0 https://github.com/relan/exfat.git
cd exfat
git rev-parse HEAD
patch -p1 < /absolute/path/to/exfat-macfuse-exclusive.patch
autoreconf --install
mkdir /absolute/private/macfuse2-pc
cp /usr/local/lib/pkgconfig/fuse.pc /absolute/private/macfuse2-pc/
PKG_CONFIG_PATH=/absolute/private/macfuse2-pc \
PKG_CONFIG_LIBDIR=/absolute/private/macfuse2-pc \
  ./configure --prefix=/absolute/private/exfat-stage
make -j1
otool -L fuse/mount.exfat-fuse
```

The reviewed Buran executable links `/usr/local/lib/libfuse.2.dylib` and
`/usr/lib/libSystem.B.dylib`. Install it using the trusted system `install`
program as `/usr/local/libexec/lurker-filesystems/fuse-exfat-macfuse`, owned by
root:wheel, mode 0755. Retain its source revision, patch and binary hashes,
license and build provenance with the host prerequisite. Do not run a
user-writable build with sudo. macFUSE remains separately installed; no FUSE or
filesystem driver is packaged in Lurker.

For an already identified, disposable, login-user-owned raw filesystem image,
`mkfs/mkexfatfs IMAGE` creates an exFAT filesystem. The tested mount command is:

```sh
/usr/local/libexec/lurker-filesystems/fuse-exfat-macfuse -d \
  -o rw,uid=501,gid=20,umask=077,fmask=177,dmask=077 IMAGE MOUNTPOINT
```

Use the actual login UID/GID. The mount masks provide 0700 directories and 0600
files; they do not demonstrate per-file Unix permission changes on exFAT. Check
the actual local `macfuse` mount and exFAT boot signature before writing. Close
vaults normally before unmounting the backing filesystem and wait for its driver
to exit. Never format a guessed device or replace an existing mounted driver.

Fresh controls passed four downloaded-payload opens across normal backing
unmount/remount, private modes, file/parent fsync, new exclusive publication and
sequential collision refusal. The installed frozen `3bfed46` package passed all
three directory engines on this backing: four payload opens, append, second file
and repeated hash verification for each. The source/binary receipts remain in
`target/goal-validation/native-fresh-20261127/hosts/macos/`.

The 16-way exclusive-publication stress control retained one winner and all 15
losing source files. Some race losers received ERANGE instead of EEXIST from the
installed macFUSE high-level path. The matching upstream library source has a
positive EEXIST precheck before its negative-errno reply wrapper; this is the
inferred explanation, not a verified source identity for the installed macFUSE library.
No installed macFUSE library was changed. The exact errno contract is not a
pass, and these normal persistence results do not qualify concurrent vault
writers or crash recovery.
https://github.com/macfuse/library/blob/7a6cdd2b6e11071a706f4295aa931c65223ba953/lib/fuse.c
