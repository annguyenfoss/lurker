# DRAGONFLY implementation and review handoff

Read README.md in this directory first; it contains binding scope, evidence, exclusions and completion gates.

## Host ownership

dragonflybsd (adjutant@dragonflybsd-core-01). Use scripts/remote-test/hosts.json for exact SSH and toolchain settings. Exclusive ownership starts when implementation agent is spawned and ends only after its processes/jobs are terminal; then the reviewer owns it.

## Encryption routes

LUKS1 and VeraCrypt; no directory backends implemented. Inspect current source/platform allowlist and installed artifacts to confirm. Do not add selected-but-unimplemented backends.

## Filesystem route plan

Native UFS1 and HAMMER2. Existing FAT32 stays a regression only, outside ten-filesystem table. Install NTFS/exFAT formatters and inspect available FUSE runtime. Stock 6.4.2 missing FUSE: do not patch/build replacement kernel, record mount prerequisite unavailable unless a normal compatible upstream binary/module exists. ext4 native RO subset; XFS/APFS readers need working FUSE. No UFS2/Btrfs/ZFS mount route expected. Upstream HAMMER2 durability findings are out of this payload-only round; retain limitation without trying fixes.

## Code ownership

Own src/bsd/dragonfly.rs and src/bsd/dragonfly/* plus OS-specific scripts. Existing VeraCrypt probes allow HAMMER2/FAT32 only: extend only for routes actually available. Coordinate bsd.rs with FreeBSD owner; preserve wrapper crypto profile.

## Execution and acceptance

Preflight and install minimal dependencies; implement needed CLI/core adapters; use root common enum/CLI and payload harness; remote build/check; create one disposable file vault per feasible method/filesystem, write PDF/text, close/reopen/hash, append/add text, close/reopen/hash, normal cleanup. Use native tools to create imports for RO-only routes when possible, and label inability to create/write explicitly. Record all ten filesystem cells even if unavailable. Do not run SQLite, GUI, fault tests or physical-device tests. No unnecessary cipher/credential matrix.

Use /tmp/lurker-filesystem-round-<os>-<run> remotely, existing per-user caches for builds, and target/goal-validation/filesystem-round/dragonfly/ locally for receipts. Put durable concise outcome in DIRECTIVES/PLAN/FILESYSTEM_ROUND/DRAGONFLY_RESULTS.md. Record source/binary identities, install commands/versions, counts, per-pair statuses, known feature limits, cleanup and reviewer next action. Only claim observed native results.

## Checkpoint

Not started. Owner updates result note with exact commits, active job handle, receipts and next action; do not paste transcripts. Reviewer records review commits and outcomes after implementation closes.
