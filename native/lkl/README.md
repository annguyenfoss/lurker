# External LKL guardian wire contract

`guardian-protocol.h` is a host-only, syscall-free protocol-2 codec shared by
native OS bindings. The logical C struct is never serialized directly. Each
frame is exactly 512 big-endian bytes, starts with `LFG2`, and has version 2.
Unknown fields, versions, reserved bytes, kinds, flags and stage bits fail
closed. `tests/golden-v2.json` and `released-clean-v2.hex` specify a portable
example for native C and a future Rust consumer. The example is structural;
it cannot authorize anything by itself.

The guardian owns the actual spawned child and its `waitpid` result, a custody
reference to the same open source description used by the worker, and the
exclusive session status lock. It spawns the verified executable before any
LKL/FUSE threads start. Native wrappers validate inherited descriptor identity,
close unrelated descriptors, authenticate public socket peers, and bind PID
birth, boot, source/status inode and exact component identity. Startup HELLO or
QUERY can discover incomplete worker metadata; it never authorizes close.
The four native identity words become immutable after READY and must be checked
separately from `lfg_same_binding`, along with the echoed fresh request nonce.
Native wrappers own bounded stream framing/deadlines and OS credential messages.

A successful live REPLY requires worker DRAINED, actual reaped exit 0, native
mount absence, all required drain stages, no permanent errors, and successful
guardian source barrier/close plus status sync. The status file remains discovery
and recovery evidence only. Its visible contents can look clean even after a
failed fsync and worker exit 1. Neither process disappearance nor decoding that
file can substitute for live guardian authority. A missing guardian refuses
success. Errors retain custody and never authorize outer storage detach.

RELEASED_CLEAN stays live and replayable until the consumer confirms outer
storage detach and sends RETIRE. A lost response does not reverse completion or
lose the child wait result; a fresh correlated request may retry. RETIRED may
only follow a previously released-clean session. Native code must validate state
transitions, peer authority and actual results; the pure codec validates frame
shape and internal consistency, not history. Protocol-1 file-only drivers do not
satisfy this contract, and guarded core sessions must fence legacy helpers before
setup. Core admission remains separate from the external driver implementation.

Run the pure codec check natively on each target:

```
cc -std=c11 -Wall -Wextra -Werror -O2 native/lkl/tests/guardian-codec-control.c -o /private/owned/path/guardian-codec-control
/private/owned/path/guardian-codec-control native/lkl/tests/released-clean-v2.hex native/lkl/tests/char-provider-v2.hex
```

This check exercises byte portability, rejected lengths/reserved values/ranges,
startup incompleteness and structural error states. It does not test live peer
credentials, actual child supervision, backing custody or filesystem I/O.

Source kinds are REGULAR=1 and CHAR_PROVIDER=2. The latter carries actual held
native media capacity in source_size (not character-node st_size), nonzero rdev,
and power-of-two logical/physical sector sizes at offsets 304/308. Physical sector 0 means explicitly UNKNOWN: an OS binding must explicitly
permit and persist that policy; bindings must not substitute stripe size.
Native callbacks establish geometry truth and source/session association. A
structurally valid source kind never grants an OS adapter permission to use it;
macOS phase 2 continues to admit regular files only. Reserved bytes are 14..15
and 312..511. Regular frames require rdev and both sector fields to be zero.

SIGNAL_POLICY_OK means the documented native binding's observed signal policy
succeeded. Unknown signal outcomes or errors never satisfy it. macOS retains a
permanent signal error; FreeBSD's previous graceful signal-requested normal close
is a different policy. Historical results keep their original semantics.

BACKEND_RELEASE_OK is an outcome, not a requirement to call the same native API
on every OS. macOS retains its checked explicit disk removal. Another binding
may establish release through its documented checked inner unmount, halt 0 and
source-close path, combined with actual child exit 0 and guardian final barrier
and close. Record that exact native proof; never synthesize an unexecuted API
result merely to fill a common bit.
