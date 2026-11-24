# NETBSD implementation and review handoff

Read README.md in this directory first; it contains binding scope, evidence, exclusions and completion gates.

## Host ownership

netbsd (adjutant@netbsd-core-01). Use scripts/remote-test/hosts.json for exact SSH and toolchain settings. Exclusive ownership starts when implementation agent is spawned and ends only after its processes/jobs are terminal; then the reviewer owns it.

## Encryption routes

CGD only; no planned NetBSD VeraCrypt/directory additions. Inspect current source/platform allowlist and installed artifacts to confirm. Do not add selected-but-unimplemented backends.

## Filesystem route plan

Native FFSv1/FFSv2; package exFAT, NTFS-3G, e2fsprogs/fuse-ext2. ext4 writes experimental: record observed limits. Base ZFS on supported architecture with bounded ordinary config. HAMMER2 upstream module may be built with upstream HAMMER2_WRITE config against matching installed headers; no source edits/kernel patches. Btrfs/XFS/APFS no established route, record unavailable. Keep CGD sidecar and WAPBL requirement for FFS only, not unrelated filesystems.

## Code ownership

Own src/bsd/netbsd.rs and new src/bsd/netbsd/* files plus OS-specific scripts. Shared dispatch via FreeBSD owner; report native raw-device naming and formatter expectations.

## Execution and acceptance

Preflight and install minimal dependencies; implement needed CLI/core adapters; use root common enum/CLI and payload harness; remote build/check; create one disposable file vault per feasible method/filesystem, write PDF/text, close/reopen/hash, append/add text, close/reopen/hash, normal cleanup. Use native tools to create imports for RO-only routes when possible, and label inability to create/write explicitly. Record all ten filesystem cells even if unavailable. Do not run SQLite, GUI, fault tests or physical-device tests. No unnecessary cipher/credential matrix.

Use /tmp/lurker-filesystem-round-<os>-<run> remotely, existing per-user caches for builds, and target/goal-validation/filesystem-round/netbsd/ locally for receipts. Put durable concise outcome in DIRECTIVES/PLAN/FILESYSTEM_ROUND/NETBSD_RESULTS.md. Record source/binary identities, install commands/versions, counts, per-pair statuses, known feature limits, cleanup and reviewer next action. Only claim observed native results.

## Checkpoint

Implementation complete. Exact source `58c97b0188ee2d604a094acb9ffe0e10ce84de23`: seven CGD/filesystem payload pairs passed (49 CLI operations), native core/CLI/helper checks passed (69 tests, 4 ignored). All jobs terminal; native mounts/devices/pools/session registry clean. Read `NETBSD_RESULTS.md` and `target/goal-validation/filesystem-round/netbsd/summary.json` before review. Reviewer records review commits and outcomes after implementation closes.
