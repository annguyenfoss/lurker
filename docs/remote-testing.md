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

## Previous frozen-source native build validation

Previous combined application source: `a16cd18b8462293c670e66803fc4e37fc4a9c59f` (2026-11-14). All eleven native hosts verified and tested that exact deployed Git snapshot.

Core checks compile and test `lurker-core`, CLI and helper. Workspace checks also cover the Slint desktop. Ignored native encryption tests were not executed by these jobs; packaged cipher lifecycle and durability suites have separate evidence in `docs/backend-testing.md`, `docs/linux-package-testing.md` and `docs/macos-backends.md`.

| Host | Scope | Passed | Failed | Ignored | Run |
| --- | --- | ---: | ---: | ---: | --- |
| archlinux-core-01 | Core | 60 | 0 | 12 | F |
| macOS (buran) | Workspace | 62 | 0 | 8 | A |
| freebsd-core-01 | Core | 46 | 0 | 5 | B |
| dragonflybsd-core-01 | Core | 34 | 0 | 1 | C |
| netbsd-core-01 | Core | 36 | 0 | 2 | C |
| openbsd-core-01 | Core | 43 | 0 | 9 | B |
| debian-core-01 | Workspace | 87 | 0 | 12 | D |
| ubuntu-core-01 | Core | 60 | 0 | 12 | E |
| fedora-core-01 | Core | 60 | 0 | 12 | E |
| opensuse-core-01 | Core | 60 | 0 | 12 | E |
| gentoo-core-01 | Core | 60 | 0 | 12 | E |

Total: **608 passed, 0 failed, 97 ignored** across eleven completed hosts. Debian reused its qualified workspace cache with debug information and incremental compilation disabled. Matrix builds used at most two active jobs with two Cargo workers each. Arch waited until the Linux runtime owner released its exclusive test window.

Run identifiers:

- A: `20261113T232436Z-29fdf1b7`.
- B: `20261113T232436Z-0d7fb74f`.
- C: `20261113T232657Z-4aa7d310`.
- D: `20261113T232820Z-f5d8546c`.
- E: `20261113T232820Z-a867207a`.
- F: `20261113T233001Z-848de1e8`.

The local aggregate is `target/remote-test/final-matrix-result.json`; each run retains its manifest, result JSON, source hash, toolchains, stage exits and log references. Generated artifacts remain ignored by Git. The earlier `76f05fbd2167313273c53201a070252719e4f4ea` baseline remains separately labeled in `target/remote-test/baseline-76f05fbd-matrix-result.json`: ten completed hosts, 511 passed, 0 failed, 85 ignored.

BSD core results do not claim BSD desktop qualification. macOS runtime/FUSE limitations remain documented separately. No runtime encryption tests were performed on endeavour.


## Native BSD desktop development evidence

These later checks qualify specific revisions; they do not relabel the earlier eleven-host baseline.

- NetBSD/OpenBSD `cargo check --workspace --locked` passed at `2cd73aa1c040b2c5b8fc726a096fa965094dfb77`, run `20261114T103726Z-ca3e2e77`.
- OpenBSD core/CLI/helper unit checks passed **65/0/12 ignored** at `f92c5e822dfa78b164388a300947101109790210`, run `20261114T114612Z-925c03b1`. The same built CLI/helper were copied to an administrator-controlled `/var/run` prefix for separate native RAID tests.
- NetBSD built the actual desktop test executable and helper at that same `f92c5e8` source, run `20261114T114612Z-e3fb6c8c`. Its opt-in CGD test then passed **1/0** as login UID 1300 through real Slint callbacks and the software renderer: create, mount, write/sync, unmount, reopen/read, and unmount, including helper-backed native listing. This is native GUI workflow evidence without a display-server requirement.

For the NetBSD test, official binary `sudo-1.9.17p1` was installed from the configured NetBSD repository. A temporary drop-in authorized only the root-owned test helper with fixed `--as-root run` and `--as-root list-native` arguments. `visudo` validated it before use. After the test, that exact drop-in was removed, preexisting policy hashes were verified unchanged, the native list was empty and all CGD/VND devices were idle. No broad or permanent passwordless delegation was added. Reports and policy/cleanup proofs remain in ignored `target/remote-test/native-desktop-development-f92c5e8/`.

DragonFly's first full desktop build exposed an upstream `rustix 1.1.4` target-cfg bug: unsupported IPv4 source-membership bindings were included through `freebsdlike`. The lockfile now selects official `rustix 1.1.6`, whose upstream cfg restricts those APIs to FreeBSD. This changes no kernel ABI or encryption code. Upstream source: https://github.com/bytecodealliance/rustix/blob/v1.1.6/src/backend/libc/net/sockopt.rs


Actual BSD workspace binaries were subsequently built at `ecc9175932650880afbb003e480adc28beb0555b`: DragonFly run `20261114T120401Z-5d15afad`, FreeBSD run `20261114T121544Z-2c47b13d`. All three products were archived with per-file digests under ignored `target/remote-test/{dragonfly,freebsd}-desktop-ecc9175/`. FreeBSD's software-renderer desktop suite passed **32/0/1 ignored** at `ed65cd90d91cd0da5189b71e3550d4fd84a666a0`, run `20261114T122722Z-72d85d40`, after giving the standalone zoom probe an explicit bundled font. The production window already rendered using its bundled fonts.

FreeBSD used an explicitly approved temporary 2 GiB build-only tmpfs, one worker, debug information/incremental compilation disabled, and RAM/swap/disk guards. Earlier conservative guard stops remain recorded as resource failures. The final cache, sources and relocated dependency files were archived and all 22,052 file/link entries verified before ordinary unmount. Only the exact temporary persistent caches were reclaimed, restoring about 1 GiB; native engines, sessions, failure fixtures and all result/log evidence stayed intact. Restore inventory and exact bytes remain in `target/remote-test/freebsd-desktop-ecc9175/complete-build-cache.tar.zst` and its JSON inventory.

## Real platform-window probe

`test-desktop-window.py` launches the actual desktop executable through winit/X11 and its software renderer. This is separate from the MinimalSoftwareWindow callback tests. It requires the login user's exact UID, an independently verified source commit and binary digest, plus test-only `Xvfb`, `xauth`, `xdpyinfo`, `xwininfo` and `xwd` binaries. It installs nothing and never creates/mounts encrypted storage.

```bash
python scripts/test-desktop-window.py \
  --binary /var/run/lurker-package/bin/lurker-desktop \
  --binary-sha256 EXPECTED_SHA256 --commit FULL_SOURCE_COMMIT --expected-uid LOGIN_UID
```

The probe creates a private authenticated display with TCP disabled, removes loader environment overrides, requires a real named window and varied rendered pixels, retains its XWD frame/logs, then reaps only its owned processes and removes the authority file. Standard output contains one JSON result reference; exit 0 is passed, 1 failed, 2 missing prerequisites. The result retains the probe's own digest. The caller supplies the commit/digest from its build/package provenance; the probe does not attest the build itself.

NetBSD's real window initially exposed runtime-library discovery missing from ELF `NEEDED`: xkbcommon is loaded dynamically. After installing official `libxkbcommon-1.13.2`, commit `3be5898e88b49d09fdfd697813ef6eb5ae526f0c` records the standard administrator-controlled `/usr/pkg/lib` and `/usr/X11R7/lib` paths in the desktop binary. Its actual native window rendered **1280×860 with 1,179 sampled pixel colors** without `LD_*` overrides. The private display, authority file and child processes were cleaned afterward. This proves the X11 route; Wayland and other BSD platform-window routes need separate evidence.
