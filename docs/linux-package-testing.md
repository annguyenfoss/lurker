# Linux package qualification

`scripts/stage-unix.py` checks source identity, payload hashes, notices and native dependencies. Staging is not functional qualification. Run the extracted package on a disposable Linux VM, with administrator-controlled executables and ancestors; keep encrypted fixtures under `/tmp`.

`scripts/test-linux-package.py` drives the shared 13-case CLI conformance suite against the package's `bin/lurker`. It verifies every recorded package file before execution and removes developer backend overrides. By default it requires no external `veracrypt`, `java` or `go` on PATH; the package supplies its private engines and Java runtime. Kernel crypto/filesystem tools and FUSE remain host prerequisites.

Run both routes at the same complete source commit:

```bash
python scripts/test-linux-package.py --prefix /run/lurker-package/lurker --commit FULL_SOURCE_SHA --route kernel --result /tmp/lurker-package-kernel.json
runuser -u adjutant -- python scripts/test-linux-package.py --prefix /run/lurker-package/lurker --commit FULL_SOURCE_SHA --route user --result /tmp/lurker-package-user.json
```

The kernel route requires root and tests LUKS2, LUKS1 and VeraCrypt. The user route must run as the login account and tests unprivileged VeraCrypt, gocryptfs, CryFS and Cryptomator. Each route uses newly allocated private `/tmp` fixtures. Failed suites stop that route and retain evidence and storage; successful suites reclaim only closed images inside that job's own directory. JSON and redacted command logs remain available. No forced unmount, filesystem repair or VM termination is performed.

These seven suites are **AES package smoke checks**, including passwords, application I/O, aliases, busy mounts, persistence and read-only behavior. They do not replace the full AES/Serpent/Twofish matrix, keyfile tests, fault injection, desktop qualification or cross-distribution release gates. `--allow-external-tools` records a prepared-host result and does not prove runtime independence.

`test-linux-cross-mode.py --prefix PREFIX --result RESULT` adds an independent root-run source-exclusion suite. It switches to the login account for the userspace route and uses one new `/tmp` container with a binary keyfile. It tests both startup orders, hard-link/renamed aliases, a deleted kernel backing dentry, the native driver's post-lock check, read-only exclusion and three concurrent startup rounds. Exactly one route must win each race. Only normally mounted test resources are closed; failures preserve their evidence.
