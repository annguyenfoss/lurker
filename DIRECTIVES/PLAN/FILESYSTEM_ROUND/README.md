# Filesystem installation and CLI integration round

User-authorized scope, 2026-11-25. Baseline: 671fa06 (native macOS APFS is already implemented). This round overrides broader historical release/fault-test requirements where they exceed the user's explicit scope.

See [round results](RESULTS.md) for the observed matrix, individual OS reports, independent reviews and remaining upstream or prerequisite limits.
See [unachieved-only retest results](RETEST_RESULTS.md) for later targeted fixes and the conditions of the new passes.

## Outcome and boundaries

Current global acceptance rule: **Create + Read/Write** for every targeted
OS/filesystem pair. There is one mount action and one unmount action; access is
determined by filesystem/driver capability, not a user-selectable mode. Use
read/write whenever an eligible route supports it. Cannot create, cannot mount,
or can only read is incomplete support. Read-only labels describe limitations;
they are not desired features or successful Create + Read/Write results. The
ordinary write/append/close/reopen payload flow is the filesystem acceptance
test; do not add independent read-only-mode qualification. These instructions
supersede the earlier round's read-only test requirements.

Cover Linux (Arch, Debian, Ubuntu, Fedora, openSUSE, Gentoo), macOS, FreeBSD, OpenBSD, NetBSD and DragonFly. Windows is excluded. Assess ext4, Btrfs, XFS, APFS, UFS1/FFSv1, UFS2/FFSv2, HAMMER2, NTFS, exFAT, ZFS/OpenZFS for each OS. README entries are research promises, not proof of installed/runtime support. Keep unavailable and read-only cells explicit.

Use existing integrated encryption methods only; do not implement planned encryption backends. Keep cipher/KDF/credential defaults unchanged. Filesystem selection is distinct from encryption selection. ZFS here is an inner filesystem, not authorization to add ZFS native encryption as an encryption backend. No GUI changes/testing, real physical-device tests, SQLite, fault injection, forced/lazy dismount, power cuts, security audit, benchmarks or exhaustive credential/cipher permutations. Default AES/password once per encryption route/filesystem is sufficient. Linux privileged and login-user VeraCrypt are distinct routes.

Install available OS packages, upstream binaries or unmodified upstream source. Prefer one adequate tool/suite per cell; add a second only for a material gap, e.g. formatter plus mount driver. Ordinary driver enablement and configuration are allowed. Do not patch kernels, drivers or upstream applications, backport fixes, invent filesystem bridges, disable platform protections, or start an upstream repair project. A missing compatible upstream build is a recorded limit. No filesystem microVM/WSL or paid product procurement. A module built against matching installed kernel headers is allowed; a custom patched kernel is not. Respect Buran: no reboot, shutdown, aggressive tests or broad changes. Installation is explicitly authorized; record prerequisites requiring interactive operator approval as blocked rather than bypassing them.

## Work sequence and evidence

1. Read AGENTS.md, CODEBASE.md, DEVELOPMENT.md and ENCRYPTION.md plus your OS plan. Inspect worktree/index. Read hosts.json and current code, not only earlier reports. Own only assigned hosts. Preflight SSH, OS/architecture, existing CLI/private backends, tools, mounts, sessions, disk/RAM and caches once. Reuse installations/build caches; no blind bootstrap. Store concise inventory in target/goal-validation/filesystem-round/OS/.
2. Select a minimal route per filesystem. Try package manager first, then upstream release, then upstream source. Bound each unavailable/failed route investigation to one useful build attempt plus one ordinary configuration correction; stop on required upstream source changes. Record install commands, versions, source pins/hashes, prerequisites and exact failure. Do not silently count candidates as passes.
3. Implement only necessary Lurker core/CLI filesystem format, probe, mount and close dispatch. Preserve defaults and existing source/mount/device identity guards, atomic publication, cleanup and ownership rules. Use small explicit adapters; no generic plugin framework or external profile configuration. Ask coordinator for shared contract changes, not permission for authorized work.
4. Commit each coherent change after appropriate remote checks and diff review. Stage only owned files/hunks; preserve other agents' edits. Shared checkout: never git reset, stash, amend or bulk add. Coordinate commits through a short /tmp/lurker-filesystem-git.lock advisory flock when committing, rechecking index inside the lock. Do not include someone else's staged files. Root coordinates pushes/final combined verification.
5. Build exact committed CLI/core/helper remotely. Existing remote runner core recipe or existing cache with recorded source snapshot is preferred. No desktop rebuild is needed for ordinary iterations; root will arrange final workspace checks. Install any elevated executable into administrator-controlled paths before executing it as root. KVM sudo password was supplied in the live user request; never persist/log it. Buran sudo is passwordless. Never elevate an executable from a user-writable build directory.
6. Use a small bounded script and JSON receipts, not repeated ad hoc commands. Root provides scripts/test-filesystem-cli.py; do not use test-backends.py's default SQLite workload. One fresh encrypted container for every feasible (encryption route, inner filesystem) pair. RAID1C uses its two file members. Write a real PDF and UTF-8 text, normal close, reopen, verify hashes, append text/add a second file, close/reopen and verify again. Do not falsely require directory fsync where a supported non-POSIX filesystem lacks it; record capabilities. Only normal non-forced close. Record read-only-only or import-only routes as shortcomings against Create + Read/Write; do not build a separate read-only-mode test flow. Preserve failure fixtures; reconcile only owned resources.
7. Directory engines are not block formatters: smoke each already integrated directory method independently, and test its backing-store combinations on disposable file-backed mounted filesystems where feasible. Label these directory-on-filesystem cases separately; do not present them as alternate inner filesystems of the directory format. Fixed APFS encryption likewise remains APFS-only. Explicitly explain structurally inapplicable pairs instead of faking a Cartesian product.
8. Produce an OS result table for all ten filesystems: installed route, create/format, CLI encryption pairing, actual access limitations, Create + Read/Write status, metadata/features known from upstream, observed lifecycle, limitations and evidence. Use the prior filesystem/driver capability research with the representative payload test; do not require an exhaustive feature audit to accept a successful expected workflow. Record discovered limitations explicitly. Native upstream failures are limits, not tasks to fix. Clearly separate product bugs from upstream/missing prerequisites.

## Shared ownership and contracts

Root owns api.rs/CreateFilesystem, CLI parsing/help, lib.rs exports, workflow.rs shared builders, common payload harness, shared policy docs and final aggregate. OS owners own their platform modules and OS-specific tests/report/installation scripts. FreeBSD owner additionally owns bsd.rs and a minimal shared bsd/filesystems.rs dispatch; coordinate its interface with the other BSD owners. Other BSD owners can add files under bsd/<os>/ and edit their native adapter only; do not independently edit bsd.rs. Root will expand CreateFilesystem with Ext4, Btrfs, Xfs, Apfs, Ufs1, Ufs2, Hammer2, Ntfs, Exfat, Zfs, Fat32 and as_str(). Tell root platform/backend restrictions discovered; runtime code must refuse unsupported combinations. Expanded enums require existing exhaustive matches to handle unsupported selections explicitly.

## Overengineering review (completed before delegation)

- One owner per OS; Linux owner covers six distributions. No nested delegation.
- No new encryption engines, GUI work, upstream patches, kernel replacement, full release packaging/signing, benchmarks or recovery suite.
- No all-cipher/keyfile expansion; one representative profile per supported method/route/filesystem.
- Reuse native tools, existing trust/session machinery and remote build caches. Use explicit commands and small helpers only where multiple routes actually need them.
- A failed upstream prerequisite is an honest blocked/unsupported row. Fix Lurker defects, never hide upstream limitations.
- No repeated full logs or polling loops. Send root only initial approach, a material blocker/shared-interface need, and completion. Keep status, active PID/job handle, commit, artifact hashes, receipt path and next action on disk. Completion message <= 250 words.

## Review stage

When implementation owner finishes, root records its terminal status and starts one fresh reviewer for that OS (not concurrently on the host). Reviewer reads this plan, OS handoff and concise result JSON first; inspects changes/evidence, verifies coverage and cleanup, reruns only missing/failed meaningful payload cases, and patches serious Lurker gaps with atomic commits. No expansion into excluded testing. Review result must list remaining limitations honestly. Root cannot mark round complete until all six OS reviews and final shared-code checks are reconciled.
