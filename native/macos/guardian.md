# Native macOS guardian primitives

`lkl-guardian.h` implements host-only process, custody, status and live socket
primitives using the common `native/lkl/guardian-protocol.h`. It contains no
embedded kernel or filesystem code. It is not yet integrated into the external
FUSE driver or Lurker. No driver installation, product dispatch or mapped-device
support follows from these controls.

The parent records `waitpid` for its exact spawned child. A worker DRAINED
message or clean-looking file cannot override a nonzero wait result. Source and
status descriptors bind regular-file ownership, mode, single link and inode.
Parent and worker share the source open-file description; its flock preserves
custody, but the same-OFD status lock never proves worker termination. The parent
holds its status lock and checks a separate writer-reference close after status
barriers. A failed status sync/close can leave visible clean bytes: live in-memory
failure state remains authoritative and permanently prevents positive replies.
A failed final source close has an uncertain descriptor outcome; it still never
authorizes outer-storage detach or recovery as clean.

The native binding supplies a fresh captured-mount-absence result after ordinary
OS unmount. Busy is retryable; a negative identity/check error is latched. Release
also requires READY, DRAINED, all checks, actual child exit 0 and successful parent
source/status barriers and closes. The kernel peer PID/UID and process birth are
verified before public RPC. The response echoes a fresh request nonce and exact
source/status/component/native identities; guardian birth is checked again while
it remains live. Querying socket peer credentials after the accepted connection
closes is not required: Darwin may reject that lookup even while the verified
process is alive. Missing guardian and wrong nonce fail. Released-clean replies
can be retried after a lost response; RETIRE is reserved for a caller that has
already confirmed outer-storage detach.

The isolated `tests/lkl-guardian-control.c` native control spawns the same test
executable with explicit inherited descriptors before any worker threads. It
verifies actual wait, shared-source flock, exclusion of a deliberately non-CLOEXEC
sentinel FD, lost live reply/retry, wrong nonce, absent guardian and seven normal
or injected failure outcomes. It links only native system libraries, with no LKL,
FUSE, filesystem mount or device operation. The visible-clean/exit-1 case writes a
structurally valid final frame, injects its status-fsync EIO, refuses the attempted
error overwrite and actually exits 1; the parent still refuses success.

Remaining gates include the long-lived supervisor loop and startup context,
worker/driver integration, exact executable/source identity capture, native mount
identity callbacks, production caller/core/session capability fencing and the
three-filesystem I/O/halt controls. The embedded-LKL dependency is separately held
under the kernel-patch scope; plain POSIX primitive results do not clear that hold.

Run only on a native Mac as the login user, using a new owned 0700 fixture root
whose UNIX socket paths fit Darwin's `sun_path` bound:

```
clang -std=c11 -Wall -Wextra -Werror -O2 -I native/lkl native/macos/tests/lkl-guardian-control.c -o /private/owned/guardian-control
/private/owned/guardian-control /private/owned/new-fixtures
```
