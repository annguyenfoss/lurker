# Development, build, test, and validation runbook

## Development cycle

1. Read the relevant project directives and inspect the working tree and
   index. Define the intended behavior, the affected platforms and entry
   points, and the evidence needed to verify the change. For encryption
   changes, include the real create, unlock, write, close, and reopen
   lifecycle for the affected route.
2. Start from the latest verified baseline for the affected workflow. Use
   the batch process below to collect failures, implement compatible fixes,
   and verify them together. Keep logical changes in atomic commits and
   preserve unrelated work.
3. Choose the validation scope based on what changed. Workspace checks
   cover compilation and unit tests. Native functional tests cover real
   encryption and storage behavior. Installed-artifact tests cover package
   contents, dependencies, and the shipped CLI or desktop entry points.
   Include fault or recovery scenarios when the change affects them.
4. Deliver a verified candidate with its source revision, build artifacts,
   test results, and any outstanding issues. Report milestones as they
   pass so follow-up investigation can proceed from a known baseline.

## Remote execution boundary

The user's workstation is the editor/coordinator. Run **all tests on remote
VMs**, including Rust/Python unit tests, desktop probes and storage suites.
Run builds remotely by default too. A warm local cache does not justify testing
on the host. macOS work runs on Buran. Never touch installed Lurker sessions.

Read `CODEBASE.md`, `../PLAN/CROSS_PLATFORMS.md` and
`../../docs/remote-testing.md`. The latter documents the runner's actual
capabilities and historical results; this file explains operating discipline.

## Batch failure investigation, fixes and validation

Earlier qualification cycled through rebuilds and broad retests as individual
issues surfaced. Once the clean-install baseline passed, many follow-up checks
could run against that existing build. For example, the six stock Linux guests
ran all 54 loop-device profiles against the existing `14ede46` installers.
Use the following workflow to reduce the number of expensive build cycles.

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

## Remote job setup and evidence

1. Inspect the working tree/index and relevant directives. Preserve unrelated
   edits. Commit each ready atomic change; the runner deploys committed code only.
2. Check the selected guest's OS, toolchains, CPU/RAM, swap and actual storage
   locations. Reuse existing toolchains and caches; do not bootstrap blindly.
3. Freeze one product commit and dispatch an appropriate recipe with
   `scripts/remote-test.py`. Use that same product revision across the matrix.
4. Record run ID, full source commit, recipe identity and evidence paths in a
   small checkpoint. Distinguish a later harness-only revision from the product
   revision. Never relabel old binaries/results as a newer commit.
5. Collect structured summaries. Retrieve a named log only for a failed stage.
   Fix the specific cause before rerunning affected checks.

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
source snapshot once per job. Poll persisted state rather than redeploying.
An SSH timeout is an observation failure, not permission to start another job.

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

Cancellation is not cleanup. Check detached workers, job locks, mounts, devices,
temporary authorization, tmpfs and attached scratch disks separately. Reconcile
a stale lock with its owner record before removing it. A missing runner or lost
SSH connection is incomplete, not passed. Resume only when requested, from the
saved identity/state, without duplicating a possibly live job.

## Stopped checkpoint — 2026-11-15

The user stopped builds and cleared the implementation goal. **Do not resume
builds, package installation or repairs without a new instruction.** All build
jobs were canceled; process and custom-worker audits are retained in
`target/build-stop-20261115/`. OpenBSD was additionally checked directly and had
no build worker. No fault-test domains/networks are currently registered.

- Product package candidate: `2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1`.
  Later harness fix: `8293000` (resolve full identity with Git).
- Five Linux candidates passed privileged suites and 52 userspace cases each;
  cleanup then hit a harness bug. Its fix was committed, but the replacement
  userspace matrix was canceled. Do not claim complete final qualification.
- FreeBSD root was full and a temporary 2 GiB tmpfs nearly full. A separate
  16 GiB disk, serial `LURKERBUILD_20261115`, is attached live as `vdb`/`vtbd1`,
  mounted at `/mnt/lurker-build-scratch`. Persistent domain XML is unchanged.
  Its migration script failed with `SyntaxError`; caches were **not** moved and
  root space was **not** recovered. Inspect its `user/migration.log`. Scratch
  disk/tmpfs cleanup remains pending. Host files:
  `/mnt/virt/lurker-build-scratch-20261115/`.
- NetBSD's damaged rustix archive/source were renamed with
  `.damaged-20261115`; do not reuse those as valid cache entries.
- DragonFly has a missing Python shared library and malformed package database;
  package backups exist, but restoration was not completed.
- OpenBSD recovered boot/SSH and passed workspace/window checks at their
  recorded revisions. Final package staging needed sudo; inspect its retained
  result instead of assuming the subsequent installation completed.
- `/mnt/virt/lurker-fault-*` contains separate crash-test VM copies/evidence.
  `/mnt/virt/vmi/20261113T091832Z/` contains dated reusable VM images, not
  continuously updated backups. Do not delete or substitute either implicitly.
