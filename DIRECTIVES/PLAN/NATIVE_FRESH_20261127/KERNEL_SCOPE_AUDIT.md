# Kernel scope audit

Updated 2026-11-27 after all eleven host owners acknowledged the clarified rule.
The authoritative policy is `DIRECTIVES/LLM/DEVELOPMENT.md`, Kernel integration
boundary. Existing upstream/mainstream patches and established documented
procedures may be applied for Lurker. Inventing and validating novel kernel
behavior, ABI hooks or experimental ports is outside scope. Local authorship,
file location and successful local tests do not settle the classification alone.

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

- All eleven host owners received and acknowledged the scope and handling rule.
- No out-of-scope kernel build or test is currently reported running. NetBSD's
  rebuild had already been cancelled before the user instructed agents not to
  interrupt existing work for a scope finding; it has not been restarted.
- The active FreeBSD Cryptomator build/provider checks are native userspace
  Java/FFM integration and perform no kernel changes, kernel builds or LKL
  execution. Standalone userspace
  guardian, observer, evidence and provenance work is tracked separately.
- Already-running work must not be interrupted solely because an audit flags
  its scope. Report its changes, method and progress. Start no new out-of-scope
  work and do not automatically restart previously held/cancelled jobs.
- Preserve source, partial artifacts and failures; do not destructively revert,
  delete or relabel them. Stock native workflows retain their original scope.

Detailed private audits are retained under the round's host directories:
NetBSD `build/lkl-native/new-scope-kernel-audit.*`, OpenBSD
`build/lkl-native/new-kernel-scope-audit/`, DragonFly
`build/lkl/native-kernel-boundary-source-audit.json`, Linux
`hosts/archlinux/build/unprivileged-pilot/fs5-patch-domain-audit/`, macOS
`build/darwin-kernel-scope-audit/`, and FreeBSD
`validation/kernel-domain-audit/`.
