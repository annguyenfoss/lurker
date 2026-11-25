# OPENBSD implementation and review handoff

Read README.md in this directory first; it contains binding scope, evidence, exclusions and completion gates.

## Host ownership

openbsd (adjutant@openbsd-core-01). Use scripts/remote-test/hosts.json for exact SSH and toolchain settings. Exclusive ownership starts when implementation agent is spawned and ends only after its processes/jobs are terminal; then the reviewer owns it.

## Encryption routes

softraid CRYPTO, RAID1C, VeraCrypt. Inspect current source/platform allowlist and installed artifacts to confirm. Do not add selected-but-unimplemented backends.

## Filesystem route plan

Native FFSv1/FFSv2; package exfat-fuse and ntfs-3g. ext4 RO subset via ext2fs; XFS/APFS RO via libyal FUSE tools. HAMMER2 upstream requires patched kernel and is out of permitted setup: record unavailable under this round, no patching. Btrfs/ZFS no existing eligible native route. Existing label geometry for each encryption method must be preserved.

## Code ownership

Own src/bsd/openbsd.rs, src/bsd/raid1c*.rs as needed and new src/bsd/openbsd/* files; coordinate bsd.rs glue with FreeBSD owner. Test both RAID1C file members through ordinary lifecycle; no degraded/fault suite.

## Execution and acceptance

Preflight and install minimal dependencies; implement needed CLI/core adapters; use root common enum/CLI and payload harness; remote build/check; create one disposable file vault per feasible method/filesystem, write PDF/text, close/reopen/hash, append/add text, close/reopen/hash, normal cleanup. The target is Create + Read/Write through one mount/unmount workflow. Access restrictions follow actual filesystem/driver capability, not a selectable mode. Record inability to create or write as incomplete support; do not add separate read-only-mode implementation or qualification. Record all ten filesystem cells even if unavailable. Do not run SQLite, GUI, fault tests or physical-device tests. No unnecessary cipher/credential matrix.

Use /tmp/lurker-filesystem-round-<os>-<run> remotely, existing per-user caches for builds, and target/goal-validation/filesystem-round/openbsd/ locally for receipts. Put durable concise outcome in DIRECTIVES/PLAN/FILESYSTEM_ROUND/OPENBSD_RESULTS.md. Record source/binary identities, install commands/versions, counts, per-pair statuses, known feature limits, cleanup and reviewer next action. Only claim observed native results.

## Checkpoint

Not started. Owner updates result note with exact commits, active job handle, receipts and next action; do not paste transcripts. Reviewer records review commits and outcomes after implementation closes.
