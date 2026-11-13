# Scripted native test jobs

The coordinator deploys one exact Git commit into private `/tmp/lurker-matrix-<uid>/runs/<job-id>/source` trees. Existing guest checkouts and installed Lurker are untouched. The tracked inventory names the eleven supplied SSH hosts; authentication uses the caller's SSH configuration and agent. No credentials are copied to guests.

Run from the repository with its user-managed Python:

```bash
python scripts/remote-test.py start --hosts archlinux,freebsd,netbsd,dragonflybsd,openbsd --suite core
python scripts/remote-test.py collect RUN_ID
python scripts/remote-test.py log RUN_ID rust-check --host openbsd
python scripts/remote-test.py cancel RUN_ID --host freebsd
```

`start` returns a run ID immediately. A detached local dispatcher limits simultaneously active remote jobs (default two); each job also runs detached from SSH. `collect`, `inspect`, and `resume` collect persisted results without restarting jobs. Observation errors remain incomplete. Process identity is checked with its recorded start signature; a missing/replaced runner is reported incomplete rather than left running or assumed passed. The local manifest, summary, uploaded source, and dispatcher log live in ignored `target/remote-test/RUN_ID/`. The run is pinned to its original commit even if the local branch advances. Uncommitted edits are never deployed.

`core` runs core, CLI and helper compilation/unit tests. `workspace` also includes the desktop. Both are **build-only suites**: packaging and installed-artifact tests are explicitly skipped and no runtime encryption support is claimed. Custom `--spec recipe.json` supplies commands for backend build, packaging and disposable native CLI tests. For example:

```json
{
  "suite": "arch-native",
  "privilege_route": "unprivileged",
  "inner_filesystem": "btrfs",
  "stages": [
    {"name": "rust-build", "commands": [["cargo", "build", "-p", "lurker-cli"]], "timeout": 1800},
    {"name": "native-tests", "blocked": "replace with the reviewed disposable CLI test command"}
  ],
  "artifacts": ["debug/lurker"]
}
```

Commands are argv arrays, run in the verified source tree with stdin closed; no shell interpretation is added. `{python}` resolves to the remote job's selected interpreter. Native test scripts obtain `LURKER_TEST_JOB_DIR`, `LURKER_TEST_SOURCE_COMMIT` and `CARGO_TARGET_DIR`. They must use only owned disposable files/devices and clean up their own sessions. Never run runtime encryption or destructive suites on endeavour. Recipes and logs must not contain credentials. A missing required implementation or prerequisite uses `blocked`, not `skip`.

Prerequisites are checked, not installed. Rustup hosts explicitly select `~/.cargo/bin`; an inventory `rust_bin` override selects another existing absolute shim directory. Buran uses its existing `/opt/homebrew/opt/rustup/bin` rustup shims. OpenBSD/DragonFly use their approved repository Rust. An inventory `uv` executable plus `python_request` resolves an already installed managed interpreter with `uv python find --offline --managed-python --no-python-downloads --no-project --no-config`; it never installs or falls back to Apple Python when that configured manager is unavailable. Actual paths and versions are retained. Override an inventory only when a host's intended toolchain changes. Each job checks expected OS/architecture and free space independently for job files (default 256 MiB, `--minimum-free-mib`) and compiler caches (default 1 GiB, `--minimum-cache-free-mib`). Build concurrency per host defaults to two Cargo workers. Compiler output stays on the home filesystem under `~/.cache/lurker/remote-test` by default, not potentially small `/tmp` tmpfs. `--cache-root` or host inventory overrides it explicitly. Results retain both actual storage locations and free space. Cargo outputs use a separate `cargo/KEY` cache keyed by host OS/architecture, explicit compiler paths/versions and relevant Cargo/Rust build flags (including `env CARGO_PROFILE_...` assignments in stage commands). Cargo tracks source/lockfile/features itself, so unrelated backend recipes and native patches do not evict Rust/Slint dependencies. Native artifacts use `native/KEY`, additionally keyed by native inputs, build recipes and stages. Both complete identity hashes and paths are retained in results. Native build stages receive `LURKER_NATIVE_CACHE_DIR` / `LURKER_NATIVE_CACHE_KEY`; `{native_cache}` is available in argv recipes. Existing legacy caches remain intact unless an explicitly reconciled terminal cache is migrated. One host-wide job lock prevents cache and storage-test conflicts.

Each job publishes `result.json` atomically with source identity, toolchains, stage status/timing/exit codes, actual Rust test counts, log hashes and artifact copies/hashes. Logs stay remote; failures return only a capped excerpt (40 lines / 8 KiB). Named log retrieval uses the same cap. Successful compiler commands are never counted as passing tests. Aggregate collection returns nonzero for failed, blocked, queued, running or incomplete required jobs.

Cancellation prevents queued jobs from launching and creates a marker for launched jobs; the live runner terminates only a child process group it launched. It never signals persisted PIDs, kills VMs, or force-unmounts storage. A killed runner may leave its host lock and resources: inspect the lock's owner record, reconcile that job and clean only its verified resources before removing the stale lock. A crashed dispatcher leaves queued jobs unstarted; inspect its log and start a new run rather than resubmitting potentially launched job IDs. Preserve failure evidence; old job/cache cleanup is currently explicit, not automatic.

The harness does not replace absent-backend/clean-machine packaging qualification. It does not yet provide automatic bootstrap, release signing, retention pruning, automatic stale-resource reconciliation, or every backend's native test recipe. These remain explicit implementation work.

Harness verification (no SSH or encryption operations):

```bash
python -m unittest discover -s scripts/tests -p test_remote_test.py
```
