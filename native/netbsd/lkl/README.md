# Archived NetBSD LKL investigation — cut

Status: the experimental route is cut from actionable plans under the user's
non-invasive, mainstream-repository boundary. No build, test, mounted driver or
Lurker integration is scheduled for this route. Source, uncommitted WIP, partial
objects, failures and original recipes remain preserved. This document records
their disposition rather than instructions to continue them.

Novel means kernel code or behavior invented by Lurker. Applying established
pre-made kernel-source patches remains allowed, including patches applied
specifically for Lurker. Existing supported parameters and configuration,
including selections before a build, are allowed. Reproducible customized kernel
builds using those selections and established patches/guidance are also allowed;
their configuration differences, provenance and build steps must be recorded.
There is no blanket patch, customized-build or userspace ban. The boundary
applies to the kernel embedded in LKL as well as a
host boot kernel. In particular, `MMU=n` matches upstream LKL's default and
KASAN is an existing option; the locally invented ABI and lifecycle behavior
below are separate from those choices.

## Completed work and its limits

The investigation used pinned upstream
`d0f76a77e8c2ca68bd7d96190b8da09040eab072`, archive SHA-256
`ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168`, on the
native NetBSD host as UID 1300. Source/cache verification covered 86,818 original
files. Compiler, SDK, generated-header, source and artifact identities are retained.

The first unmodified-kernel RAM-only boot attempt stalled at startup. Native
timer controls and the installed `sigevent(3)` manual identified unsupported
`SIGEV_THREAD` delivery. A local pthread timer adaptation subsequently added a
`timer_quiesce` field to `arch/lkl/include/uapi/asm/host_ops.h` and a call in
`arch/lkl/kernel/setup.c` after CPU/thread/clock shutdown and before memory
release. These are locally designed embedded-kernel ABI/shutdown changes, not
established pre-made upstream changes.

That modified kernel passed all 35 cases in the upstream RAM-only boot suite,
followed by credential, set-ID, error-translation and native file-image transport
experiments. One-shot write/flush failures demonstrated why a later successful
sync, unmount or halt must not erase an earlier I/O error. These are preserved
experimental observations, not eligible NetBSD filesystem support claims. No
native FUSE filesystem was mounted and no driver or product/model route was
installed or enabled.

Independent userspace work covered native SDK/mmap compatibility, caller
snapshots, held credentials, sticky block errors, descriptor-release gates,
global mount-identity comparison, an isolated ReFUSE library with a distinct
SONAME, and a bounded read-only inventory worker. The last completed mixed
suite passed 23 tests. Its kernel-dependent cases retain the scope limitation
above; standalone userspace results do not qualify the unfinished driver.
The ReFUSE bridge and observer never replaced installed system providers.

## Cancelled and cut items

| Item | Progress and final disposition |
| --- | --- |
| Custom `timer_quiesce` ABI and shutdown integration | Compiled and exercised historically. Further development, builds, kernel tests and dependent driver admission are cut. |
| Local syscall-bounds repair in `arch/lkl/kernel/syscalls.c` | A RAM-only invalid-syscall control exposed a real table lookup before its bounds check. The crash/core/debugger evidence and proposed repair are preserved. Its rebuild was cancelled; no repaired library or boot executable completed or ran. |
| New negative-syscall controls and producer changes | Preserved as uncommitted WIP. Required verification was cancelled; these are not ready or planned for execution. |
| Proposed replacement drain through `page_free` or another lifecycle point | Source-only inference; never implemented or tested. Removed from actionable plans. Relocating the design is not an approved continuation. |
| Native mounted LKL client, further experimental lifecycle work and dependent Lurker enablement | Not completed. The kernel-dependent continuation is cut; no mount or support claim follows from compilation or RAM tests. |
| New upstream-acceptance project for these changes | Not planned. Publishing local experimental code would not establish mature upstream provenance. |

Supervisor PID 18864 and its owned make process group 15917 were verified by
UID, command and ancestry. The supervisor was stopped while TERM and then
unconditional group KILL were sent to the reserved child group; it resumed to
record terminal return code `-15`. That cancellation preceded the earlier
non-interruption instruction and was never restarted. The latest explicit cut
instruction was then checked against all owned jobs and descendants: zero
matching builds, compilers or runtime jobs remained. No subagents were spawned.

Partial `syscalls.o` and `setup.o` are preserved. No new `liblkl.a`, `lkl.o` or
boot output completed. Experimental source and WIP have not been deleted,
reverted or executed to close this audit. The mixed suite and adapted/boot
builder commands remain historical recipes, not an actionable validation plan.

## Preserved evidence and host state

Private receipts are under
`target/goal-validation/native-fresh-20261127/hosts/netbsd/build/lkl-native/`:

- `new-scope-kernel-audit.json` and `latest-scope-source-addendum.json`: exact
  file/provenance classifications and historical source assessment.
- `bounds-build-cancel.json` and `bounds-boot-build.json`: cancellation, process
  identities, terminal status and partial-build inputs.
- `final-experimental-cut-verification.json` and
  `final-experimental-cut-status.json`: latest zero-job check and cut plan.
- `errno-invalid-syscall-failure.json` and `errno-invalid-syscall-gdb.txt`:
  original crash/core identity and failing source location.

The identified work/cache qcow2 and its FFS mount remain solely for preserved
source, cache and evidence. Original scratch disks and failed fixtures remain
untouched. User-mount policy is zero; `/dev/putter` remains root:wheel 0600,
rdev 45568. All three installed native provider hashes are unchanged. The
separately qualified installed `8e09ac8` CGD/CryFS package and its receipts retain
their own artifact identities and are not relabelled as LKL results.

The applicable project boundary is recorded in
`DIRECTIVES/LLM/DEVELOPMENT.md` and
`DIRECTIVES/PLAN/NATIVE_FRESH_20261127/KERNEL_SCOPE_AUDIT.md`.

https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
https://man.netbsd.org/sigevent.3
