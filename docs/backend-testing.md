# Native CLI backend conformance

`scripts/test-backends.py` runs the actual CLI artifact against newly allocated
private fixtures under `/tmp/lurker-conformance-*`. It never looks up an installed
Lurker, reuses an existing image, repairs storage, force-unmounts, or detaches a
device itself. All fixtures and detailed logs remain available after the run.
Use a remote test host; do not run this suite on endeavour.

Examples, after building/staging the matching native artifact:

```bash
# Native container operations: run under the appropriate authorized administrator.
python scripts/test-backends.py --binary /absolute/stage/bin/lurker --backend geli
python scripts/test-backends.py --binary /absolute/stage/bin/lurker --backend luks1 --cipher aes

# Run directory engines and Linux's explicit user route as the login user.
python scripts/test-backends.py --binary /absolute/stage/bin/lurker --backend gocryptfs
python scripts/test-backends.py --binary /absolute/stage/bin/lurker --backend veracrypt --unprivileged
```

`--root` selects an existing parent for a **new** private test directory, never an
existing vault. `--commit` records the exact source revision verified by the remote
build coordinator. Binary SHA-256, OS, architecture, privilege route, per-case
status and timings are saved in `result.json`. Standard output is one small JSON
summary with the result path. Nonzero exit means failed (1) or blocked (2).
The remote runner can invoke this as an installed-artifact stage in its custom
specification; the stage log contains the result reference.

The suite checks create, protection of an existing destination, unlock, Unicode
and spaces, fsync, atomic rename/truncation, SQLite WAL and integrity, duplicate
attachment refusal, busy close refusal, close, wrong credentials, reopen and
persisted contents, and read-only storage. Passwords are generated only in memory
and sent through a controlling PTY; logs redact credentials and echoing fails the
suite. The script never supplies sudo/administrator passwords. Native container
suites outside macOS require running the suite with administrator privileges;
userspace directory engines must run as the login user.

For a native API that cannot attach read-only, use
`--expect-readonly-unsupported` (for example, OpenBSD softraid). This **executes**
the read-only CLI request and requires both a nonzero exit and no mount at the
owned target. A writable fallback, unexpected success, or attached resource
fails the case. `readonly_expectation: "unsupported"` records that this tests
safe rejection rather than read-only operation support.

A backend without a native read-only operation may alternatively be tested with
`--skip-readonly 'concrete native limitation'`. That case remains **skipped** with
its reason and is not evidence of read-only support. This option and
`--expect-readonly-unsupported` are mutually exclusive. Every other required failure
stops the suite. A timeout signals only the owned CLI child and retains its storage
for inspection; a dead CLI does not prove mounts or backend processes are gone.

This is a fast lifecycle/application-I/O suite. It does not qualify every keyfile,
stale-session or PID-reuse case, full storage, abrupt shutdown, signing, dependency
independence, full-home login or desktop behavior. Those require the separate
release/fault suites in `DIRECTIVES/PLAN/CROSS_PLATFORMS.md`. Retained fixtures may
contain plaintext inside an attached mount; inspect `mount_present` and safely
close the recorded owned mount with the same artifact before deleting a fixture.
Never bulk-delete active test directories.

Host-independent harness tests use fake CLI processes and no real encryption or
mounts:

```bash
python -m unittest discover -s scripts/tests -p test_backend_conformance.py
```
