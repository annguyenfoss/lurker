# MACOS implementation and review handoff

Read README.md in this directory first; it contains binding scope, evidence, exclusions and completion gates.

## Host ownership

macos (adjutant@buran; fallback buran.local/buran.swarm). Use scripts/remote-test/hosts.json for exact SSH and toolchain settings. Exclusive ownership starts when implementation agent is spawned and ends only after its processes/jobs are terminal; then the reviewer owns it.

## Encryption routes

Apple encrypted image, native APFS encrypted file, VeraCrypt; directory gocryptfs, CryFS, Cryptomator. Inspect current source/platform allowlist and installed artifacts to confirm. Do not add selected-but-unimplemented backends.

## Filesystem route plan

Native APFS/exFAT first. Prefer signed upstream FSKit releases for ext4/Btrfs/XFS/NTFS; standalone formatter may complement one mount driver. Existing FUSE-T encryption runtime stays unchanged. OpenZFS only a compatible installer that does not require reboot/security changes; otherwise record blocked. Legacy UFS candidates get bounded investigation; no driver port, new entitlement work or kernel extension security reduction. HAMMER2 no known route: record unavailable. Preserve Buran existing sessions; never reboot/shutdown.

## Code ownership

crates/lurker-core/src/macos.rs and src/macos/*, macOS-specific scripts/tests. Native APFS encryption cannot contain another inner filesystem: record inapplicable, not failed. Apple image encryption may wrap alternate supported filesystems via existing hdiutil attach/format workflows.

## Execution and acceptance

Preflight and install minimal dependencies; implement needed CLI/core adapters; use root common enum/CLI and payload harness; remote build/check; create one disposable file vault per feasible method/filesystem, write PDF/text, close/reopen/hash, append/add text, close/reopen/hash, normal cleanup. The target is Create + Read/Write through one mount/unmount workflow. Access restrictions follow actual filesystem/driver capability, not a selectable mode. Record inability to create or write as incomplete support; do not add separate read-only-mode implementation or qualification. Record all ten filesystem cells even if unavailable. Do not run SQLite, GUI, fault tests or physical-device tests. No unnecessary cipher/credential matrix.

Use /tmp/lurker-filesystem-round-<os>-<run> remotely, existing per-user caches for builds, and target/goal-validation/filesystem-round/macos/ locally for receipts. Put durable concise outcome in DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RESULTS.md. Record source/binary identities, install commands/versions, counts, per-pair statuses, known feature limits, cleanup and reviewer next action. Only claim observed native results.

## Checkpoint

Implementation terminal; host released for review on 2026-11-25. Read `MACOS_RESULTS.md` and `target/goal-validation/filesystem-round/macos/result.json` first. Native candidate `a9aeaa6` passed 63 tests and focused exFAT publication regressions; all owned jobs, mounts and sessions are closed. Reviewer records review commits and outcomes after inspecting the recorded native passes and prerequisite limitations.
