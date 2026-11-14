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
