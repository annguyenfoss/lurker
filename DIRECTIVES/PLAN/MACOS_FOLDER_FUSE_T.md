# FUSE-T evaluation for CryFS and gocryptfs on macOS

Evaluate each backend independently on Buran. A candidate must demonstrate
that it mounted through FUSE-T and can preserve application data across
close and reopen. The current macOS runtime policy uses macFUSE for both
backends in Lurker packages. The `c997d6a` DMG provides the macFUSE
lifecycle baseline for comparison.
[macOS runtime policy](../LLM/CIPHER.md) ·
[Buran qualification results](../../docs/macos-backends.md)

## Ownership and test order

- Assign one subagent to CryFS and one to gocryptfs. They can review source,
  prepare separate worktrees and test recipes, and analyze results in parallel.
  The coordinator owns changes to shared build, packaging, and test files.
- Use Buran through the tracked `macos` SSH inventory. For direct SSH,
  use `adjutant@buran` with `buran.local` and `buran.swarm` as name-resolution
  fallbacks. Give one owner the Buran build and test window at a time.
  Prefer the remote runner for its
  host-wide lock. Route any direct SSH test through the same coordinator
  because that lock only covers runner jobs. Separate SSH sessions still
  share mount state, FUSE-T services, and the login user's Lurker registry.
- Start with CryFS because its existing experimental patch gives us a mount
  path and a specific SQLite failure to reproduce. Give gocryptfs the next
  Buran window after the CryFS owner verifies cleanup, even if CryFS fails
  a check.
- Give each backend a private source tree, output and dependency-cache
  directories, build logs, ciphertext directories, mountpoints, and app
  copies. Use distinct fixture paths and volume names. Existing workloads
  on Buran continue outside the test scope.

## Build and test notes from DEVELOPMENT.md

Use the workstation to edit, review, dispatch SSH jobs, and collect evidence.
Run native builds and all tests on Buran as the login user. Noninteractive SSH
should select the existing managed toolchains explicitly. Check OS and SDK
versions, available tools, active jobs, CPU and memory, and free space on the
filesystems holding source, caches, outputs, and fixtures. Reuse a verified
build or cache when its inputs match. Size worker limits and timeouts to the
measured host capacity. Buran is a live shared Mac. Coordinate prerequisite
installation and system configuration changes with its operator. The test
sequence uses ordinary vault closes without restarting or shutting down the Mac.
[Development runbook](../LLM/DEVELOPMENT.md)

1. Freeze the Lurker source commit, upstream component revisions, and any
   patches before dispatch. Keep experimental changes in separate commits
   from the production build path. Record the native toolchain, FUSE-T
   version, intended transport, Go module or Conan dependency identities,
   and effective flags. Keep passphrases out of command arguments and logs.
2. Batch related fixes and build each candidate once for the selected native
   target. Use distinct absolute output paths and pass `--cache`, `--output`,
   and bounded `--jobs` when reusing `scripts/build-folder-backends.py`.
   Its current production build selects macFUSE for CryFS. Build and stage
   the experimental FUSE-T candidate with a reviewed recipe.
3. Record the source commit, run ID, recipe and harness revisions, binary
   and package hashes, native dependency inspection, and result paths.
   Keep compiler logs remote and retrieve the relevant failed stage by
   name. Resume collection of an existing job after an SSH interruption
   and reconcile its state before starting another one.
4. Start with a native create, mount, write, close, and reopen smoke test.
   Reuse the verified artifact for subsequent checks. For applicable
   Lurker code changes, run `cargo check --workspace --locked` and
   `cargo test --workspace --locked` on Buran. Native lifecycle and package
   results require their own checks. [Remote testing](../../docs/remote-testing.md)

After preparing a reviewed custom recipe, dispatch it through the runner.
Substitute the frozen commit and actual recipe path from the handoff. Adjust
the example build worker count to match the preflight capacity.

```bash
python scripts/remote-test.py start --hosts macos --commit "$candidate_commit" \
  --spec "$recipe_path" --concurrency 1 --build-jobs 2
python scripts/remote-test.py collect "$run_id"
python scripts/remote-test.py log "$run_id" "$failed_stage" --host macos
```

## Backend investigations

### CryFS

1. Start at CryFS 1.0.3, commit
   `5d0cc36785d93e12250696a3ffc928042f2e8d61`, with the existing Conan lock.
   Review the experimental `cryfs-fuse-t.patch` alongside the production
   `fsyncdir.patch` and macOS compiler patch. Apply overlapping compiler
   fixes once and retain the reviewed durability behavior in the Lurker
   candidate. Record the exact effective patch and FUSE ABI.
   [Experimental patch notes](../../native/macos/README.md) ·
   [Directory backend build notes](../../native/folder/README.md)
2. Inspect the linked library with `otool -L` and verify the runtime and
   transport on the actual mount. Use the historical FUSE-T 1.2.9 result as
   the comparison point. That build passed ordinary writes, fsync, rename,
   and truncation, but SQLite could not enable WAL. Record the transport
   actually observed on the replay.
3. Try explicit FUSE-T FSKit selection if the candidate ABI supports it.
   Run the same workload for each transport under investigation and label
   its results separately. Capture the SQLite error, checkpoint stage,
   and any lock, sync, or truncate failure. Use the qualified macFUSE build
   as the control if the result needs isolation.

### gocryptfs

1. Start at gocryptfs v2.7.0, commit
   `756cd453d7afe69fd2a00a7592fd2d9869a6f756`. Check the pinned Darwin
   Go-FUSE mount path before attempting a runtime switch. The stock
   pure-Go build uses Go-FUSE, whose pinned helper lookup selects macFUSE
   or OSXFUSE by absolute path. Establish a supported FUSE-T mount path
   and verify its file-descriptor handshake with Go-FUSE.
   https://raw.githubusercontent.com/rfjakob/gocryptfs/v2.7.0/go.mod
   https://raw.githubusercontent.com/hanwen/go-fuse/2d50fff207d7/fuse/mount_darwin.go
2. If a compatible upstream route exists, pin it and verify its mount and
   unmount behavior in an isolated candidate. If it needs a new transport
   bridge, document the missing interface and the smallest repro before
   expanding the investigation. Reuse the qualified macFUSE build as the
   workload control where helpful.
3. Record the Go module identities with `go version -m`. Capture the helper
   executable path, the mounted filesystem identity, and any runtime process
   evidence needed to confirm the mount went through FUSE-T.

For either backend, prefer per-mount transport selection supported by the
candidate and leave system-wide FUSE settings outside the experiment. If a
failure could come from the upstream engine or macOS transport, run the
unmodified pinned upstream engine with equivalent settings wherever that
route exists. Record a missing equivalent control as a limitation of the
comparison. https://github.com/macos-fuse-t/fuse-t/wiki

## Native acceptance checks

| Stage | Required evidence |
| --- | --- |
| Direct backend smoke test | Confirm the FUSE-T runtime and transport on the mounted fixture. Create, mount, sync a write, close normally, reopen, and compare bytes. |
| SQLite WAL | Confirm SQLite actually enters WAL mode. Commit with `synchronous=FULL`, exercise a second connection and checkpoint, close the vault, reopen, and verify committed rows and `PRAGMA integrity_check`. Retain the SQLite version and exact error for failures. |
| Lurker CLI | Exercise explicit and Auto mount and close, wrong credentials, existing-target protection, aliased duplicates, read-only access, and busy close. Repeat ordinary close and reopen, including an immediate close after mount. |
| Durable payload | Pin hashes for `bitcoin.pdf`, `ethereum.pdf`, `lamport.pdf`, and `page.txt`. Run the payload harness `write` phase, close and reopen, then run `append` to verify the originals, append to the text, and add a fifth file. Close and reopen to run `verify`, then repeat close, reopen, and verification on Buran. |
| Candidate app, if packaging proceeds | Use a private app copy extracted from the exact DMG. Test the packaged CLI and Slint desktop helper, dependency resolution, and a fresh Firefox profile. Exercise scoped recovery only if the changes touch recovery and there is a macOS harness that can identify its owned process. |

Use [folder conformance](../../scripts/test-folder-backends.py),
[native CLI conformance](../../scripts/test-backends.py), and
[mounted payload verification](../../scripts/test-mounted-payload.py).
An unprivileged development CLI can point `LURKER_BACKEND_DIR` at the
absolute private staging directory. Track direct backend, development CLI,
and packaged artifact evidence under their own artifact hashes. Record OS
reboot as unexercised on Buran. [Native test guidance](../../docs/backend-testing.md)

## Cleanup and decision

Compare mounts, tracked helper processes, backend state, and the boot identity
with the preflight inventory. Close fixtures normally and reconcile any
uncertain mount before handing off Buran. Verify process identity and start
time before signaling an owned process. Preserve failed fixtures and their
evidence. Keep one checkpoint per backend under
`target/goal-validation/macos-folder-fuse-t/` with artifact hashes, actual
transport, checks passed or failed, cleanup status, and the next action.

Record each backend independently as passing, failing a named check, or blocked
by a concrete prerequisite or missing interface. A passing experimental result
supports review of that backend's production scope and packaging. If both
pass independently and simultaneous mounts matter, run a dedicated coexistence
case under one coordinated job with two disposable fixtures and verified
cleanup for both.
