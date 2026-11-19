# Linux package qualification

`scripts/stage-unix.py` checks source identity, payload hashes, notices and native dependencies. Staging is not functional qualification. Run the extracted package on a disposable Linux VM, with administrator-controlled executables and ancestors; keep encrypted fixtures under `/tmp`.

`scripts/test-linux-package.py` drives the shared 13-case CLI conformance suite against the package's `bin/lurker`. It verifies every recorded package file before execution and removes developer backend overrides. By default it requires no external `veracrypt`, `java` or `go` on PATH; the package supplies its private engines and Java runtime. Kernel crypto/filesystem tools and FUSE remain host prerequisites.

Run both routes at the same complete source commit:

```bash
python scripts/test-linux-package.py --prefix /run/lurker-package/lurker --commit FULL_SOURCE_SHA --route kernel --result /tmp/lurker-package-kernel.json
runuser -u adjutant -- python scripts/test-linux-package.py --prefix /run/lurker-package/lurker --commit FULL_SOURCE_SHA --route user --result /tmp/lurker-package-user.json
```

The kernel route requires root and tests LUKS2, LUKS1 and VeraCrypt. The user route must run as the login account and tests unprivileged VeraCrypt, gocryptfs, CryFS and Cryptomator. Each route uses newly allocated private `/tmp` fixtures. Failed suites stop that route and retain evidence and storage; successful suites reclaim only closed images inside that job's own directory. JSON and redacted command logs remain available. No forced unmount, filesystem repair or VM termination is performed.

By default these seven suites are **AES package smoke checks**, including passwords, application I/O, aliases, busy mounts, persistence and read-only behavior. Add `--all-ciphers` to both routes to run all AES/Serpent/Twofish container profiles (9 kernel and 6 login-user suites, each with 13 lifecycle cases). These do not replace separate keyfile tests, fault injection, desktop qualification or cross-distribution release gates. `--allow-external-tools` records a prepared-host result and does not prove runtime independence.

`test-linux-cross-mode.py --prefix PREFIX --result RESULT` adds an independent root-run source-exclusion suite. It switches to the login account for the userspace route and uses one new `/tmp` container with a binary keyfile. It tests both startup orders, hard-link/renamed aliases, a deleted kernel backing dentry, the native driver's post-lock check, read-only exclusion and three concurrent startup rounds. Exactly one route must win each race. Only normally mounted test resources are closed; failures preserve their evidence.

`test-linux-block-package.py --prefix PREFIX --result RESULT` exercises all nine Linux backend/cipher block-create profiles using **only newly created `/tmp` images and loop devices allocated by the script**. It checks the loop backing device/inode before destructive create, tests write/fsync/reopen/read-only behavior, and detaches only verified owned devices with no remaining holders. It accepts no physical-device argument. This covers the private creator's `--quick` and omitted-size device path without using a real disk. Legacy native Rust matrix tests need the private creator; optional stock-VeraCrypt golden-reference checks require explicit `LURKER_TEST_STOCK_VERACRYPT=1`.

## Resumed package lifecycle matrix

The Arch, Debian, Ubuntu, Fedora and openSUSE KVM guests ran their own packages
built from `2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1`. Each passed 117
privileged checks (nine backend/cipher suites) and 78 login-user checks (six
suites), with zero failures or skips. These are prepared guests. Their
reports record no VeraCrypt, Java or Go executable on the test PATH.

The privileged results come from the original package qualification. The fresh
login-user run `20261119T171442Z-06f02cd7` used harness `8293000` and passed
fixture cleanup as well as the encryption checks. The original run reached
52 passed checks per guest before the cleanup harness rejected the intended
gocryptfs symlink fixture. Those results remain under
`target/linux-final-2ebae8f/state.json`. The collected replacement evidence is
under `target/linux-final-2ebae8f/resumed/` and records the package manifest,
CLI and conformance harness digests per guest.

The 975 passing checks cover both VeraCrypt routes, all three container
ciphers, gocryptfs, CryFS and Cryptomator. The lifecycle suites exercise
file creation, unlock, writes and SQLite WAL, close, reopening and reading
the persisted data, wrong passwords, read-only behavior and busy mount
handling. Keyfile, block-device, crash and desktop evidence is collected
separately.
