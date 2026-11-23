# Development, build, test, and validation runbook

## Development cycle

1. Read the relevant project directives and inspect the working tree and
   index. Use [ENCRYPTION.md](ENCRYPTION.md) to identify the approved routes and
   supported create options, then compare them with the code and available
   native receipts. Define the affected platforms, entry points, checks,
   and success criteria. Use day-to-day Arch Linux LUKS2 with elevation and
   VeraCrypt as the login user as the practical baseline for normal behavior.
2. Start from the latest verified baseline for the affected workflow. Use
   the batch process below to collect failures, implement compatible fixes,
   and verify them together.
3. Smoke-test a new or repaired route on its primary host with a real
   create, unlock, write, close, and reopen. Then run broader validation
   against the affected workflows. Workspace checks cover compilation and
   unit tests. Native functional tests cover real encryption and storage
   behavior. Installed-artifact tests cover package
   contents, dependencies, and the shipped CLI or desktop entry points.
   Include fault or recovery scenarios when the change affects them.
4. Review each atomic diff, run its applicable checks, and commit it when
   ready. Push validated commits as they land. Deliver the source revision,
   artifact identities, test outcomes, and remaining issues or scope
   exceptions. Preserve unrelated work in the index and working tree.

## Remote execution boundary

Use the workstation to edit, dispatch SSH jobs, and collect evidence. Run
all tests on their remote targets, including Rust/Python unit tests, desktop
probes and storage suites. Run builds remotely by default. Preserve existing
Lurker installations and sessions outside the job's test scope.

- **Linux and BSD.** Start development on Arch Linux and the affected BSD
  guests. For a full native matrix, include Arch, Debian, Ubuntu, Fedora,
  openSUSE and Gentoo, plus FreeBSD, NetBSD, OpenBSD and DragonFly BSD. Use
  the [tracked SSH inventory](../../scripts/remote-test/hosts.json) to select
  guests. Dedicated KVM guests can receive needed package updates and
  orderly reboots. Coordinate exclusive guest access before reboots,
  package changes or changes to attached storage. Run those operations
  within the assigned guest or its libvirt domain.
- **macOS.** Use the inventory's `macos` target via `ssh adjutant@buran`.
  If the short name fails to resolve, try `buran.local` or `buran.swarm`.
  Buran is a live shared Mac. **Never reboot or shut it down.** Pause the
  job if an operator is restarting it, then verify SSH and the job's state
  before continuing. Use private source, staging directories, app bundles,
  and disposable fixtures. Run as the login user and use the available
  passwordless sudo only for scoped tasks that require it. Coordinate
  prerequisite or system changes with the operator. See the
  [macOS build and package notes](../../docs/macos-backends.md).

The [cross-platform plan](../PLAN/CROSS_PLATFORMS.md) defines topology and
release coverage. The [remote runner reference](../../docs/remote-testing.md)
covers invocation, recipes, toolchains, and result collection.

## Coordinating full-matrix work

Before dispatching a multi-OS round, write a common brief and an OS-specific
handoff for each of Linux, macOS, FreeBSD, NetBSD, OpenBSD and DragonFly BSD.
Give each OS one owner when delegating. The Linux owner covers all six
distributions. Include the baseline revision, inventory keys, allowed host
operations, build recipe, artifact and fixture locations, test matrix, and
definition of done. Assign one owner per guest while it is in use and
coordinate edits to shared build and packaging files.

Keep the handoffs with the private run evidence under `target/goal-validation/`
in a directory specific to the round. Owners update their note with commit
and run IDs, result paths, blockers, and the next action so a later owner can
resume from that state. Let owners run their checks independently. The
coordinator reviews concise milestones and intervenes for shared changes,
host conflicts, or blockers. Reconcile each OS result against the agreed
checks before closing the round.

## Batch failure investigation, fixes and validation

Group compatible changes so a single native build can verify multiple fixes.

1. **Freeze a baseline and collect failures together.** Record the source
   revision, artifact hashes, and relevant guest and fixture details. Run the
   relevant lifecycle and failure cases against the same artifacts, using
   separate fixtures for cases that change storage or guest state. Run
   independent cases in parallel on separate KVM guests or disposable clones
   within the storage and concurrency limits below. Preserve each case's
   expected result, observed result, and logs. Continue independent checks
   after an isolated failure. If a shared prerequisite fails, record the cases
   it blocks and continue the other jobs.
2. **Batch compatible fixes.** Identify whether each finding needs a product,
   backend, package, or harness change. Group fixes that share build inputs or
   validation coverage. Keep commits atomic and run the checks required for
   each change. Schedule expensive builds and native regression checks for the
   batch when possible. An early targeted build is appropriate when a case
   needs a changed binary to diagnose the fault or unblock other checks.
3. **Build one candidate for the batch.** Freeze one source commit containing
   the batch and build each affected artifact once per required target and
   toolchain. Reuse verified build outputs whose inputs still match. A
   harness-only change can test the same installer. A packaging-only change
   can restage or repackage verified binaries when their inputs and provenance
   still match. Record the artifact hashes and use those exact outputs for
   every applicable test stage.
4. **Validate the batch and report the milestone.** Run the targeted
   regressions together, then qualify the affected platforms and workflows
   with that candidate. Scope a follow-up rerun to the inputs and behavior that
   changed. Run the broader matrix when shared changes affect it and the full
   required matrix for a final candidate. Report fixes, per-platform results,
   blockers, and evidence as each milestone lands. Queue new findings for the
   next batch unless they invalidate the current results or block a required
   check.

## Native lifecycle and persistence

Run the shipped CLI and desktop helper from the exact candidate package in
the target's test scope. Use the existing
[backend conformance suites](../../docs/backend-testing.md) and
[Linux package checks](../../docs/linux-package-testing.md). Use a clean
guest or isolated test environment for offline dependency and install/remove
checks. Keep a staged or development binary's results identified separately
from an installed package's results.

1. For full qualification, enumerate the supported create variations from
   [ENCRYPTION.md](ENCRYPTION.md), including ciphers, password or keyfile, privilege
   routes, and applicable options. Use representative sizes and boundaries
   for numeric inputs. Use fixture-owned loop devices for Linux device tests
   and the approved file-backed routes on BSD and macOS. Where supported,
   test both explicit and Auto discovery, read-only, wrong credentials,
   duplicate and aliased sources, existing-destination protection, and busy
   close. Include application and recovery cases affected by the change,
   such as SQLite WAL, atomic rename, or full storage.
2. Prepare a common payload with the Bitcoin and Ethereum papers, a third
   math or computer science PDF, and about one page of plain text. Pin the
   input hashes and expected hashes after appending. The
   [payload harness](../../scripts/test-mounted-payload.py) expects
   `bitcoin.pdf`, `ethereum.pdf`, `lamport.pdf`, and `page.txt`. Run its
   `write` phase on the verified mount, then close and reopen the vault.
   The `append` phase verifies the original bytes before it updates the
   text and syncs both that update and the new file. Use `verify` after the
   final reopen.
3. On KVM, leave the test vaults mounted and request an ordinary guest
   reboot. Record pre-reboot mounts and both boot identities. Batch
   independent fixtures in one reboot when native device limits permit.
   After SSH returns, reopen through the normal product workflow and
   compare all five SHA-256 digests. Record any repair, custom shutdown
   hook, or manual reconciliation as a separate result with its prerequisites.
4. On Buran, close and reopen the fixture again to verify all five digests.
   For changes to recovery behavior, use a scoped exit of an owned backend
   process where applicable. Record OS reboot as unexercised. For the native
   app, exercise the packaged CLI and desktop helper from a private copy of
   the app extracted from the candidate DMG. Use the intended FUSE
   prerequisites and login-user session.

If a failure could be in an upstream engine or native OS layer, run the
unmodified pinned upstream app on the same guest with the same workload
and comparable credential, privilege, and filesystem settings. For OpenBSD
VeraCrypt, test the upstream default consumer workflow as well as a matched
inner-filesystem control when they differ. Treat an upstream success as the
bar for Lurker. Record a shared limitation only when the equivalent upstream
workflow reproduces it. Keep ordinary reboot qualification distinct from
abrupt power-loss work, which uses [disposable VM clones](../../docs/vm-powerloss-testing.md)
when required by the task.

## Remote job setup and evidence

1. Commit the code under test before dispatch. The runner uses Git snapshots.
2. Check the selected guest's OS, toolchains, CPU/RAM, swap and actual storage
   locations. Reuse existing toolchains and caches; do not bootstrap blindly.
3. Freeze one product commit and dispatch an appropriate recipe with
   `scripts/remote-test.py`. Use that same product revision across the matrix.
4. Record run ID, full source commit, recipe identity, package and relevant
   binary hashes, and evidence paths in a small checkpoint. Record the
   harness revision separately when it differs from the product revision.
   Carry a prior result forward under its original identity after checking
   that the relevant inputs and artifacts still match.
5. Collect structured summaries. Retrieve a named log only for a failed stage.
   Fix the specific cause before rerunning affected checks.
6. Before closing the round, match each required check to its run result,
   confirm the recorded artifact and payload hashes, and verify cleanup.
   Record failed and blocked cases and any scope exceptions alongside passes.

Coordinator example (use its existing managed Python):

```bash
python scripts/remote-test.py start --hosts debian --suite workspace
python scripts/remote-test.py collect RUN_ID
python scripts/remote-test.py log RUN_ID rust-check --host debian
python scripts/remote-test.py cancel RUN_ID
```

Use inventory host keys, not guessed domain names. The runner key `freebsd`
maps to SSH host/domain `freebsd-core-01`. Query the system hypervisor explicitly:
`virsh -c qemu:///system list --all`; the default session may list no domains.
Check recipe stage names before requesting a log. `core`/`workspace` cover
compilation and unit tests; they do not qualify packaging or encryption runtime.
Use reviewed custom recipes for those. A successful compilation is not a test
pass; retain actual passed/failed/ignored counts and blocked prerequisites.

The local manifest/summary lives under `target/remote-test/RUN_ID/`; the remote
result retains precise source, toolchain, log and artifact identities. Upload a
source snapshot once per job. Poll persisted state after reconnecting and
reconcile any still-running job before launching another one.

## Context and token budget

- Script repeated SSH operations and aggregate locally. Return one row per host:
  host, commit, run ID, stage/status, counts, elapsed time, free space, evidence.
- Keep complete compiler output remote. For failures, return the first useful
  error plus at most 40 lines / 8 KiB, the exit code and full-log path.
- Read summary JSON first. Do not repeatedly dump full manifests, compiler logs,
  process lists, environment variables, package inventories or cache trees.
- Poll several hosts in one bounded collection, with a useful interval. Do not
  repeatedly reconnect to watch unchanged output. Keep the user informed while
  jobs run; orchestration calls should yield within a minute.
- Before context handoff, save only objective, frozen commits, run IDs, artifact
  hashes, remaining blockers, resource locations and next exact action. Evidence
  files replace transcripts. Read the checkpoint before searching old logs.
- Reuse existing scripts. If a repeated manual action is needed, turn it into a
  script with concise JSON output rather than another sequence of ad hoc SSHs.

## SSH, toolchains and privileges

Use local subprocess argv arrays, explicit timeouts and checked exit codes. For
remote commands use correct shell quoting (`shlex.join` where appropriate), or
send a reviewed script over stdin to an explicit `/bin/sh -s`. BSD root shells
may be csh: do not assume Bash syntax. Avoid nested shell/JSON interpolation.
Use Bash explicitly when a script actually requires it.

Select the existing uv/rustup/nvm user toolchains explicitly; noninteractive SSH
may not source their startup files. The four BSDs have user-approved official
package exceptions for Python/Node/Rust where upstream managers/binaries cannot
supply them. Do not repeat unavailable-version searches or source builds without
need. Cross-built backend artifacts must retain target, pins and hashes and
still require native runtime validation. Reuse a qualified builder rather than
installing another compiler solely to rebuild an identical artifact.

Build as the login user. Elevated tests use only owned disposable fixtures and
administrator-controlled helpers. Never elevate a user-writable executable.
Do not copy SSH credentials into guests or put passphrases in command lines/logs.

## Storage and concurrency

Check **each** filesystem used by root, `/tmp`, Cargo home, Cargo target, native
cache and fixtures. A 10 GB virtual disk does not guarantee 10 GB usable on `/`
or on a separate `/usr`. `result["cache"]` identifies the Cargo cache base;
compiler output is its `target` child. Resolve symlinks before measuring.

Estimate peak consumption, including extracted sources, link outputs, test
executables, archives and staging copies. Disable debug info/incremental output
consistently when appropriate; changing profile flags changes cache identity.
Runner free-space preflight is not a continuous guard. Monitor long builds and
stop before exhaustion; do not simply lower thresholds to get a job started.
Tmpfs consumes RAM/swap and can fill independently of the root filesystem.

Use measured guest and host capacity to set job/worker limits. Serialize jobs
per guest; the runner's host lock does not cover arbitrary custom scripts.
Do not run independent root/user coordinators against the same cache. Prefer a
dedicated temporary build disk over filling the system disk, but record its
domain attachment, serial, mount, ownership and cleanup state. Never repartition
the system disk as an incidental build fix.

Preserve failed fixtures/results. Reclaim only verified terminal job outputs,
after retaining required artifacts and manifests. Validate archive hashes,
source pins and restore inventory before trusting saved caches. Never delete a
shared cache just because one build failed.

## Stop, resume and cleanup

Cancel queued/running jobs through the runner. Then verify both compiler child
processes and custom coordinators on every affected host, including root-owned
workers. Check process identity/start time before signaling anything; never use
broad `pkill`, stop the VM, or force-unmount to stop a build.

Compare the final inventory against the preflight inventory. Check detached
workers, locks, packages, mounts, devices, temporary authorization, and scratch
storage. Reconcile a stale lock with its owner record before removing it. Keep
failure evidence and mark missing results incomplete. After a user-requested
stop, resume only when requested and use the saved job state to avoid
duplicating a potentially live job. Keep dated checkpoints with their run
evidence so the runbook remains applicable to future work.
