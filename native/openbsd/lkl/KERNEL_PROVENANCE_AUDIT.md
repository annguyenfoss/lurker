# OpenBSD LKL kernel provenance hold

The clarified user scope permits only popular/upstream/mainstream, already
published and tested kernel changes. It excludes Lurker-specific kernel patches
and their tests. This includes LKL kernel source compiled into a native process;
being outside the OpenBSD boot kernel does not create an exception.

The present OpenBSD LKL route is **held**. No further kernel-source delta,
new kernel-dependent build/control, filesystem admission or mounted pilot will
use these artifacts under the clarified scope. Completed diagnostics, first
failures, sources and exact outputs remain preserved under their original
identities. Ordinary encryption qualification retains its separate identities.

## Exact input and patch provenance

The unmodified upstream source pin is
`d0f76a77e8c2ca68bd7d96190b8da09040eab072` (Linux6.12), archive SHA-256
`ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168`.
The local six-file adaptation is `native/openbsd/lkl/host.patch`, committed in
`440e63b`, SHA-256
`8d418903144152e49276e2b2b52071d31170bd7f0b29bdee926ed1bbbba63be6`.
It was authored and tested in this task, using sibling local BSD lifetime models.
Those local models do not establish independent mainstream/upstream or official
port publication. No qualifying published provenance has been established for
the added kernel shutdown hook and its UAPI field.

| Patched upstream file | Classification and dependency |
| --- | --- |
| `arch/lkl/kernel/setup.c` | Bespoke kernel source: calls the new timer-quiescence hook after CPU/thread/tick shutdown and before `free_mem`. The current timer lifetime model relies on that placement. |
| `arch/lkl/include/uapi/asm/host_ops.h` | Bespoke kernel UAPI/ABI: inserts `timer_quiesce` into the operations structure before subsequent fields. All kernel/host objects and exported headers were rebuilt together. It is not ABI-compatible with an unmodified kernel object simply by changing the host library. |
| `tools/lkl/lib/posix-host.c` | Custom host implementation: OpenBSD pthread timer, stack and safe-mapping helpers; binds the timer drain into the custom UAPI. Host-side portability alone does not remove that kernel dependency. |
| `tools/lkl/include/lkl.h` | Custom host FFI header: isolates/restores native signal macros around generated Linux declarations. The generated SDK it imports still contains the modified host-operations ABI. |
| `tools/lkl/Makefile.autoconf` | Custom build configuration: OpenBSD target/native tool format/pthread linking, host network backends disabled, upstream MMU/KASAN options selected off. |
| `tools/lkl/Makefile` | Custom build tooling: private GNU stat/sed aliases for native OpenBSD tools. |

No Kconfig source file was changed. Selecting existing upstream configuration
options is distinct from patching source; it does not cure the shutdown/UAPI
changes. Inherited CONFIG_NET=y and enabled EXT4/JBD2/Btrfs/XFS were recorded
honestly. The native boot kernel remains the official OpenBSD GENERIC.MP#14
with recorded official syspatches; that fact does not exempt the LKL changes.

The added `openbsd-timer.h` and `openbsd-memory.h` are custom native host
components, committed in `b669ad9` and `188ee65`. Their standalone pthread/mmap
controls are independent host primitives. The timer as integrated into this
library depends on the bespoke kernel hook; its standalone test success is not
proof of an eligible unmodified-kernel lifetime contract. The private byteswap
alias header is host compatibility glue. The ELF overlay is unchanged published
glibc2.42 file-format declarations, SHA-256
`fd6e7f3e3c74ee9d64b9f92ce3cc31b34413ca2622a167320ea053ce9a68110f`,
not a copied glibc runtime. Native GNU gcc/binutils/bc and OpenBSD base FUSE are
ordinary host prerequisites, independent of the eligibility hold.

## Artifact dependency

The original feature-zero library is SHA-256
`d07dbeda5039932129d5a66dd232fd8d92c5d4e0aafecdbb98f718fe0070d4d3`.
Its kernel member is SHA-256
`5713c84878cafc4d0b9bde493cd3d9971831f657ff4d43caf3fb6e7c4050ef83`.
It contains the bespoke kernel source/UAPI changes above.

The separate opt-in FLUSH library is SHA-256
`0e9e795d0e50abb70e47a58908f83251cf3c13101b68931a2c9858116dbf0e1c`.
Its new patch `native/openbsd/lkl/flush.patch`, committed in `61cb41b`, SHA-256
`2079b42fad7998d69d8fa4aa9739c68ea614f33318b8fbeab1713af2929c444f`,
changes only `tools/lkl/lib/virtio_blk.c`: explicit custom-callback FLUSH opt-in,
two-buffer requests, native status forwarding and malformed-request checks.
That is a host transport change. However, the resulting archive reuses the exact
same custom kernel member and modified generated ABI. It is also held for route
admission. Its standalone stubbed codec test is independent of a running kernel;
its RAM tests remain historical diagnostics of the recorded custom archive.
Neither artifact may be relabeled as an unmodified or published-kernel result.
The proposed NetBSD syscall-bounds kernel fix was never imported or built here.

## Independent work boundary

The native mount observer uses only OpenBSD headers, process spawning, pipes,
clock and native mount inventory APIs. It can be reviewed/tested independently,
without linking LKL, running a kernel or mounting anything. The installed signed
OpenBSD e2fsprogs1.47.3 inventory/manual assessment is likewise independent and
has not formatted storage. Native FUSE callback/client source and snapshots
are preserved, but kernel-dependent callback execution/admission is held.

Detailed exact before/after file hashes, controller/source snapshots and stopped
process inventory are retained in the host evidence directory at
`build/lkl-native/new-kernel-scope-audit/dependency-audit.json` and
`held-source-before-review.json`. Earlier test receipts remain intact.

Primary published references inspected earlier:

- https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
- https://github.com/freebsd/freebsd-ports/tree/46cf3d9696491c61d91ecd22db11c7f7c1e62c24/filesystems/lkl
- https://github.com/lkl/linux/blob/d0f76a77e8c2ca68bd7d96190b8da09040eab072/arch/lkl/kernel/setup.c
- https://github.com/lkl/linux/blob/d0f76a77e8c2ca68bd7d96190b8da09040eab072/arch/lkl/include/uapi/asm/host_ops.h
