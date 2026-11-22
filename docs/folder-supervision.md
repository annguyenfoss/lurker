# Directory backend close supervision

Directory mounts started by the CLI or desktop helper now keep a private monitor
running beside the selected backend. The frontend invokes its own executable in
an internal supervisor mode; the desktop still uses `lurker-helper`. Helper
capability `folder_supervision_protocol: 1` identifies this lifecycle.

The monitor records the engine's PID and birth identity before sending its
password over a private pipe. After the engine exits it drains both output pipes,
redacts the credential, bounds diagnostics, and atomically publishes a private
`backend-result.json` containing the actual exit code/signal. File and parent
folder fsync complete before the result is considered published. Passwords never
travel through argv, environment variables or a persisted request.

Unmount first verifies the recorded source, mount and process identities. A busy
mount or live backend preserves its active session. A normal close requires an
actual successful engine exit. The Cryptomator helper additionally emits a
`closed` acknowledgement only after both mount and CryptoFS resources close;
SIGTERM's expected JVM exit code alone cannot prove that its shutdown hook
succeeded. Both resources are attempted even when one fails.

Once a mount is detached and its backend/monitor are conclusively dead, a failed
or unavailable exit result produces a nonzero CLI/helper result. Its private
session and diagnostics move to `failed-session-*` under the same runtime folder.
They remain available for inspection while an explicit reopen can inspect the
vault. This does not claim that pending writes were recovered. Older sessions
without a supervisor cannot acquire a manufactured successful exit result.

## FreeBSD gocryptfs restart recovery

New FreeBSD folder journals record the kernel boot ID. After a reboot,
the old journal reserves its vault and mountpoint until an explicit
unmount reconciles it. If the recorded mountpoint is vacant, Lurker
archives the earlier journal and reports that it cannot confirm a clean
close. The vault can then reopen. An occupied mountpoint preserves both
the current mount and the old journal. Recovery issues no native unmount
or process signal.

FreeBSD can assign a new device number to the same filesystem after reboot.
Journals written before boot IDs were recorded still use their original
device, inode, and process checks. A legacy journal with a changed device
number requires manual recovery.

On FreeBSD 15.1-RELEASE-p6, the `14ede46` installer passed 260 lifecycle
checks and four installed-helper callback routes. Eleven fixtures were
left mounted for an orderly reboot. The GELI, VeraCrypt and PEFS fixtures
reopened with all five file digests intact. The gocryptfs fixture retained
its journal, but the legacy close command refused it after the device
number changed from 64 to 70. A later boot restored device number 64,
after which the same gocryptfs fixture reopened with all five digests intact.

The integrated `d1608f897631` installer passed 26 explicit and Auto
gocryptfs lifecycle checks plus all four installed-helper callback routes.
The test wrote and synced three PDFs and a text page, closed and reopened
the vault, then synced an append and a second text file. The guest
rebooted with the vault still mounted and reported a new kernel boot ID.
An occupied mountpoint preserved the replacement mount and old journal.
After the mountpoint was clear, explicit unmount archived the journal and
reported the unconfirmed close. The installed CLI reopened the vault,
verified all five SHA-256 digests, and closed it normally. Package removal
restored the original native resource and package inventories.

The exact package SHA-256 was
`48263178155db76bab4c360a9fb9f6709312e549ebe88739f29c8ae30d6db377`.
The native run was `20261122T163738Z-408a7eb4`. The package and runtime
receipts are under `target/goal-validation/ssh-matrix-20261122/freebsd/integrated-evidence/`.

## macOS Cryptomator underlay protection

Before mounting, core opens the original empty directory without following
symlinks, refuses extended ACLs, journals its identity/mode and sets mode 000.
The supervisor inherits that descriptor and retains it after backend exit.
Unexpected FSKit detach therefore leaves a protected underlay, rather than a
writable plaintext directory. The private Java backend and desktop helper must
advertise fence protocol 1; macOS Auto requests are gated too.

Explicit unmount/recovery restores the original mode only through that held
descriptor after checking the source, mount, process, directory identity, ACLs
and emptiness. Replacement or changed permissions leave the directory untouched.
Wrong passwords retain the fence until explicit recovery; recovery still reports
the backend's failed exit. A durable matching acknowledgement permits retry after
an interrupted frontend. Losing the guardian/descriptor leaves mode 000 and
requires manual recovery; there is no path-based chmod fallback.

If an ordinary unmount of a fenced FSKit volume briefly reports that the
resource is busy, Lurker rechecks the recorded source and mount identities
and tries once more after 150 milliseconds. A volume that remains busy
keeps its mount and session so open consumers can finish.

Native Buran evidence: source `2b56da973f` passes 26 explicit/Auto lifecycle
checks; `6aeed62b69` passes five supervised crash/recovery checks using a test-only
wrapper that SIGKILLs its own unreaped JVM child. Plaintext writes are denied,
replacement is refused, original permissions restore explicitly, committed file
and SQLite WAL data recover with integrity, and normal close succeeds. Source
`cce7249719` passes 21 folder tests, including actual guardian descriptor
inheritance/SIGKILL. This is development evidence; final packaged tests remain
separate. Harness: `scripts/test-cryptomator-fence.py`.

## Native fault evidence

CryFS full-storage runtime tests ran on Arch, as the ordinary test user, using a separate
8 MiB tmpfs mounted beneath private `/tmp/lurker-folder-fault-*` fixtures. The
harness refuses ordinary backing filesystems and tmpfs mounts larger than
16 MiB. It changes no host quotas, forces no unmounts, performs no repair and
retains fixtures/results/logs.

- Fixture `20261114-01`: old CryFS FUSE translation returned `EIO` for a real
  backing `ENOSPC`. Previously committed file/SQLite data survived recovery.
- Fixture `20261114-03`: an unsynced directory rename followed by close while
  full aborted CryFS in its dirty-block destructor. The old frontend falsely
  reported `Directory vault closed.` and discarded the session.
- Fixture `20261114-04`: corrected FUSE errno translation returned `ENOSPC` to
  an existing-file write/fsync. Restoring only the fixture's filler space allowed
  clean close/reopen, and the committed file and SQLite integrity/row matched.
- Fixture `20261114-05`: close while full still triggered the upstream destructor
  abort, but the supervised frontend reported failure and retained the actual
  SIGABRT result. Explicit reopen preserved the previous committed file and
  SQLite row/integrity. The harness labels this an **unclean exit reported with
  previous commits preserved**, separately from a clean recovery.

- Fixture `20261114-06`: the same mounted write/fsync returned actual `EDQUOT`
  from a new private 8 MiB tmpfs with a per-instance 4 MiB user hard limit. The
  filesystem still had 4 MiB free when quota stopped writes. Removing only its
  filler allowed clean close/reopen with the committed file and SQLite row intact.
  The host /tmp quota and system configuration were untouched. The quota default
  existed only on this disposable mount; see the kernel's tmpfs quota options:
  https://docs.kernel.org/filesystems/tmpfs.html

- Fixture `20261114-07`: close while the private quota was exhausted produced
  the same explicit, retained SIGABRT failure and preserved the prior committed
  file/SQLite data after reopen. This is safe failure reporting, not a clean flush.

All seven finite backing mounts were ordinarily unmounted after their complete
ciphertext trees were copied into each fixture's `retained-backing/`. Results,
backend logs and failed-session exit evidence remain retained.

CryFS still lacks a fully recoverable mounted full-storage shutdown path when
pending directory metadata reaches a throwing destructor. Supervision exposes
that failure; it does not suppress the exception or turn it into a clean close.
Arbitrary power-loss safety and complete full-storage recovery remain
unqualified. See `native/folder/cryfs/README.md` for the persistence patch and its
other native regressions.

### Linux close with exhausted backing storage

The Arch KVM guest repeated four ordinary finite-storage cases with native
package source `14ede46fb401578f0e8a4ee9b51e72ce17c0ad04` and package SHA-256
`3e8782b5cc1da4f242039dfb185962fef8510414ac74a98d0c04fbdad1bd9436`.
The engine SHA-256 was
`45e4b770b8cd4eb0a5c46cf15f5db9b6daf19372127bcd8c165a25d696713bbc`.

| Condition | Write test followed by space recovery | Close while an unsynced rename was pending |
| --- | --- | --- |
| 8 MiB tmpfs full | File write or fsync returned `ENOSPC`. Removing the test filler allowed a clean close and reopen. | CLI close returned 1 and retained the backend's SIGABRT and `ENOSPC` diagnostic. |
| 4 MiB user quota on 8 MiB tmpfs | File write or fsync returned `EDQUOT` with 4 MiB still free. Removing the filler allowed a clean close and reopen. | CLI close returned 1 and retained the backend's SIGABRT and `EDQUOT` diagnostic. |

Every reopen recovered the previously committed file and SQLite WAL-backed
record and passed SQLite integrity checking. Separate read-only mounts of the
two failed-close fixtures found the original file name and bytes intact and
the uncommitted new name absent. These cases leave the filesystem detached
after the failed close. Restoring backing space before a new unlock permits
normal inspection. A second unmount of the detached session cannot flush its
lost changes. The fixture results, redacted exit records and ciphertext copies
remain under `/tmp/lurker-folder-fault-14ede46.*` on the Arch guest. All owned
mounts were closed.

In pinned CryFS, the FUSE destroy callback releases cached directory blobs
after the native mount detaches. The directory destructor serializes dirty
entries through a data tree that can resize and write several encrypted blocks.
The Linux patch makes each individual block replacement atomic. A cross-directory
rename can still update two directory blobs and a parent pointer and delete an
overwritten blob. Those changes have no shared transaction or rollback record.
A destructor catch would avoid the abort without establishing which parts of
that mutation reached disk. Safe close recovery needs an explicit commit or
rollback path for those writes before reporting success. Keep the supervised
failure in the meantime.

## Cryptomator append with a full backing store

The exact Debian `14ede46` package mounted a Cryptomator vault on a
dedicated 12 MiB tmpfs. A 2 KiB file was written and synced before the
fixture filled the backing store. The test then appended 11 KiB with
zero or one free 4 KiB page. Both runs acknowledged `fsync` even though
the encrypted file grew by only 1,952 or 6,048 bytes. Reading the original
2 KiB returned `EIO` after clean close and reopen in both runs. An 80 KiB
append returned `EIO` at `fsync` and also left the original prefix unreadable.
After space was freed and the vault reopened, a separate new file synced
successfully in all three cases.

A comparison on the same guest reused the installed helper and launcher in
an isolated copy with CryptoFS 2.12.0 and its resolved runtime dependencies.
The official CryptoFS JAR matched the release checksum. The candidate returned
`EIO` at `fsync` for all three appends, including the two short writes that
previously reported success. The original file prefix still returned `EIO`
after reopen. The upgrade alone does not meet the preservation requirement
and was not promoted. The existing adapter also leaves `ENOSPC` propagation
unresolved. See the CryptoFS short-write fix at
https://github.com/cryptomator/cryptofs/pull/370

The fixtures closed normally and the guest mount inventory was restored.
Per-case data is in `target/goal-validation/cryptomator-short-write-candidate/`.

The installed CLI regression is available through `scripts/test-folder-fault.py`.
Prepare a fresh private `/tmp/lurker-folder-fault-*` fixture with an empty
`backing` tmpfs of at most 16 MiB, owned by the ordinary test user. Run the
script on the dedicated Linux KVM guest as that user, with the installed CLI,
fixture, `--commit` matching the installed source, `--type` selecting the
backend, `--fault append`, and `--runtime-tests`. Add `--leave-one-block`
to exercise a partial write with one filesystem block still available.
Each condition requires a separate fresh fixture.

This regression commits a 2 KiB prefix, appends 11 KiB under storage pressure,
checks the prefix immediately, closes while full, frees the fixture's filler,
and verifies the prefix after reopening. Independent synced files and SQLite
commits are also checked after restoring space. The result records bytes
accepted, the failing operation, `fsync` acknowledgment, and both prefix reads.
Validation failures are reported after the normal close and reopen checks.

One six-case Debian batch reused the exact `14ede46` installer. gocryptfs
passed both conditions with `ENOSPC` and readable prefixes. Cryptomator
acknowledged both writes and `fsync` calls but returned `EIO` when reading
the prefix, including after reopening. CryFS reported `ENOSPC` during the
append and immediate prefix read, then an explicit failed close. Its original
prefix was readable after space was freed and the vault reopened. Both CryFS
cases failed the requirement for readable committed data at capacity and
clean close. The guest's mounts and loops returned to baseline and the
package was removed. Receipts are in
`target/goal-validation/debian-folder-append-harness-14ede46/`.

The fault contract permits a failed close with the real backend result
retained, followed by exact recovery of the original prefix after reopening.
An immediate read error at capacity is recorded separately. The earlier
append harness imposed the additional requirements of readable data at
capacity and clean close while full. Those requirements were removed to
match the existing full-storage failure contract.

The corrected six-case batch passed both gocryptfs and both CryFS cases on
the same frozen installer. The CryFS cases retained the actual nonzero
backend results and recovered the exact prefix after reopening. Both
Cryptomator cases still failed because they acknowledged `fsync` and lost
the readable prefix after reopening. Results and retained close evidence
are in `target/goal-validation/debian-append-contract-14ede46/`.

A later Debian replay paired the verified `14ede46` CLI with an isolated
Cryptomator journal prototype. On both the zero-block and one-block fixtures,
the appending write returned `ENOSPC` before accepting bytes. The original
prefix remained readable at capacity and after a clean close and reopen.
New file and SQLite commits made after freeing space also survived reopen.
Both fixtures were retired and the mount inventory matched baseline.

The replay used an extracted CLI with an experimental component and a
fixture-local manifest. The installed package remains affected. The journal
prototype still needs source integration and packaged qualification.
Exact CLI and patch-JAR hashes and result receipts are in
`target/goal-validation/ssh-matrix-20261122/cryptomator/cli-draft-evidence/`.

## Linux process exit during close

An installed Gentoo `c21e0cc` run detached a read-only CryFS mount and
then reported `No such process` while checking the exiting backend. A
retry with the same native package passed all 78 login-user checks.
Linux can return `ESRCH` when a process is reaped between opening and
reading its procfs `stat` file.

The close path recognizes `ESRCH` as process exit. It continues to
require the recorded mount identity and supervisor exit result, and
reports permission or other inspection errors. A Linux regression test
opens a live child process's stat file, reaps the child and verifies
the kernel's `ESRCH` result. The Gentoo report and regression receipts
are under `target/linux-coordinated-c21e0cc/` and
`target/goal-validation/folder-process-exit/`.

https://github.com/torvalds/linux/blob/v6.12/fs/proc/base.c
