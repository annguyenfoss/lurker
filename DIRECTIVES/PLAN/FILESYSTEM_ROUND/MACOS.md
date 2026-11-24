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

Preflight and install minimal dependencies; implement needed CLI/core adapters; use root common enum/CLI and payload harness; remote build/check; create one disposable file vault per feasible method/filesystem, write PDF/text, close/reopen/hash, append/add text, close/reopen/hash, normal cleanup. Use native tools to create imports for RO-only routes when possible, and label inability to create/write explicitly. Record all ten filesystem cells even if unavailable. Do not run SQLite, GUI, fault tests or physical-device tests. No unnecessary cipher/credential matrix.

Use /tmp/lurker-filesystem-round-<os>-<run> remotely, existing per-user caches for builds, and target/goal-validation/filesystem-round/macos/ locally for receipts. Put durable concise outcome in DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RESULTS.md. Record source/binary identities, install commands/versions, counts, per-pair statuses, known feature limits, cleanup and reviewer next action. Only claim observed native results.

## Checkpoint

Not started. Owner updates result note with exact commits, active job handle, receipts and next action; do not paste transcripts. Reviewer records review commits and outcomes after implementation closes.
