# FREEBSD implementation and review handoff

Read README.md in this directory first; it contains binding scope, evidence, exclusions and completion gates.

## Host ownership

freebsd (adjutant@freebsd-core-01). Use scripts/remote-test/hosts.json for exact SSH and toolchain settings. Exclusive ownership starts when implementation agent is spawned and ends only after its processes/jobs are terminal; then the reviewer owns it.

## Encryption routes

GELI, VeraCrypt; directory PEFS and gocryptfs. Inspect current source/platform allowlist and installed artifacts to confirm. Do not add selected-but-unimplemented backends.

## Filesystem route plan

Native UFS1/UFS2 and OpenZFS, package NTFS-3G and exFAT+FUSE, compatible ext4 via ext2fs or fuse-ext2; prefer current native driver. XFS tools + available LKL; Btrfs old LKL only if compatible. APFS RO via libfsapfs. HAMMER2 matching-kernel upstream module can be built without edits. No LKL modernization or new kernel.

## Code ownership

Own src/bsd.rs shared filesystem glue and src/bsd/filesystems.rs in addition to src/bsd/freebsd.rs and OS-specific files. Coordinate small dispatch contract immediately with other BSD owners. Maintain existing other BSD defaults while exposing selected format/mount dispatch.

## Execution and acceptance

Preflight and install minimal dependencies; implement needed CLI/core adapters; use root common enum/CLI and payload harness; remote build/check; create one disposable file vault per feasible method/filesystem, write PDF/text, close/reopen/hash, append/add text, close/reopen/hash, normal cleanup. Use native tools to create imports for RO-only routes when possible, and label inability to create/write explicitly. Record all ten filesystem cells even if unavailable. Do not run SQLite, GUI, fault tests or physical-device tests. No unnecessary cipher/credential matrix.

Use /tmp/lurker-filesystem-round-<os>-<run> remotely, existing per-user caches for builds, and target/goal-validation/filesystem-round/freebsd/ locally for receipts. Put durable concise outcome in DIRECTIVES/PLAN/FILESYSTEM_ROUND/FREEBSD_RESULTS.md. Record source/binary identities, install commands/versions, counts, per-pair statuses, known feature limits, cleanup and reviewer next action. Only claim observed native results.

## Checkpoint

Implementation complete; host released for fresh review. Product `1a37591`, no live job handles. Read `FREEBSD_RESULTS.md` and `target/goal-validation/filesystem-round/freebsd/summary.json` for receipts, limits and cleanup. Reviewer records review commits and outcomes after implementation closes.
