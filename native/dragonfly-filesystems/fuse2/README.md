# Scoped native FUSE2 compatibility for the external filesystem prerequisite

This LGPL-2.1 library belongs only to the separately installed native host
filesystem driver. It must never enter Lurker's app or private crypto bundle.
The global host FUSE installation is unchanged. Source and every official DPorts
patch are pinned in `recipe.json`; upstream is https://github.com/libfuse/libfuse .

Apply the official patches, then the native patches and copy `dragonfly-init.h`
into `lib/`. Patch 0001 retains the already qualified native child-only `_exit`
and native `size_t` sysctl fixes. Run autoreconf, configure as recorded, then
build the static library with one worker.

Stock DragonFly FUSE ABI7.28 audits a 64-byte INIT reply even when FUSE2 negotiates
minor19, whose source-private reply structure is 24 bytes. Patch 0002 preserves
that prefix and adds exactly40 zero bytes when the native request minor is23 or
newer. It changes neither negotiated version nor feature flags and advertises no
extended capability. The native kernel source explicitly ignores the extension
fields. Earlier kernel negotiations retain the upstream reply behavior.

The native `tests/kernel-wire.c` probe measures the exact stock kernel source
header. The FUSE2 package does not install its private wire header; its qualified
source header is measured by `tests/init-wire.c`. Both agree on shared field
offsets and the native64-byte layout. The latter also checks preservation of the
7.19 prefix/flags and every zero extension byte. Native library compilation
passes; actual linked-driver INIT and mounted payload proof remain required.
Privileged use requires review and a verified copy into an administrator-owned
external prerequisite prefix. No loader override or user-writable executable may
be elevated.
