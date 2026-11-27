# DragonFly LKL scope cut — 2026-11-27

The current LKL prototype is cut from Lurker's actionable work. The user's
latest instruction requires stopping partial, current and planned novel kernel
work now. `DIRECTIVES/LLM/DEVELOPMENT.md` defines the integration boundary:
existing supported configuration, parameters and established mainstream
procedures are acceptable; injecting new kernel behavior is not. Source,
partial artifacts, failures and evidence remain preserved.

Here, novel means kernel code or behavior invented by Lurker. Applying an
established pre-made kernel-source patch does not count as novel, even when
applied specifically for Lurker. Existing supported parameters and configuration,
including before a build, remain allowed. This cut is not a blanket kernel-patch
or userspace ban.

## What was done and how

Commit `fbdfbc5` added `timer_quiesce` to
`arch/lkl/include/uapi/asm/host_ops.h` and called it from
`arch/lkl/kernel/setup.c` after CPU/thread/clock shutdown and before freeing
kernel memory. The native pthread timer adapter depends on this new point to
drain callbacks. Timer draining is conventional in concept, but this ABI slot
and shutdown placement were locally designed kernel behavior, not an existing
parameter, configuration choice or established patch. Native library builds,
35 RAM boot controls and later dependent diagnostics completed with that code.
Local success does not make the kernel change eligible.

Read-only comparison covered all 63,013 original regular files from LKL
`86dd3afb590eccc1903611bdaa8bac87757eb80d`. Twelve paths differed; the recorded
seven official patches plus ten local patches exactly reconstruct those files.
The official patches are byte-identical between DPorts
`91731d0e53e99b4ed0ab75677c9325d741a85123` and FreeBSD Ports
`46cf3d9696491c61d91ecd22db11c7f7c1e62c24`. The kernel ABI and shutdown additions
are absent from that official baseline. No other original kernel namespace or
filesystem implementation file changed. UFS remained disabled.

Host ELF-header constants, build/link flags, native timer/I/O adapters, FUSE
wire compatibility and client/guardian logic are classified separately by
technical effect. Their location or local authorship alone does not make them
kernel violations. However, the current `liblkl.a` embeds the novel kernel hook,
so its dependent executables cannot qualify as eligible host drivers. Existing
ordinary LUKS1/VeraCrypt qualification does not depend on this prototype.

## Cut state and progress

The fresh native process and mount inventory found no running experimental
compiler, client, test or prototype FUSE mount. No stop signal was required.
This owner spawned no descendant agents. Previously completed experiments and
their failures remain recorded; no source, binary, fixture or evidence was
deleted or reverted. The identified work/cache partition remains mounted to
preserve the retained work. The original encrypted final scratch is separate.

The new foreground client did not pass mounted qualification. The reviewed024
attempt failed before readiness/publication; subsequent025/027/028 clients were
not privileged-mounted. Observer query/thread controls and callback/halt
regressions completed, but actual owned FUSE present/unmounting/absent controls
and full caller/payload matrices did not. They are unproven, not support credit.
CryFS's root-only control and blocked caller/product integration remain separate.

The following work is removed from actionable plans: new or restarted builds
of this modified kernel/library; dependent client controls and mounts;
Btrfs/XFS product, frontend and device admission through this prototype;
UFS configuration/formatter validation dependent on this prototype; and any new
kernel workaround, ABI replacement or upstream-interface project. Earlier wording describing this
work as pending, deferred or awaiting review is superseded. There is no
kernel-dependent continuation queue for this prototype. Independent native
userspace work may continue without executing the cut embedded kernel.

Private retained evidence is under
`target/goal-validation/native-fresh-20261127/hosts/dragonflybsd/`:

- `inventory/lkl-source-assessment/kernel-boundary/audit.json`: source domains,
  original/official/current hashes, patch provenance and historical dependencies.
- `build/lkl/native-kernel-boundary-source-audit.json`: full native source scan.
- `inventory/lkl-source-assessment/kernel-boundary/source-reconstruction.json`:
  exact reconstruction of all twelve changed source files.
- `build/lkl/native-kernel-boundary-held-artifacts.json`: retained executable,
  library and source identities; the current library hash is
  `34e5ffd3346cd84ceb422483433ca94d7b3f44985cd711c2cf4599ea2d0975d9`.
- `build/lkl/native-kernel-scope-cut.json`: fresh process, mount and boot inventory.
- `inventory/lkl-source-assessment/kernel-boundary/cut-state.json`: supersedes
  historical actionable dependencies while preserving the original records.
