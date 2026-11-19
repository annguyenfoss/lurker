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

`--auto-detect` retains the known format for create and sends `--type auto` for
mount/unmount, including incorrect-credential, alias, busy and read-only cases.
Results identify `detection_mode` as `auto` or `explicit`; keep these qualification
runs distinct. A new empty vault has no header to detect, so create stays explicit.

The suite checks create, protection of an existing destination, unlock, Unicode
and spaces, fsync, atomic rename/truncation, SQLite WAL and integrity, duplicate and aliased-source
attachment refusal, busy close refusal, close, wrong credentials, reopen and
persisted contents, and read-only storage. Passwords are generated only in memory
and sent through a controlling PTY only after terminal echo is disabled; missing
no-echo readiness fails within a bounded wait. Logs redact credentials and echoing
fails the suite. The script never supplies sudo/administrator passwords. Native container
suites outside macOS require running the suite with administrator privileges;
userspace directory engines must run as the login user.

For a login-user run, add `--firefox /absolute/path/to/firefox` to verify a
real browser profile on the encrypted mount. The suite launches headless
Firefox with a new profile, stores a generated value through a loopback page,
and checks that a second Firefox process reads it after the vault has closed
and reopened. The standard 13 cases still run and the report includes the two
Firefox cases separately.

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

## Captured packaged CLI evidence

The runtime baseline is `76f05fbd2167313273c53201a070252719e4f4ea`; the corrected
FreeBSD folder runtime is `a16cd18b8462293c670e66803fc4e37fc4a9c59f`. All runs used
harness `8bf9f0ee0ee31599f00c8244ca547c692712560e` (22 synthetic harness tests passed
on Arch Linux and all four BSDs). These revision identities are separate.

| Guest / packaged backend | Runtime | Passed cases | CLI SHA-256 prefix |
| --- | --- | --- | --- |
| FreeBSD 15.1: VeraCrypt, GELI, PEFS | `76f05fb` | 104 | `30825cbc222980a8` |
| OpenBSD 8.0: VeraCrypt, softraid CRYPTO | `76f05fb` | 78 | `43ba5ab59c5d13ce` |
| NetBSD 11.1: CGD | `76f05fb` | 26 | `3b345f237b8c4175` |
| DragonFly 6.4: LUKS1 | `76f05fb` | 52 | `07b682605ba75122` |
| FreeBSD 15.1: gocryptfs, UID 1300 | `a16cd18` | 26 | `3fcaf0d86ffd07a3` |

All 286 cases passed without skips or backend overrides. VeraCrypt and DragonFly
LUKS1 covered AES/Serpent/Twofish explicitly plus AES Auto; other formats covered
explicit selection and Auto. OpenBSD read-only cases verified **safe rejection**,
not read-only support. All encrypted data used private `/tmp` fixtures; packages
were installed in administrator-controlled `/var/run` prefixes. Package payload
hashes were checked before and after tests. No owned mounts, sessions or crypto
streams remained; temporary FreeBSD FUSE permissions/sysctl were restored exactly.

The initial `76f05fb` gocryptfs run failed closed after detaching its filesystem:
FreeBSD returned ESRCH for an exited process. That failure remains separate;
`a16cd18` added a native live-to-reaped process regression and passed fresh
explicit/Auto suites. The BSD native adapters and shared dispatch/model/profile
sources are unchanged between these runtime revisions; the folder fix is covered
by its new package tests. Do not present these rows as one identical binary build.

Full hashes, package manifests and collected result files are under:

- `/tmp/lurker-bsd-package-runs/pkg-76f05fbd-20261113T230230Z/` (FreeBSD/OpenBSD).
- `/tmp/lurker-bsd-package-runs/pkg-76f05fbd-20261113T231047Z/` (NetBSD/DragonFly).
- `/tmp/lurker-bsd-package-runs/pkg-a16cd18b-gocryptfs-20261113T232858Z/` (fixed FreeBSD gocryptfs).

Earlier development/prototype evidence remains in the `cli-5d1ec6ec` and
`pkg-66e8f9c4` run directories; closed successful prototype image payloads were
reclaimed with retained hash/provenance markers. Failed evidence was preserved.
These prepared-guest checks do not establish clean-machine independence,
fault-injection, desktop or release-signing qualification.
