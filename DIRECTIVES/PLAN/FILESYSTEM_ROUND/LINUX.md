# LINUX implementation and review handoff

Read README.md in this directory first; it contains binding scope, evidence, exclusions and completion gates.

## Host ownership

archlinux, debian, ubuntu, fedora, opensuse, gentoo. Use scripts/remote-test/hosts.json for exact SSH and toolchain settings. Exclusive ownership starts when implementation agent is spawned and ends only after its processes/jobs are terminal; then the reviewer owns it.

## Encryption routes

LUKS2, LUKS1, privileged VeraCrypt, login-user VeraCrypt; directory gocryptfs, CryFS, Cryptomator. Inspect current source/platform allowlist and installed artifacts to confirm. Do not add selected-but-unimplemented backends.

## Filesystem route plan

Native ext4/e2fsprogs, Btrfs/btrfs-progs, XFS/xfsprogs, exFAT/exfatprogs; NTFS kernel/NTFS-3G+mkntfs; OpenZFS package where compatible. UFS via installed kernel module with correct variant and makefs; do not rebuild kernel to enable writes. APFS via available unmodified driver/apfsprogs; HAMMER2 matching-kernel upstream module only if buildable without edits. Login-user route is limited to filesystems compiled into existing LKL; do not extend/patch LKL in this round.

## Code ownership

crates/lurker-core/src/linux_workflow.rs, userspace.rs, system.rs; Linux-specific new modules/tests/scripts. All six distros need independent native receipts, not an Arch-only extrapolation.

## Execution and acceptance

Preflight and install minimal dependencies; implement needed CLI/core adapters; use root common enum/CLI and payload harness; remote build/check; create one disposable file vault per feasible method/filesystem, write PDF/text, close/reopen/hash, append/add text, close/reopen/hash, normal cleanup. The target is Create + Read/Write through one mount/unmount workflow. Access restrictions follow actual filesystem/driver capability, not a selectable mode. Record inability to create or write as incomplete support; do not add separate read-only-mode implementation or qualification. Record all ten filesystem cells even if unavailable. Do not run SQLite, GUI, fault tests or physical-device tests. No unnecessary cipher/credential matrix.

Use /tmp/lurker-filesystem-round-<os>-<run> remotely, existing per-user caches for builds, and target/goal-validation/filesystem-round/linux/ locally for receipts. Put durable concise outcome in DIRECTIVES/PLAN/FILESYSTEM_ROUND/LINUX_RESULTS.md. Record source/binary identities, install commands/versions, counts, per-pair statuses, known feature limits, cleanup and reviewer next action. Only claim observed native results.

## Checkpoint

Not started. Owner updates result note with exact commits, active job handle, receipts and next action; do not paste transcripts. Reviewer records review commits and outcomes after implementation closes.
