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

On the Debian KVM guest, Firefox ESR 153.7.0 passed both browser-profile
checks for gocryptfs, CryFS, Cryptomator, and AES VeraCrypt in the
unprivileged route. Each suite passed 15 checks and ended with its mount
closed. The packaged CLI source was
`2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1` and the harness was
`ed94676`. Collected results and hashes are under
`target/browser-profile-qualification/`.

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
