# Kernel scope audit

Updated 2026-11-27; the newest stop-and-cut instruction supersedes the previous
non-interruption policy. All eleven owners returned fresh scope/job audits.
The authoritative policy is `DIRECTIVES/LLM/DEVELOPMENT.md`, Kernel integration
boundary. Existing upstream/mainstream patches and established documented
procedures may be applied for Lurker. Inventing and validating novel kernel
behavior, ABI hooks or experimental ports is outside scope. Local authorship,
file location and successful local tests do not settle the classification alone.
"Novel" means Lurker's own invented kernel code or behavior. Applying established
patches does not count as novel code; existing kernel configuration and parameter
changes, including before a kernel build, are generally allowed.
Customized kernel builds are allowed with reproducible existing configuration
selections, established patches and upstream-associated guidelines. Competent
users should be able to inspect those settings and reproduce the build without
reviewing or adopting kernel code invented by Lurker.

## Confirmed experimental kernel work already performed

| Host | What and how | Progress and current state |
| --- | --- | --- |
| NetBSD | Added a `timer_quiesce` slot to LKL's kernel host-operations ABI and a new shutdown hook. No established upstream patch provenance was demonstrated. | Native library, 35 RAM boots and credential/transport experiments completed using that hook. A subsequent locally authored syscall-bounds fix reached a kernel rebuild, which was cancelled before the user's later non-interruption instruction. No new bounds-fixed library or test completed; no mount or product admission. |
| OpenBSD | Added the same new kernel ABI slot and shutdown hook to support the native port. | Native libraries, RAM boots and dependent diagnostics completed. Kernel-dependent client WIP remains preserved; no mounted driver or product admission. No kernel job is running. |
| DragonFly BSD | Added the same new kernel ABI slot and shutdown hook; native timer glue depends on it. | Native library and dependent diagnostics completed. New foreground-client mounted qualification has not passed. No kernel job is running. |

These observations remain experimental evidence, not eligible support claims.
An established alternative is required before dependent routes can be admitted;
removing or relocating a hook without an established safe replacement is not a
validated solution.

### Degree of novelty and packaging assessment

The three ports adapt the same lifecycle design; they are not three unrelated
kernel inventions. Each adds a function pointer to LKL's host-operations ABI
and calls it during kernel shutdown, after thread/clock shutdown and before
freeing kernel memory. Native timer code waits for outstanding callbacks at
that new point. Waiting for callbacks is a conventional synchronization idea;
the new ABI and its placement in this kernel are locally designed behavior,
not an existing configuration choice. They require matching kernel/host builds
and carry deadlock, shutdown-hang and callback-after-free concerns that local
boot tests alone do not settle. No filesystem algorithm or host BSD boot-kernel
change is involved, but the embedded kernel dependency remains substantive.

NetBSD additionally has a small, conventional bounds-check ordering repair in
preserved WIP: validate the syscall number before indexing its handler table.
It is less novel than the shutdown hook, but it is still a locally authored
kernel repair; its rebuild was cancelled and no repaired binary was qualified.

Under the user's mainstream-repository benchmark, none of these dependent
routes is ready for distribution on the strength of these local patches.
Existing supported settings and established patches are acceptable; maintaining
our own kernel interface or pursuing its upstream acceptance is outside this
project. Keep independent userspace work and native supported routes separate.

## Provenance or effect still requiring assessment

| Area | Existing work and remaining question |
| --- | --- |
| Linux unprivileged FS5, all six distributions | The reused LKL kernel includes local configuration selections and an `asm/Kbuild` generic-header addition. Host transport/FUSE safety changes are separate. The builds and affected runtime checks completed, but the retained evidence does not yet establish the relevant upstream guidance for every delta. Configuration/build edits are not automatically novel kernel development. |
| macOS LKL | Reuses a pre-existing public Darwin port. The local symbol-visibility change is in its Python linker build script, not kernel C or syscall ABI. Official Apple export-list guidance has been retained for review. Judge that established linker procedure and the vendor port's maturity on evidence; do not classify either solely by filename or local application. |
| FreeBSD LKL | Uses seven recorded official Ports patches plus local host transport/client changes and a Python header-generator fork-context adjustment. No locally changed filesystem/core-kernel C was found. The toolchain/build procedure and host transport changes still need individual classification. |
| Conditional Linux UFS1 configurations | Existing write-enabled modules/configurations were reused on some hosts. Exact established configuration provenance remains relevant; no new Arch/Ubuntu UFS1 kernel work has started. |
| Deferred NetBSD ZFS and OpenBSD HAMMER2 work | An existing upstream fix or established current-OS port may be used. Inventing a host-ID kernel repair or a new kernel-port adaptation is not authorized. These deferred kernel tasks have not started. |

Functional receipts and original indexes remain unchanged. Neither an unresolved
provenance question nor passing our own tests should be rewritten as an
established scope decision.

## Current execution and handling

- All eleven host owners received the exact definition and returned fresh
  completed/current/planned-work audits under the stricter stop-and-cut rule.
- No out-of-scope kernel build or test is currently reported running. NetBSD's
  rebuild had already been cancelled before the user instructed agents not to
  interrupt existing work for a scope finding; it has not been restarted.
- The active FreeBSD Cryptomator build/provider checks are native userspace
  Java/FFM integration and perform no kernel changes, kernel builds or LKL
  execution. Standalone userspace
  guardian, observer, evidence and provenance work is tracked separately.
- Cut partial, in-progress and planned work that invents kernel code or behavior
  for Lurker. Stop any such running jobs through their owner and verify terminal
  state; remove tasks from actionable plans. The user's latest instruction
  explicitly replaces the earlier non-interruption rule. Established patches,
  supported configuration and independent userspace work remain in scope.
- Preserve source, partial artifacts and failures; do not destructively revert,
  delete or relabel them. Stock native workflows retain their original scope.

## Stop-and-cut owner audit

All eleven owners were sent the exact definition and the replacement handling
rule in `2d3c82c`. Their fresh completed/current/planned-work audits report:

| Owner | Verified execution state and scope disposition |
| --- | --- |
| Arch Linux | No owned jobs across all UIDs, mounts or descendants. No novel kernel implementation found in its FS5 delta. Cut prospective kernel/interface/port experiments, including novel UFS1 work. Common configuration/build review remains separate. |
| Debian | No kernel jobs or descendants. Completed Linux donor-dependent observations preserved. Cut novel-kernel-dependent follow-ups; current FreeBSD Cryptomator Java/FFM/source packaging is independent userspace work. |
| Ubuntu | No owned build/test/runtime jobs, mounts, GUI binds or descendants. No novel UFS1 code started. Cut all prospective novel kernel work; no automatic restart. |
| Fedora | No owned jobs, mounts, sessions or descendants. Donor FS5 observations and existing modules preserved. Cut prospective novel kernel/ABI experiments; no rebuild or reboot needed. |
| openSUSE | No owned build/qualification jobs or descendants. All e927 work terminal. No novel-kernel continuation remains planned. |
| Gentoo | No owned jobs, e927 mounts, descendants or queued work. Existing UFS1 configuration distinguished from novel code. Cut prospective new kernel behavior/ABI/lifecycle/port work. |
| macOS | No active LKL/FUSE/guardian/formatter jobs or descendants. No Lurker-invented kernel implementation found in nineteen local patches. Cut hypothetical new kernel hooks/ABI/halt logic, Darwin-port invention and NetBSD bounds-fix import. Established vendor-patch maturity remains a separate question. |
| FreeBSD | No active kernel/LKL build or storage-test job; no descendants. Cut the unstarted plan to invent FUSE RENAME2 kernel support. Seven official Ports patches remain eligible established inputs. Local host-transport classification is separate. |
| NetBSD | Cancelled build remains terminal `-15`; no owned build/runtime jobs or descendants. Cut custom ABI/shutdown, bounds-repair and proposed novel lifecycle/page-free continuation. Preserve all experimental code and results. |
| OpenBSD | No matching root/user coordinators, compilers, controls or descendants. Cut invented ABI/shutdown-dependent development and its client qualification. Independent completed observer remains separate. |
| DragonFly BSD | No experimental compilers, clients, tests, mounts or descendants. Cut the locally invented ABI/shutdown-hook route and dependent qualification. Preserve source, binaries and failure receipts. |

No owner reported an active novel-kernel job requiring a new termination signal.
This does not claim that every host is free of blocked processes: FreeBSD's old
failed-FUSE cleanup/stat waits and the unrelated Cryptomator global-stat wait
remain nonterminal. They are retained operational failures, not running kernel
development, and must not be relabeled as successful cleanup.

Detailed private audits are retained under the round's host directories:
NetBSD `build/lkl-native/new-scope-kernel-audit.*`, OpenBSD
`build/lkl-native/new-kernel-scope-audit/`, DragonFly
`build/lkl/native-kernel-boundary-source-audit.json`, Linux
`hosts/archlinux/build/unprivileged-pilot/fs5-patch-domain-audit/`, macOS
`build/darwin-kernel-scope-audit/`, and FreeBSD
`validation/kernel-domain-audit/`.
