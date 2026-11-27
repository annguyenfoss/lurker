# Targeted Linux mounted namespace supplement

The full original payload matrices keep their recorded 576/678/e927 identities.
Their common payload covers creation, synchronization and append/reopen hashes,
but does not by itself prove plaintext rename/deletion. Existing backend atomic
rename/truncate conformance and UFS2 no-clobber controls retain their narrower
scope; a prerequisite control is not a directory-engine product pass.

Use `scripts/test-mounted-namespace.py` on a normally opened, independently
qualified mount as the intended payload user:

```sh
python scripts/test-mounted-namespace.py /exact/mounted/root \
  --expect-mount-id ID --expect-source SOURCE --expect-fstype TYPE \
  --expect-euid 1300 --receipt /outside/backing/namespace.json \
  --protect-tree 'Lurker payload' --timeout 45
```

Supply the exact current native mount record, not a guessed source or generic
requested-format label. UFS-family/FUSE labels still need the existing separate
superblock/driver proof. `--protect-tree` may be repeated for known caller-owned
payload directories; it checks their file bytes and ownership/modes before and
after. Keep the receipt and controller logs outside any backing that may later
be reformatted. The probe holds and rechecks the actual mount-root descriptor,
uses only unique dirfd-relative names, records actual caller credentials and
checks both native read/write option fields before and after each action.

The eleven actions create/write/fsync/read/hash files and directories, rename a
file and directory, move a file across parents, unlink both files and remove the
two owned directories. Parent directories are synced after changes. Successful
cleanup removes only the newly generated namespace; protected existing data is
unchanged. Unexpected failures retain their namespace and fixture. A timeout is
a failure even if its worker terminates; an unknown/still-running worker and
mount require review before any cleanup or next destructive operation.

Maximum authorized supplement per Linux host is forty small sessions, subtracting
any existing complete same-driver/backend namespace proof: ten native filesystem
mounts distributed across LUKS1/LUKS2/VeraCrypt, plus all three directory engines
on ten native backings. Prefer safely identified retained closed fixtures with
their original factors. Do not repeat 720 complete payload lifecycles, reboot for
namespace alone, change filesystem selectors or invent kernel behavior. Preserve
every source/package/driver identity and all unexpected remnants.

Arch's initial harness qualification used managed Python 3.15.2 as UID/EUID1300:
three admission unit checks and five real native checks passed. The native
checks cover all eleven actions with protected bytes, wrong mount ID, wrong EUID,
an injected harness-only rename failure with retained file/directory contents,
and a bounded stalled worker which remains a failure after termination.
Private logs, executed controller and per-action receipts are retained under
`hosts/archlinux/build/mounted-namespace/` in the round evidence root.

The worker-exit follow-up also requires the native wait status to be a normal
zero exit; a passed-looking JSON file cannot override a nonzero exit or signal.
Four unit checks and seven actual native controls passed with this correction,
including forked workers which write a passed receipt and then exit1 or receive
SIGTERM. Those outcomes remain failures. The earlier harness/version receipts
are preserved separately rather than relabeled.
