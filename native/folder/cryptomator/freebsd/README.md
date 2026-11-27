# Native FreeBSD Cryptomator platform candidate

This directory ports the jFUSE 0.7.3 host boundary and FUSE-NIO 5.1.0 provider
to actual FreeBSD/amd64. CryptoFS 2.8.0 and the remaining locked runtime jars
retain their original bytes. The original helper still rejects FreeBSD mount
selection; shared core/helper/package admission is a later reviewed change.

The build runs a native C/header probe and C-to-Java FFM memory controls before
compiling the platform. No Linux generated ABI classes enter the output. A
native compiler supplies every consumed structure's size/alignment, field
offset/width and errno/open-flag constants. Its independent test DSO checks
92 scalar/pointer fields, all structure dimensions/offsets/constants, and all
30 registered callback upcalls. The initial native probe passed on actual
FreeBSD as UID/EUID1300; this is not a mounted Cryptomator result.
Native C-string return, pointer-field and upcall dereference controls also check
the target bounds required by the upstream wrappers.

The port derives the ordinary jFUSE Java callback wrappers from verified LGPL
release sources, rebuilds its API with genuine FreeBSD platform registration,
and adds a separate native module. The FUSE-NIO module is rebuilt to register
the FreeBSD MountService. `prepare.py` records narrow source adaptations and
rejects a changed source anchor. Existing metadata/encryption implementations
remain upstream code. Source jars, original replaced jars, transformed source,
build input hashes, license texts and native runtime legal notices are retained.
The registration probe constructs the adapter with an unused null FileStore,
without querying a filesystem. This test-only fixture does not qualify the
ordinary upstream factory or mounting. Those remain separate native gates.

FUSE3 is a separately installed native host prerequisite. No FUSE library or
filesystem tool is copied into the candidate runtime. The initial profile is
single threaded, uses actual login-user IDs, kernel permission checks and
`fuse_config.direct_io=1`, and never requests allow-other, automatic/forced/lazy
unmount, writeback cache or a Linux unmount helper. Ordinary native unmount must
succeed before closing the FUSE loop and adapter.

FUSE-NIO registers file FSYNC and forwards it to the existing channel force
operation. It does not register FSYNCDIR. The provider probe reports this upstream
limit explicitly; the port adds no success stub or new CryptoFS persistence
algorithm. Mounted file fsync/error forwarding, permissions, busy close, full
payload append/reopen and SQLite behavior remain separate native gates.

Only the FreeBSD host owner may schedule native execution. Build example:

```text
python scripts/build-freebsd-cryptomator.py --java-home /usr/local/openjdk25 \
  --source-cache <owned-cache> --output <new-owned-output> \
  --cc /usr/local/llvm19/bin/clang
```

This command performs compiler, FFM memory and provider-registration probes. It
does not mount, install, elevate, format, change a kernel, or enable a product
route. Kernel configuration and Linux Kernel Library are unrelated to this port.
