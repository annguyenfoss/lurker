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

## Resolved source-scope findings

The coordinator's read-only review under final policy `10fbc99` resolves the
following changes as eligible integration, without creating new runtime passes:

- **Linux FS5:** the five local defconfig selections enable existing upstream
  options: FILE_LOCKING, EXFAT_FS, NTFS3_FS, NTFS3_LZX_XPRESS and
  NTFS3_FS_POSIX_ACL. The `generic-y += ring_buffer.h` addition selects the
  unmodified upstream generic header through the documented Kbuild mechanism.
  The pinned official 6.12.118 stable patch supplies that same header and makes
  the identical selection for twenty other architectures. This is established
  configuration/build integration, not an invented kernel implementation.
  Host virtual-block/FUSE error handling is separate userspace integration.
- **FreeBSD LKL:** the seven official Ports patches are established inputs.
  The only semantic local header-generator change explicitly selects Python's
  `fork` pool context, following CPython's migration guidance after the 3.14
  default changed. It changes the build process, not generated kernel ABI or
  kernel behavior. Local virtual-block FLUSH/error handling, native barriers
  and FUSE-client lifecycle use existing kernel interfaces; they add no local
  kernel host-operations ABI or shutdown hook. Their userspace integration is
  in scope; reliable completion and product qualification remain open.
- **macOS symbol visibility:** the local change uses Apple's documented linker
  export-list procedure, retaining existing LKL APIs and boot references. It
  does not invent kernel implementation or interfaces. The third-party Darwin
  port was pre-existing; its maturity and distribution suitability remain a
  separate assessment, not proof of Lurker-invented code.

These findings supersede earlier categorical conclusions based solely on local
configuration changes, build-script location or a kernel-object link target.
Original audits, source identities and functional receipts remain frozen. No
held job automatically restarts, product guard is removed, or cut BSD timer-hook
work becomes eligible through this review.

Primary guidance and source:

- https://docs.kernel.org/6.12/kbuild/makefiles.html#generic-y
- https://raw.githubusercontent.com/torvalds/linux/v6.12/fs/Kconfig
- https://raw.githubusercontent.com/torvalds/linux/v6.12/fs/exfat/Kconfig
- https://raw.githubusercontent.com/torvalds/linux/v6.12/fs/ntfs3/Kconfig
- https://cdn.kernel.org/pub/linux/kernel/v6.x/patch-6.12.118.xz
- https://docs.python.org/3.14/whatsnew/3.14.html#multiprocessing
- https://developer.apple.com/library/archive/documentation/Performance/Conceptual/CodeFootprint/Articles/ReducingExports.html
- https://github.com/apple-oss-distributions/ld64/blob/main/doc/man/man1/ld-classic.1

Private source captures, hashes, patch inventories and read-only comparisons are
in `audits/established-integration-10fbc99/root-scope-reassessment.json` under the
round's evidence root. This is scope evidence, not a complete product audit.

## Remaining prerequisite and product assessment

| Area | Existing work and remaining question |
| --- | --- |
| Linux unprivileged FS5, all six distributions | Source-scope question resolved above. Preserve exact six-host functional evidence; final package/requirement reconciliation remains separate. Fedora's ordinary unsigned-RPM installation remains blocked by signature policy. |
| macOS LKL | Local linker/host integration is in scope. Establish vendor-port maturity/maintenance and ordinary installation suitability; complete live completion, ownership, crypto-view and applicable device qualification before product admission. |
| FreeBSD LKL | Source integration is in scope. Complete userspace guardian/session integration, exact lifecycle/geometry proofs and installed product qualification before lifting existing guards. |
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
