# OpenBSD LKL kernel provenance and final cut

Lurker must remain non-invasive software suitable for ordinary mainstream OS
repositories. **Novel means kernel code or behavior invented by Lurker.**
Applying established pre-made patches does not count as novel, including
kernel-source patches applied specifically for Lurker. Existing supported
parameters and configuration, including selection before building, are allowed.
There is no blanket ban on kernel patches or userspace integration. Eligibility
depends on provenance, maturity and actual effect, not a "Lurker-specific" label
or a file's location in a userspace tool.

Inventing and validating kernel ABI fields, lifecycle behavior or an
experimental kernel port is outside this project. Our timer-draining concept
is conventional, but Lurker invented this inserted ABI field and its shutdown
placement; neither came from established pre-made upstream/mainstream guidance.
This includes LKL kernel source compiled into a native process.

The experimental OpenBSD LKL route is **CUT**, not queued for resumption.
The latest user instruction supersedes the earlier non-interruption rule:
partial, in-progress and planned out-of-scope work must end immediately. A fresh
read-only root/user process inventory found no task coordinator, compiler,
control, worker or descendant still running, so no signal was necessary.
No experimental work was restarted, reverted, deleted or executed for this cut.
Source, exact outputs and all first failures remain preserved.

The custom kernel-dependent library/client/driver work and its proposed
formatter/mount/install/raw-provider/product stages have been removed from the
actionable plan. There is no kernel workaround, upstream-PR project, maintained
kernel variant or wait for a new kernel interface in this task. Ordinary
qualification keeps its original artifact identities; independent userspace
observer work is not cut by this decision.

## Completed, partial and planned work

- Completed: the custom shutdown/UAPI adaptation, two native library identities,
  generated ABI/RAM contracts, upstream 35-test RAM suites and the recorded
  primitive controls. These are historical experimental results, not an
  eligible filesystem route under the current boundary.
- Partial and cut: four API26 client/callback source files. Normal RAM callback
  and unmounted constructor controls completed before the earlier scope audit;
  callback fault modes, live guardian admission and actual service integration
  did not. The four files remain preserved and untracked, with a private exact
  source archive; they are not queued for further testing or completion.
- Planned and cut: additional kernel/source work, importing the syscall-bounds
  fix, a first file-backed formatter/mount pilot, driver installation, raw
  providers and Lurker capability/matrix admission through this experimental
  route. No such mount, format, installation or admission occurred.
- Independent and complete: the native-only observer controls and signed native
  ext4 formatter inventory. The observer has no LKL header/library/runtime
  dependency; its work was not stopped. The formatter assessment was read-only.

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

Local host build, linker and header glue is not excluded merely because it was
written for Lurker. Its actual effect and established provenance need individual
assessment. This cut follows the confirmed invented kernel behavior and ABI.

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
ordinary host prerequisites, independent of the experimental-route cut.

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
same custom kernel member and modified generated ABI. Its route-admission plan
is also cut. Its standalone stubbed codec test is independent of a running kernel;
its RAM tests remain historical diagnostics of the recorded custom archive.
Neither artifact may be relabeled as an unmodified or published-kernel result.
The proposed NetBSD syscall-bounds kernel fix was never imported or built here.

## Independent work boundary

The native mount observer uses only OpenBSD headers, process spawning, pipes,
clock and native mount inventory APIs. It can be reviewed/tested independently,
without linking LKL, running a kernel or mounting anything. The installed signed
OpenBSD e2fsprogs1.47.3 inventory/manual assessment is likewise independent and
has not formatted storage. Native FUSE callback/client source and snapshots
are preserved, and kernel-dependent callback execution/admission is cut.

Detailed exact before/after file hashes, controller/source snapshots and stopped
process inventory are retained in the host evidence directory at
`build/lkl-native/new-kernel-scope-audit/dependency-audit.json` and
`held-source-before-review.json`. The final fresh process inventory and cut
receipt are in `build/lkl-native/final-cut-audit/terminal-jobs.json`. Earlier
test receipts remain intact.

Primary published references inspected earlier:

- https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
- https://github.com/freebsd/freebsd-ports/tree/46cf3d9696491c61d91ecd22db11c7f7c1e62c24/filesystems/lkl
- https://github.com/lkl/linux/blob/d0f76a77e8c2ca68bd7d96190b8da09040eab072/arch/lkl/kernel/setup.c
- https://github.com/lkl/linux/blob/d0f76a77e8c2ca68bd7d96190b8da09040eab072/arch/lkl/include/uapi/asm/host_ops.h
