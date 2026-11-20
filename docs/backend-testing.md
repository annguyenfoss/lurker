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

`--session-close` selects the Linux/BSD shutdown hook for the busy and normal
close cases using the encrypted source path. Use an administrator-owned CLI
installation and test parent for native backends. Linux native file cases
also reject a replacement source while the original mount is still live.
Directory cases reject closing a vault by passing its plaintext mountpoint.

`--full-storage` runs a dedicated exhaustion workload on a fresh Linux or
BSD file container of at most 0.5 GiB. It checks that the mounted
filesystem is separate from the fixture parent before beginning bounded
writes. Use this option with `test-openbsd-raid1c.py` for mirrors. The
workload records `ENOSPC`, checks committed file and SQLite data, and
attempts a new 16 MiB SQLite transaction in a separate process. It then
frees the filler, writes and commits again, closes, and verifies the data
after reopen. The final check requires `SQLITE_FULL` from the writer.

The reusable harness passed nine checks on the OpenBSD RAID1C CLI from
`e607cc7`. On Debian's `14197ef` LUKS1 package, Btrfs reached `ENOSPC`
and SQLite 3.46.1 exited with `SIGBUS` during the new transaction.
The first two runs lost the in-process harness. With the writer in a
separate process, the eight lifecycle checks passed, including recovery
of the committed file and rows after reopen. The check for
`SQLITE_FULL` failed and the signal remains recorded. Evidence is under
`target/goal-validation/file-container-enospc/`.

FreeBSD 15.1-RELEASE-p6 passed all nine checks for AES VeraCrypt on
the installed `e607cc7` native package with harness `4554a0d`. The
128 MiB container's inner UFS reached `ENOSPC` at 129,073,152 bytes
of filler. SQLite 3.54.1 returned `SQLITE_FULL`, and the previously
committed file and row were still readable while the filesystem was
full. The suite removed the filler, synced a new file, committed a new
row, and verified both old and new data after a clean close and reopen.
Package, module, mount, md and session inventories returned to baseline
before the isolated guest was retired. The package, command and
filesystem receipts are under `target/freebsd-veracrypt-full-storage/`.

The suite checks create, protection of an existing destination, unlock, Unicode
and spaces, fsync, atomic rename/truncation, SQLite WAL and integrity, duplicate and aliased-source
attachment refusal, busy close refusal, close, wrong credentials, reopen and
persisted contents, and read-only storage. Passwords are generated only in memory
and sent through a controlling PTY only after terminal echo is disabled; missing
no-echo readiness fails within a bounded wait. Logs redact credentials and echoing
fails the suite. The script never supplies sudo/administrator passwords. Native container
suites outside macOS require running the suite with administrator privileges;
userspace directory engines must run as the login user.

Add `--firefox /absolute/path/to/firefox` to verify a real browser profile
on the encrypted mount. For supported privileged Linux and BSD routes, run
the suite through sudo or doas from the login account. The fixture uses
the package's invoking-user filesystem ownership and launches Firefox as
that user with a private process environment. The suite launches headless
Firefox with a new profile, stores a generated value through a loopback page,
and checks that a second Firefox process reads it after the vault has closed
and reopened. The standard 13 cases still run and the report includes the two
Firefox cases separately.

For OpenBSD RAID1C, pass `--owner-user` and `--firefox` to
`test-openbsd-raid1c.py`. The owner suite exercises the same profile write
and read around its normal close and reopen. Use an isolated guest with
loopback access for these tests.

On the Debian KVM guest, Firefox ESR 153.7.0 passed both browser-profile
checks for gocryptfs, CryFS, Cryptomator, and AES VeraCrypt in the
unprivileged route. Each suite passed 15 checks and ended with its mount
closed. The packaged CLI source was
`2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1` and the harness was
`ed94676`. Collected results and hashes are under
`target/browser-profile-qualification/`.

The Debian KVM guest also passed 15 checks each for AES LUKS2, LUKS1 and
privileged VeraCrypt on the installed `14197ef167f8f40b4393b0c77dfd0672757c102e`
DEB. Firefox ESR 153.7.0 ran as UID 1300 in a loopback-only network namespace.
Each profile stored a value and a separate browser process retrieved it
after close and reopen. All 45 checks passed with zero skips, and the
test package, mounts, loop devices and disposable encrypted images were
retired. Harness `9a6a897` also passed its 23 unit checks on Debian. The
package, CLI, manifest, harness and Firefox SHA-256 values and raw receipts
are in `target/browser-profile-qualification/debian-privileged-14197ef/`.

Harness `b9117eb` reran the LUKS2 profile with its restricted Firefox
environment. All 15 checks passed again on the exact Debian package,
the test package was removed, and the native fixture inventory was restored.
Receipts are in `target/browser-profile-qualification/debian-luks2-b9117eb/`.

On an isolated OpenBSD 8.0 KVM guest, Firefox ESR 153.4.0 ran as UID 1000
against the native-installed `e607cc7` package. CRYPTO and AES VeraCrypt
each passed 15 checks with harness `b9117eb`. RAID1C passed 13 invoking-user
checks with harness `5ea6e3f`. All three profiles stored and retrieved a
loopback page value in separate browser processes across close and reopen.
The guest verified 73 official package archives against the OpenBSD 8.0
package signing key and disabled its only non-loopback interface for the
workload. Native mounts, devices and receipts matched their baselines.
The test package was removed and the guest shut down normally. The transient
domain, isolated bridge, overlay and control credentials were removed.
Evidence is in `target/openbsd-firefox-bed2448/`.

FreeBSD 15.1-RELEASE-p6 passed 15 checks each for GELI and AES
VeraCrypt with the installed `e607cc7` package and harness `b9117eb`.
Firefox ESR 153.7.0 ran as UID 1300 with a fresh profile in the encrypted
mount. Only loopback was up during the replay. Both profiles stored a value,
which a second Firefox process recovered after closing and reopening the vault.

The first headless Firefox process exited with SIGSEGV on a guest where
Fontconfig listed zero fonts. The rerun passed after installing DejaVu
from the signed FreeBSD quarterly catalogue. The guest installed the
verified browser prerequisites offline, removed them and Lurker after
the replay, and restored its package, kernel module and native storage
inventories. The isolated guest, network and scratch disk were removed.
Raw receipts, hashes and command logs are under
`target/freebsd-browser-profile/`.

FreeBSD 15.1-RELEASE-p6 also passed 15 checks each for PEFS and
gocryptfs with the retained full native package from
`ca3c8456e507102a1c6e2b9194981afbd48df8db` and harness `b9117eb`.
PEFS used the installed kernel module through sudo. gocryptfs used the
package's private engine through the login-user FUSE route. In both
cases Firefox ESR 153.7.0 ran as UID 1300 with only loopback. A second
process recovered each profile's stored value after a clean close and
reopen.

The isolated guest reused 105 signed Firefox and font prerequisite
archives offline. It restored its package, mount, module, session and
FUSE settings before shutdown. The guest, network and scratch disk were
removed. Receipts and per-case logs are under
`target/freebsd-directory-browser/`.

On a fresh stock NetBSD 11.1 amd64 KVM guest, CGD passed all 15 cases with
the installed `e607cc7` package and harness `b9117eb`. Firefox ESR 153.7.0
ran as UID 1300 with DejaVu fonts and only loopback. A second Firefox
process recovered the recorded page value after ordinary close and reopen.
The host and guest verified 81 official archives against NetBSD's
published SHA-512 catalogue, including the Firefox and Python packages.
Stock `pkg_add` installed them offline. The native storage inventory matched
its baseline, Lurker and the temporary sudo rule were removed, and the
guest shut down. Results, captured frames, and the initial signature probe
are under `target/netbsd-firefox-e607cc7/`.

https://cdn.netbsd.org/pub/pkgsrc/packages/NetBSD/x86_64/11.0_2026Q3/

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

## Session close qualification

The hook change in `0eea358` passed these focused KVM checks. The source
snapshot used base `b0c71a6` plus the byte-matched hook change and has SHA-256
`753d50628d98dffe3ccf1f8400be53f36091c3da254313618001427f4364ffee`.
This run is separate from the `a60e4bc` package matrix.

| Guest | Backend and privilege | Passed |
| --- | --- | ---: |
| Debian | Native LUKS2 and VeraCrypt | 28 |
| Debian | Login-user gocryptfs and VeraCrypt | 27 |
| NetBSD | Native CGD | 13 |
| FreeBSD | Native PEFS | 14 |

Workspace checks passed on Debian and NetBSD. Their Rust suites passed 135
and 95 tests with 13 and four ignored, respectively, and each guest passed
all 22 conformance harness unit tests. The FreeBSD CLI and helper built
against the original guest's matching official PEFS package. Its mount,
module, permissions, package and registry inventories matched the baseline
after cleanup.

On NetBSD, holding the registry lock for the disposable CGD fixture caused
the hook to return `124` after 1.017 seconds while the worker and mount were
still live. Releasing the lock let the worker close the vault normally.
Reopening recovered the synced file and SQLite data, and all CGD and vnode
devices were idle after cleanup. The tests ran before operating-system
teardown. The [session close usage notes](usage.md#opt-in-session-and-shutdown-close-on-linux-and-bsd)
describe supervisor ordering and worker lifetime.

The initial Debian native VeraCrypt run lacked its private creator in the
test stage. The retry used the hash-verified creator and libraries from the
earlier native package inputs. A concurrent login-user VeraCrypt attempt
refused an inaccessible native loop backing file. The separate retry completed
with both privilege routes idle. Input manifests, CLI hashes, initial
failures and complete result logs are in `target/session-hooks-validation/`.

### Clean systemd shutdown after the close hook

An isolated Arch KVM clone ran the documented explicit stop workflow with
the CLI and helper built at `9b428fb732477bee6d23def56c29dab9ab018f19`
in `20261119T194103Z-0266eb17`. A systemd service used a disposable
LUKS2/Btrfs mount to fsync a file and commit a SQLite FULL/WAL transaction.
The guest stopped the service and verified the consumer had exited. The
`session-close` hook then returned zero without a credential prompt and
removed the mount before the guest requested systemd poweroff.

Libvirt observed a clean stop. The restarted guest had a new boot ID and
reopened the same vault. The file digest and SQLite row matched and the
integrity check passed. The initial post-reboot verification kept its
SQLite reader open during unmount, which was safely refused as busy.
The repeat explicitly closed its reader and the vault closed normally.
The initial setup also verified safe refusal when the CLI-only test stage
lacked its trusted helper. This case exercises an administrator-ordered
close before shutdown. Evidence is under
`target/goal-validation/arch-shutdown/`.

### FreeBSD close hook in `rc.shutdown`

An owned FreeBSD KVM clone used the verified `ca3c845` portable package
to create and open a GELI AES fixture. A separate process fsynced a file,
committed a SQLite FULL/WAL row, and held another row uncommitted. The
guest's native shutdown script ran the fixture hook during `rc.shutdown`.
It verified the consumer identity, stopped it, waited for SQLite close,
and called `session-close`. The hook returned zero without a credential
prompt and the mount was gone before poweroff.

After the same guest booted again, the ordinary CLI reopened the same
vault. The file matched its digest, SQLite integrity passed, and only
the committed row was visible. The reader retry fixed a SQLite context
manager error in the verification harness. Normal close left the vnode
inventory idle. The temporary shutdown script was removed from the clone.
Evidence and package hashes are under
`/mnt/virt/lurker-fault-fb20261120a/` in `native-shutdown-result.json` and
the matching staging and inventory logs.

### DragonFly LUKS1 close hook in `rc.shutdown`

A standalone copy of the verified DragonFly 6.4.2 stock installation used
the native `ca3c845` package, SHA-256
`67941ec7534a0449da3c84ef43ba8e213e2ab97a8fa72a50ca0a377cf2423d6a`.
The SQLite consumer fsynced a 64 KiB file, committed a FULL/WAL row and
left a second row uncommitted. The temporary `/etc/rc.local.d` hook ran
under DragonFly's native `rc.shutdown`. It matched the writer's process
identity and requested exit. After SQLite rolled back and closed and the
writer exited, the hook ran `lurker session-close` with the absolute source.
The command returned zero with stdin closed. The mount, vnode, mapper
and journal inventories matched baseline before libvirt recorded the
guest's graceful poweroff.

After reboot, the installed CLI reopened the same LUKS1 vault. The file
digest matched, SQLite integrity passed, and only the committed row was
visible. Ordinary unmount restored the native inventory. The first trial
recorded a safe refusal when the configured source was under `/var/tmp`.
The passing trial used an administrator-controlled path under `/root`.
The guest removed the package and temporary hook, shut down normally, and
its isolated network was removed. The stock template's SHA-256 matched
its starting value. Evidence is under `target/dragonfly-native-shutdown/`.

### OpenBSD close hook in `rc.shutdown`

A dedicated OpenBSD 8.0 KVM overlay installed the native `ca3c845` package
offline. The guest mounted softraid CRYPTO, RAID1C and VeraCrypt FFS
fixtures with the synchronous flag. Separate SQLite consumers fsynced
their files, committed FULL/WAL transactions and held another row
uncommitted in each vault.

The native `rc.shutdown` hook verified each consumer's process identity,
stopped it and waited for SQLite to close. It then ran `lurker session-close`
for each vault. Every close returned zero without a credential prompt and
the mount, vnode and softraid inventories matched the baseline before the
console reported disk sync. Libvirt recorded a guest-requested shutdown.

After reboot, the ordinary CLI reopened each vault. All three file
digests and committed SQLite rows matched, all integrity checks passed,
and none of the uncommitted rows appeared. Each vault detached normally
without a filesystem repair step. The test installed its hook only on
the clone and removed it before the final shutdown. Evidence is under
`/mnt/virt/lurker-shutdown-ob20261120/` in `result.json`,
`test-shutdown-serial.log` and `test-lifecycle.log`.

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

## Resumed FreeBSD package qualification

FreeBSD 15.1-RELEASE-p6 amd64 passed 208 packaged CLI checks at
`029cf99a9c3876e038d03d80747f7ac7f2d3470d`, with no failures or skips.

| Backend | Passed | Routes exercised |
| --- | ---: | --- |
| GELI | 39 | Explicit and Auto passwords, Auto binary keyfiles |
| PEFS | 39 | Explicit and Auto passwords, Auto binary keyfiles |
| VeraCrypt | 104 | AES, Serpent and Twofish with passwords and binary keyfiles, plus Auto AES for both credential types |
| gocryptfs | 26 | Explicit and Auto as UID 1300 |

The suites created real encrypted storage, wrote and fsynced data, closed and
reopened, checked persisted contents and SQLite WAL/locking, and exercised
wrong credentials, busy close and read-only behavior. Four login-user desktop
callback tests performed real create/write/reopen/close operations through the
installed helper. The installed desktop rendered a 1280×860 X11 window with
1,088 sampled pixel colors on a private Xvfb 21.1.26 display. The native
VeraCrypt generation test passed after killing its identified disposable
daemon, using the engine from this same package.

The earlier `2ebae8f` package failed both gocryptfs suites at SQLite WAL
checkpoint. FreeBSD FUSE combined the requested size with a deferred atime
update, and the pinned backend sent the timestamp to the current directory
descriptor. The FreeBSD-only patch in `029cf99` uses the open file descriptor.
The original failed fixture database, WAL, and shared-memory snapshots were
hashed and retained before ordinary close. The patched package passed both
complete gocryptfs suites. Temporary mount permissions, sysctl and kernel
modules were restored. No owned mount, memory disk, GELI mapping, or active
session remained. Expected failed-authentication receipts were retained.

Remote build `20261119T174352Z-63783019` passed workspace check, build, and
tests (110 passed, 0 failed, 10 ignored). The staging audit covered 17 ELF
records and verified 77 payload files. Package SHA-256
`f77ebaf466876d7ff12f026b9592d778f2c87f2a4f37fe751bc130ffc134ba91`.
The guest result is `/tmp/lurker-freebsd-qualification-rhu9eg1v/summary.json`,
with an earlier failure and diagnosis under
`/tmp/lurker-freebsd-qualification-wx0843tb/`. The collected summary, package
manifest, and X11 frame receipt are under `target/freebsd-final-resume/`.

## Coordinated FreeBSD native package qualification

The frozen `a60e4bc69a3b78b8dd6e2297c2300a38f4508251` build on FreeBSD
15.1-RELEASE-p6 amd64 passed workspace check and build. Its workspace tests
passed 110, with 10 ignored and none failed. Remote run
`20261119T184037Z-1972fdb8` verified and reused the existing FreeBSD
VeraCrypt 1.26.31 and patched gocryptfs 2.7.0 engine inputs. The new staged
manifest binds those engine bytes and the rebuilt frontends to this source
revision.

The isolated native package installation passed all 208 CLI checks in the
same backend and credential routes listed above. These cases ran against the
native-installed binary and exercised real create, unlock, file and SQLite
WAL I/O, fsync, close, reopen, read-back, incorrect credentials, busy close,
and read-only behavior. The native package also passed four real login-user
desktop callback lifecycles, the VeraCrypt generation recovery test with its
packaged engine, and the 1280×860 X11 window probe as UID 1300. The guest
reported GELI kernel and userland version 7, FUSE kernel ABI 7.35, and
PEFS module package `g20260829.1501000,1`.

Native installation used an isolated root and package database with 22
local official dependency artifacts. Removal left the original package
database unchanged. The guest restored its mount, md, GELI, session and
kernel-module inventories, FUSE permissions, and `vfs.usermount`. This
run exercised ordinary close and reopen. The earlier GELI power-cut result
remains recorded in `docs/vm-powerloss-testing.md`.

The portable archive SHA-256 is
`b57f0331ed0c805ff452bb9b63e71087016c6ccd7be65edc3b76f9bcee4c3d7b`.
The native package SHA-256 is
`99d5cdf490d64908ded14b8f69e8febe0b3bb0ba6d0c2348b0a55f1e7617fac9`.
The build result, package receipts and individual runtime results are under
`target/freebsd-frozen-a60e4bc/`. The guest retained the full runtime logs
under `/tmp/lurker-freebsd-qualification-zwjrl3i6/`.

### FreeBSD VeraCrypt ciphertext sync qualification

The FreeBSD-only VeraCrypt sync fix at
`2fda5f7ca413f73ac7c9d3ec51feac007119386e` passed workspace check, build,
and tests on FreeBSD 15.1-RELEASE-p6 amd64 (112 passed, 0 failed, 10
ignored) in remote run `20261119T192927Z-0b1ab4b3`. The isolated native
installation passed all 104 VeraCrypt CLI checks across the eight cipher
and credential routes above. The packaged engine also passed daemon
generation recovery, the login-user desktop VeraCrypt create/write/reopen
and close lifecycle, and a 1280×860 X11 window probe as UID 1300.

The normal close and reopen tests verified file contents and SQLite WAL.
The guest removed the installed Lurker package, preserved its original
package database, and restored the mount, md, GELI, session and kernel
module inventories, FUSE permissions, and `vfs.usermount`. The ciphertext
barrier trace is recorded in `docs/vm-powerloss-testing.md`.

The portable archive SHA-256 is
`be8d7cf4aa3c5cc5dbb20755f2702d7df8c0c597332716411bf12ab07e81bb3f`.
The native package SHA-256 is
`19eccbd1cfadbc7bffc31e52e396546fe8d4e7d198866b97b9ede4f9ad0e5272`.
The build, package, and runtime receipts are under
`target/freebsd-vc-barrier/`. The guest retained the runtime logs under
`/tmp/lurker-freebsd-qualification-jwei413n/`.

### FreeBSD GELI, PEFS and gocryptfs storage exhaustion

The exact `a5a1857312fcad45bab0bf94e503810900774caa` native package passed
24 checks on FreeBSD 15.1-RELEASE-p6 amd64 in remote run
`20261119T200542Z-3b2501de`. Workspace check and build passed, with 112
tests passed, 0 failed and 10 ignored. The package installed and was
removed in an isolated root and package database.

Each eight-check workload created and unlocked a new vault, fsynced a
file and a renamed directory entry, and committed a SQLite FULL/WAL
transaction. Writes through the mounted vault filled a dedicated 128 MiB
filesystem. GELI used its inner UFS. PEFS and gocryptfs each used a separate
file-backed UFS for ciphertext. The gocryptfs workload ran as UID 1300.

| Backend | Filler size at `ENOSPC` | SQLite write error |
| --- | ---: | --- |
| GELI | 129,302,528 bytes | `SQLITE_FULL` |
| PEFS | 129,105,920 bytes | `SQLITE_FULL` |
| gocryptfs | 128,122,880 bytes | `SQLITE_FULL` |

Previously committed file and SQLite contents remained readable at
exhaustion. After deleting the filler, each vault accepted a new fsynced
file and a SQLite commit. Ordinary close, reopen, read-back, database
integrity and final close passed. The rejected transaction stayed absent.
The guest restored its original mount, md, GELI, session, kernel-module,
FUSE and package-database state. The separate GELI abrupt power-loss
results are recorded in `docs/vm-powerloss-testing.md`.

The native package SHA-256 is
`3522c4f1fc050c0867a7085fac72607a85ca62ab6e36b9b6205f9fce1bafd188`.
The portable archive SHA-256 is
`94fc0f365e7a88dee16a0fe2de204337aa14226d414d07aef628260253ff101d`.
Build and runtime receipts are under `target/freebsd-full-a5a1857/`.
The guest retained the workload logs in `/tmp/lurker-freebsd-enospc-dx9mkgyn/`.

### FreeBSD native package after the Rust dependency updates

FreeBSD 15.1-RELEASE-p6 amd64 built and wrapped the exact
`ca3c8456e507102a1c6e2b9194981afbd48df8db` workspace in run
`20261119T204122Z-30b37113`. Workspace check and tests had passed at the
same source in `20261119T202105Z-c5d427b1` (112 passed, 0 failed, 10
ignored). The private VeraCrypt and gocryptfs binaries, libfuse and
libiconv matched the `a5a1857` native package byte for byte.

The native installation reused 22 official dependencies in the isolated
package root. It passed all 208 CLI checks (39 GELI, 39 PEFS, 104
VeraCrypt and 26 gocryptfs), four login-user desktop storage lifecycles,
VeraCrypt daemon recovery and the 1280×860 X11 window check as UID 1300.
The replay resumed from the successful explicit GELI result after
correcting a fixture cleanup check for its deliberate hard link. The
guest restored its mount, device, session, FUSE and module inventories
and both package databases after removal.

The native package SHA-256 is
`9fed88319d7339a1a36ce1aba36e5dca45fdd2d8b04e6974df9f9fa3b17c6022`.
The portable archive SHA-256 is
`7281db84b8c9f0efc8efe19fe031feab31b3eb826072a3c90eeb084bfe045cb6`.
The packages and receipts are under `target/freebsd-ca3c8456/`. Guest
workload logs are under `/tmp/lurker-freebsd-ca3c8456-gdkpbq_m/`.

### FreeBSD descriptor-backed staging package

FreeBSD 15.1-RELEASE-p6 amd64 built the native CLI and helper from
`e607cc73edcfdec78055dc02b26e077474bb3b8c` in run
`20261120T014916Z-d3fe0ce7`. The package selected GELI and VeraCrypt
and installed offline in an isolated KVM guest without additional
packages. It reused the verified VeraCrypt executable and private
libfuse and libiconv bytes. The staging inventory includes both
wxWidgets notices from the pinned source archive.

All 143 installed CLI checks passed, with no failures or skips. GELI
passed 39 checks across explicit, Auto and binary-keyfile routes.
VeraCrypt passed 104 across explicit AES, Serpent and Twofish and AES
Auto, each with password and binary-keyfile credentials. The suites
created new images, checked overwrite protection, fsynced file and
directory changes, committed SQLite FULL/WAL transactions, and verified
data after close and reopen. They also exercised busy close, incorrect
credentials, hard-link aliases and read-only mounts. Each fixture's
staging directory was gone after its lifecycle.

The guest removed the package and restored its package, mount, md,
GELI, VeraCrypt process, session and kernel-module inventories. The
isolated guest, network and scratch disk were removed after log
collection. The original FreeBSD domain kept its running state,
configuration and disk attachment.

The native package SHA-256 is
`bebd8aa8bc53fafb7cfe7cc49e1be25717858db26fb5d1c0e06a8339d5f27bdf`.
The staging archive SHA-256 is
`fdfdc941aad85a72b37a117f8946ccf8d9c12bc677c9d9f87e2aa4a97f428353`.
The build, package, test and cleanup receipts and command logs are
under `target/freebsd-descriptor-staging/`.

## Resumed NetBSD package qualification

NetBSD 11.1 amd64 passed 26 packaged CGD checks with no failures or skips at
`2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1`. The explicit and Auto suites each
verified a real encrypted file container, writable mount, file and SQLite WAL
persistence after close and reopen, wrong-password rejection, and read-only
mounting. The login-user desktop callback test passed through the installed
helper. The packaged desktop opened a 1280×860 X11 window with 1,179 sampled
pixel colors. All CGD and vnode devices were idle afterward, the native session
list was empty, and the temporary sudo rule was removed.

Remote run `20261119T171506Z-79e7c727` passed workspace check, build, and tests
(92 passed, 0 failed, 4 ignored). All three ignored core tests passed separately
in an administrator-controlled test prefix, and the fourth passed in the
login-user desktop callback run. Staging initially rejected the standard NetBSD
X11 RUNPATH. Packaging fix `7e242073a8d5eeebceff44a95193ceefa128e819` staged
the same compiled binaries. The archive and complete results are in
`target/netbsd-final-resume/`. The archive SHA-256 is
`91b001fd98ddbcd9df240c8dcccfa814d51864b4d1c7a5982c955d9acf175d41`.

## Resumed DragonFly LUKS1 package qualification

DragonFly 6.4.2 x86-64 passed 39 binary-keyfile checks across AES, Serpent
and Twofish using the packaged `2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1`
CLI and native cryptsetup 1.1.2 with UFS. Each cipher covered creation with
a 256-byte binary keyfile, Auto unlock, synced file and SQLite writes,
close, wrong-keyfile rejection, reopen, read-only mounting, and cleanup
of the vnode, mapper and owned session. Every case passed without skips.

Build `20261119T172622Z-beaa837c` produced the CLI/helper archive with
SHA-256 `0f13bd11325740ec454b02164fdd72673479030d6a5fa32cf24a0662cd3e942b`.
The CLI SHA-256 is
`3e4b9a0a0023e16ff47b6c7a92f4e8956b1c3a5c58b64b32ef596902306360c7`.
Results from the isolated KVM copy are in
`target/dragonfly-resume/keyfiles/`. Harness `b78d314` enables native
resource checks, using the compatibility fix in `12d7e7f`.

The full desktop archive from the same build opened a native 1280×860 X11
window as UID 1300 with 1,200 sampled pixel colors. The test used an
isolated copy of the verified November 13 KVM archive with official
DragonFly X11 packages installed offline. The probe disabled TCP and four
unused X extensions, used private Xauthority, and reaped all owned
children. The window result and full package manifest are in
`target/dragonfly-resume/`. The full archive SHA-256 is
`22feef4528e2d684182b5900381721e5c076b01aabf960e43992931737e3a742`.

The full archive also passed 91 live CLI checks with no skips: 52 password
checks across AES, Serpent, Twofish and AES Auto, plus 39 binary-keyfile
checks across the three ciphers. The packaged helper passed the login-user
desktop callback lifecycle, including create, synced write, close and
reopen/read. Build `20261119T172622Z-beaa837c` passed workspace checks,
build and unit tests (90 passed, 3 ignored). The remaining two native core
tests passed separately on owned storage.

An eight-case UFS exhaustion run filled the owned encrypted filesystem until
`ENOSPC`, read the existing payload, freed the filler, wrote and synced new
data, and verified both files and SQLite after reopening. Post-test checks
found idle vnode devices, an empty mapper list, and no Lurker session records.
The temporary sudo files and authorization were removed. The failed initial
orchestration output is retained on the guest along with the successful
results in `target/dragonfly-resume/full/`.

## NetBSD CGD with WAPBL

NetBSD 11.1 repeated the 26 packaged CGD CLI checks, login-user desktop
callbacks, and packaged X11 window at
`24dcc6e7ff7f4bd1318ccbc238a9616cd7c791dd` after enabling WAPBL on writable CGD
mounts. A container created and populated by the original `2ebae8f` package
opened with effective journaling in the new package. The old file was intact,
and a new file written in the journaled session persisted after close and
read-only reopen. Remote run `20261119T174204Z-c04be4b9` passed workspace check,
build, and package staging, with 93 regular tests passed, 0 failed and 4
ignored. The exact archive and original-guest results are in
`target/netbsd-cgd-wapbl/`.

The coordinated `a60e4bc69a3b78b8dd6e2297c2300a38f4508251` build on NetBSD
11.1 amd64 passed workspace check and build, with 93 passed, 0 failed and 4
ignored in the regular workspace test run
(`20261119T184032Z-07601f9f`). All four ignored tests passed separately.
The native package installed with dependency checks in an alternate root
and package database. Its installed executables passed the 26 CGD CLI
checks, the login-user desktop callback lifecycle, and the X11 window
probe. A `2ebae8f` image opened with WAPBL and preserved both its original
file and a new file after a read-only reopen. The byte-matched portable
stage passed eight additional FFSv2 exhaustion checks. It reached ENOSPC
after writing 124,780,544 bytes to an owned 128 MiB fixture, then accepted
new writes after freeing space and preserved the file and SQLite data
after reopening.

The portable archive SHA-256 is
`09e9eba7bf94ba6497e490346a425b519709699e0b7c67806f4f3189627ca7fa`.
The native package SHA-256 is
`c828baf34c1405255e9c6fe92d8270933f691a3fdc5262f55d38bd9816237a5a`.
An initial local-only dependency installation required libxml2 and
xmlcatmgr. The retry used exact official dependency archives, with local
copies and digests retained. Native removal left the global package list
unchanged, temporary sudo authorization was removed, and CGD/VND devices
were idle. Results and package receipts are under `target/netbsd-a60e4bc/`.

On the original NetBSD 11.1 guest, a separate test killed the process group
of the hash-verified `a60e4bc` helper during create. The helper had recorded
both native device identities in the root-owned session journal, and the
destination and its parameter file were still unpublished. Replacing the
staged image inode caused ordinary CLI unmount to refuse cleanup and leave
the mapping and journal intact. After restoring the original inode, the
same unmount removed the session and detached both devices. A normal create
retry then passed mount, write, close and reopen checks, with the file and
SQLite data preserved. All CGD and vnd devices were idle afterward. This
test covers interruption after the device identities reach the journal.
The receipt and refusal log are under `target/netbsd-cgd-interrupted/`.

KVM run `20261119T204111Z-cd081ba8` rebuilt and staged the NetBSD 11.1
amd64 package from `ca3c8456e507102a1c6e2b9194981afbd48df8db`. The native
installer verified its dependencies in an alternate package root. The
installed CLI passed all 26 CGD cases, split evenly between explicit
selection and Auto, with zero failures or skips. The installed desktop
rendered a 1280×860 X11 window with 1,179 sampled colors as UID 1300.
The probe reaped its children, and native removal left the package,
mount, CGD, vnd, and session inventories unchanged.

The native installer SHA-256 is
`961c865324cfdb33abcf7151eb1f47a954e0157d2092afb47bef0219403cb743`.
The portable archive SHA-256 is
`f3bad8e2435118700c2a3b8105b99d75cd2886e1f4cd22f4151a55502f78f8d6`.
Both archives, the install receipt, and test evidence are under
`target/netbsd-ca3c845/`.

### Clean offline NetBSD guest

A fresh NetBSD 11.1 amd64 KVM guest booted from a writable overlay of the
stock live image. The compressed upstream image matched NetBSD's published
SHA-512, and its decompressed bytes matched the local boot image. The
guest had no network interface and started with no pkgsrc installation.
Normal `pkg_add` installed the same `ca3c845` package and its local,
hash-verified dependencies from read-only media. A new FFS disk provided
space at the standard `/usr/pkg` prefix through a null mount.

The installed CLI created a CGD fixture, mounted it with WAPBL, and
preserved the synced 65,536-byte file through close and read-only Auto
reopen. It refused a wrong passphrase and a read-only write, then detached
both devices. The installed desktop rendered a 1340×860 X11 window with
1,181 sampled colors as UID 1300 and logged a color-scheme watcher warning.
Package removal and guest shutdown completed. The media verification,
console logs, package inventory and window capture are under
`target/netbsd-clean-ca3c845/`.

https://cdn.netbsd.org/pub/NetBSD/images/11.1/SHA512

### NetBSD CGD create recovery and publication

KVM run `20261120T024124Z-0c2c4071` produced the exact `69299a9` native
package. Installed on a fresh stock NetBSD 11.1 amd64 guest, it passed
13 explicit and 13 Auto CGD cases, including file and SQLite persistence.
Both routes published private parameter files. A pre-existing orphan
sidecar remained intact, and a late image collision preserved the foreign
inode and rolled back the parameter link created by Lurker.

The test killed its helper after it journaled both device identities.
Ordinary Auto `unmount` found the interrupted create by its unpublished
destination. It refused a replaced staging inode, then detached CGD and
vnd and reported the retained staged image after the test restored the
original inode. Seven retry cases at the same destination passed. NetBSD
`ktrace` recorded the parameter link, a successful `fsync` on the
destination directory descriptor, and then the image link. Package removal,
shutdown, and disk retirement completed. The installer, syscall trace,
and lifecycle receipts are under
`target/netbsd-cgd-destination/`.

KVM run `20261120T034255Z-9cd330d7` built the `de4345c` native installer.
The installed package passed 13 explicit and 13 Auto CGD cases on a
fresh stock NetBSD 11.1 amd64 guest, including synced file and SQLite
write and readback across close and reopen. After backing up and removing
the closed fixture's `.cgdparams`, ordinary mount refused to open it and
left all mount, device, and session inventories at baseline. Restoring the
byte-identical sidecar recovered both the file and SQLite data. Package
removal, guest shutdown, and disk retirement passed. The installer and
receipts are under `target/netbsd-cgd-detach-intent/`.

## Coordinated OpenBSD native package

OpenBSD 8.0 amd64 built and checked workspace source
`a60e4bc69a3b78b8dd6e2297c2300a38f4508251` in KVM run
`20261119T184025Z-fcd5f04a`. The regular workspace suite passed 109 tests,
with 17 ignored. Staging verified the installed VeraCrypt engine's inputs
and reused the byte-matched patched engine from the earlier OpenBSD proof.
The portable archive SHA-256 is
`c1060077096f969776414b0b3ec8983c5c2088fb859aeded8787b6cf071d5b01`.
The native package SHA-256 is
`6e7786f6b9418c633ea8134e1348675eee8483afa8d1d022c18ab74c8ec47fbc`.

All 147 CLI checks ran against the native package installed into an alternate
root and package database with dependency checks enabled. CRYPTO passed 26,
VeraCrypt passed 91 across AES, Serpent, Twofish, password and binary keyfile,
and RAID1C passed 30 including paired and degraded operation. Three login-user
desktop lifecycle tests, the packaged X11 window, two native ownership and
alias tests, and the login-user VeraCrypt raw-image privacy test also passed.
The privacy test verified file and SQLite WAL persistence after reopen.
Read-only attachment cases verified safe refusal. The mount, VND and
softraid inventories matched before and after testing. The run restored the
original sudo configuration and removed the package from the alternate
root. Receipts and results are under `target/openbsd-a60e4bc/`.

### OpenBSD VeraCrypt backing-file sync

On OpenBSD 8.0, a traced login-user file fsync, rename/directory fsync, and
SQLite FULL/WAL workload against the `a60e4bc` engine issued 49 ciphertext
writes without a backing-file fsync. A direct fsync on the private image
also issued no backing-file fsync. The owned vnode device rejected
`DIOCCACHESYNC` with ENOTTY. The guest kernel's vnode strategy passed
`IO_SYNC` to the backing vnode, while the FUSE write request dropped it.

The engine built with `4189679` followed all 49 ciphertext writes with a
successful fsync on the same backing descriptor in the same thread.
Explicit private-image fsync reached the ciphertext descriptor as well.
The login user could still write only through the mounted filesystem,
and the file and SQLite data matched after clean close and reopen.
These traces establish the engine's sync behavior. Media durability
depends on the lower storage layers and the result of a separate KVM cut.

Build `20261119T191549Z-661bf390` passed workspace check and build and
reported 109 tests passed and 17 ignored. The exact staged OpenBSD archive
passed 26 AES password and binary-keyfile lifecycle checks, with native
mount and device inventories restored. Its SHA-256 is
`52829c0d1b9b3d7f4d96309882d634d51fc2b96a745dad9f0ee9f2a6711d8b68`.
The engine SHA-256 is
`0c3db467e0abc33f67997a22f6deba438cfcab228a4095c063715b0a564be99e`.
Trace summaries, the archive, and lifecycle receipts are under
`target/openbsd-vc-barrier/`.

### OpenBSD synchronous FFS mount policy

`32e14e5` applies `rw,sync` to OpenBSD's writable FFS mounts for softraid
CRYPTO, RAID1C and VeraCrypt. The shared mount helper also passes the option
during new filesystem ownership initialization. OpenBSD still requires a
manual filesystem check when FFS refuses a writable reopen after an abrupt
shutdown. The [OpenBSD cut controls](vm-powerloss-testing.md#openbsd-synchronous-ffs-after-a-fresh-cut)
record successful native preen and writable recovery on verified copies.

Build `ca3c845` included the RustSec lockfile updates. KVM run
`20261119T201835Z-1a695499` passed workspace check and build and 111 workspace
tests, with 17 ignored. The native-installed CLI passed 147 lifecycle checks
across the three routes, including AES, Serpent and Twofish VeraCrypt password
and binary-keyfile cases, plus normal-user and degraded RAID1C operation. All
28 successful FFS mounts recorded the kernel's `synchronous` flag. Native
package removal and the final device and mount inventories matched the baseline.
The native package SHA-256 is
`e3587d4723b52732df4cd5302652e5a8307cafa740c0410b2dd8fcd2c4c34e55`.
The portable archive SHA-256 is
`22d42cd27e6f2e41d9a8e619ce03ebbfe54813b387792d3e589958832e657310`.

The same native archive rendered a 1280×860 X11 window through Slint/winit as
UID 1300. The Slint callback test binary from the same build used the installed
helper to create, mount, write, reopen, verify and close CRYPTO, VeraCrypt and
RAID1C fixtures under that login account. Both mounts in each lifecycle had
the kernel's `synchronous` flag. The test removed its temporary helper-specific
sudo rule, restored the original sudoers file and removed the installed package
from the alternate root. Native mount, vnode, softraid and host package
inventories matched their initial state. The rendered frame and callback
receipts are under `target/openbsd-ffs-sync/desktop-ca3c845/`.

The CRYPTO benchmark used six fresh, hash-verified copies of one FFS seed.
It interleaved three trials under each policy and checked the kernel mount
flags on every trial. The medians are specific to this OpenBSD 8.0 KVM guest,
its backing storage and these workload sizes.

| Workload | Previous FFS policy | Synchronous FFS | Time ratio |
| --- | ---: | ---: | ---: |
| 32 MiB sequential write and final fsync | 1.404 s | 2.935 s | 2.09× |
| 128 × 4 KiB writes with an fsync after each | 0.217 s | 0.281 s | 1.30× |
| 32 autocommits with SQLite FULL/WAL | 0.078 s | 0.336 s | 4.31× |

The archives, wrapper receipt, lifecycle results and measurements are under
`target/openbsd-ffs-sync/`.
### OpenBSD CRYPTO, RAID1C and VeraCrypt storage exhaustion

A stock OpenBSD 8.0 amd64 KVM copy installed the exact `e607cc7` native
package (SHA-256
`7f066d6f1cf7f62eeb61afd5a1d885e445ed9a44f1859827b19d29895fa2c0ad`).
It reused the stock fixture's dependency inventory and verified all 16
local dependency archives with the OpenBSD 8.0 package signing key.

Each backend passed eight checks using a disposable 128 MiB encrypted
container or pair of 128 MiB RAID1C members. The test wrote and fsynced a
file and committed a SQLite FULL/WAL transaction, then filled the mounted,
synchronous inner FFS with bounded writes.

| Backend | Filler size at `ENOSPC` | SQLite write error |
| --- | ---: | --- |
| softraid CRYPTO | 128,155,648 bytes | `SQLITE_FULL` |
| RAID1C | 128,155,648 bytes | `SQLITE_FULL` |
| VeraCrypt AES | 129,253,376 bytes | `SQLITE_FULL` |

Previously committed data remained readable at capacity. After deleting the
filler, each filesystem accepted a new fsynced file and SQLite commit.
Ordinary close, reopen, read-back, database integrity, and final close passed.
The rejected transaction stayed absent. The guest recorded inner and outer
capacity and restored the package, mount, vnode, softraid and session
inventories. After normal shutdown, the transient domain, isolated network,
overlay and temporary credentials were removed. Evidence is in
`target/openbsd-enospc-e607cc7/`.

## DragonFly guarded LUKS1 metadata

The DragonFly adapter in `e98bcc6b585a7e5bf481e8d3a6c6ef2caf5ab61a`
validates the supported LUKS1 layout and uses a root-private metadata
copy for native unlock. The owned KVM clone passed 91 development CLI
checks without skips across AES, Serpent and Twofish, including
password and binary-keyfile unlocks, Auto selection, wrong credentials,
busy close and retry, and read-only mounts. The build used
`60f294c6f4e19ac8d7d8a92561fe79d40aaf3cab` with a recorded overlay
matching the committed runtime sources. Workspace check and tests
passed with 92 passing and three ignored tests.

The guarded development CLI separately reopened an older AES keyfile
vault through Auto in read-only mode and recovered its recorded payload.
The session journal identified the root-owned mode 0600 metadata file,
whose bytes matched the original prefix. Final mount, vnode, mapper and
session checks found no new resources. The initial keyfile test attempt
used a test-only Python interpreter that lost its loader path during the
busy case. The corrected wrapper restored the holder process and the
full matrix passed. Evidence is in
`target/dragonfly-metadata-v1/dev-final/`.

The exact native CLI package at
`dfc7fdc33bde59682c9719823da5d934e52d27bf` installed using the stock
DragonFly package manager in the same isolated clone. Native checksum
verification passed, and all 49 files in Lurker's payload manifest
matched their installed hashes and modes. Its AES password suite passed
13 cases with no failures or skips. Deinstallation removed the CLI and
helper, restored the prior package inventory, and left no owned mounts,
mapper devices, vnodes or session files. Package SHA-256
`c01c47cb9ae126bf6f132776b5b22b28baf66c085bbe596cc05d11e2bcdd6b33`.
Evidence is in `target/dragonfly-metadata-v1/native-final/`.

The full native CLI and desktop package at the same commit was wrapped and
installed on a separate KVM copy of the stock DragonFly 6.4.2 test image.
The wrapper matched all 645 payload entries. Native installed checksum
verification passed, and the installed desktop rendered a 1280 × 860 X11
window. Its login-user callbacks created, mounted, wrote and reopened a
guarded LUKS1 vault through the installed administrator-owned helper.
The installed CLI separately passed all 13 AES lifecycle checks.

The original build guest's X11 tools could not execute. The stock-image
copy provided working native tools. The first installed CLI attempt used a
fixture path longer than DragonFly's native mount identity allows. The
shorter path passed with the same package. The sudo policy, package
inventory, mounts, vnodes, mappers and sessions returned to baseline.
The native package SHA-256 is
`640f39cabbaa89a2caf8e6883c4390d4f792c0d436f6733f01a97c6aea0c0ff7`.
Evidence is under `target/goal-validation/dragonfly-native-maintenance/`.

The DragonFly close path now records detach intent for both the private
metadata vnode and the vnode holding the original payload. The
`29d36e7` KVM regression retried close after a real detach with a stale
journal entry. It verified that cleanup requires the recorded intent and
preserves unrelated live attachments. Workspace check passed, with 92
tests passing and four ignored. The root-only detach regression passed
when selected explicitly.

The exact native `29d36e7` CLI package passed checksum verification and
all 13 installed AES Auto lifecycle checks in an alternate installation
root. The harness used the guest's base libraries and a temporary SQLite
module from checksum-verified DragonFly packages. Package removal
restored the native mount, mapper, vnode, and session inventories.
The package SHA-256 is
`4826e6be6f4adcc2ca29dd850d769a25c2c5ff6fa35608f2e5425d37d63fe948`.
Evidence is under `target/goal-validation/dragonfly-source-detach/`.

### DragonFly vnode discovery and native timeout

On DragonFly 6.4.2 KVM, the root-only regression attached two vnodes,
detached the lower unit, and found the private metadata file on the
remaining higher unit by its kernel device and inode. It also exercised
close with a stale journal entry and verified that an unrelated live
vnode was preserved. The change is in `8f07cae`.

The `dc458b6` regression exercised SIGINT cleanup and the SIGKILL
fallback against native shell children. The production path gives
`luksFormat` and `luksOpen` up to five seconds for native cleanup after
the usual timeout. DragonFly workspace checks passed with 93 tests
passing and four ignored. The two focused checks also passed on the
root-run test binary. The same source passed core checks and tests on
FreeBSD, NetBSD, and OpenBSD in run `20261120T005444Z-4603539b`.

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/contrib/cryptsetup/luks/keyencryption.c

The exact `dc458b6` CLI package passed all 91 installed lifecycle checks
across AES, Serpent, and Twofish with passwords and binary keyfiles,
including an Auto AES run. Each suite created a vault, wrote application
data, and reopened it to verify the data. The first keyfile invocation
stopped at a missing harness import. The remaining suites ran after
adding the harness module and reverifying native checksums on the same
installation. Package removal restored the mount, vnode, mapper, and
session inventories. The package SHA-256 is
`18ad430a0bd62516cff531fe1256952073c0fce87e28de538102862604b4515c`.
Receipts are under `target/goal-validation/dragonfly-graceful-timeout/`.

### DragonFly payload vnode reservation

In `8143d6c`, both create and mount save the native payload vnode's
unit and devfs identity before attaching the encrypted file. Native
attach uses the same descriptor. A root KVM regression left a real
attachment with only the reservation in the journal and then recovered
it. The regression also proved that cleanup preserves a later attachment
of the same file on the reused unit. Workspace checks again passed with
93 tests passing and four ignored. Core checks also passed on FreeBSD,
NetBSD, and OpenBSD. FreeBSD's first compiler-cache preflight needed a
retry with the measured warm-cache requirement.

The exact native CLI package passed 91 installed checks across the three
ciphers and both credential types, including Auto AES. Deinstallation
restored all mount, mapper, vnode, and session inventories. Package
SHA-256 is `09f2389e558367704c435965476f7a880c8836ff9bb4bdeb4891785c26ca0341`.

A separate native control observed that an empty clone remains after its
descriptor is closed and returns `ENXIO` on direct detach. In the same
control, an owned empty clone provisioned with one anonymous page could
be detached and removed. Both tests restored the native inventories.
Evidence is under `target/goal-validation/dragonfly-vnode-reservation/`.

`421314c` applies the one-page release only to the recorded devfs
identity. A root test held the clone open long enough for native detach
to report it busy, then retried from the saved release intent. A separate
anonymous attachment without that intent stayed attached. An
interruption before the first reservation save can still leave an empty
clone. The kernel has yet to associate any ciphertext with that clone.

DragonFly workspace checks passed with 93 tests passing and four
ignored. The focused root test passed and restored the native inventory.
The exact `421314c` installed package passed all 13 AES Auto checks and
restored the prior inventory on removal. Its SHA-256 is
`8dfb0c286d39b4b8d870faea6b637f82b8f7e0bcdb181325adc6122d4c22c768`.
Receipts are under `target/goal-validation/dragonfly-idle-recovery/`.
