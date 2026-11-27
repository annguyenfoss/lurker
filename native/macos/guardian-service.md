# Native guardian service control

Purpose: exercise a long-lived user-only guardian with a pre-thread spawned dummy
worker. `guardian-service.h` uses the unchanged protocol/runtime from `1f7a42d`.
It contains no LKL, FUSE, mount, encryption-provider or device operation and is not
wired into Lurker. No installation or product admission follows from this work.

The production inheritance boundary is worker channel FD 3 and exact source OFD
4. Guardian status stays CLOEXEC and exclusive to the guardian. Inputs are first
duplicated above the fixed range; Darwin `POSIX_SPAWN_CLOEXEC_DEFAULT` then retains
only explicit 0–4 actions. stdin is `/dev/null`; stdout/stderr retain owned logs.
Workers immediately mark 3/4 CLOEXEC before any possible helper execution. The
source is an owned, private, single-link writable regular file with an exclusive
claim. The candidate executable is held, hashed and checked around spawn. This
prototype's self check admits a protected login-user-owned candidate; trusted
administrator-owned installation policy remains a separate review gate.

Spawn returns READY only after checked temporary-FD closes and executable
recheck. A post-spawn failure returns RETAINED with the actual owned child and
channel, requiring the caller to preserve custody and reap. No uncertain close is
retried. The guardian alone persists progress/status and performs final barriers.
Worker messages are drained through EOF before normal owned-child wait, so queued
final progress survives child exit. Public streams bind native peer UID/PID/birth
and the complete session; native callbacks retain signal/backend policy.

Each service step is bounded. Startup, partial worker frames and post-CLOSE
completion have explicit deadlines. Retryable native busy does not start a close
timeout. A partial public frame times out only that connection. Lost replies do
not poison released-clean state; fresh requests can retry. Errors remain live and
retain custody. The library performs no forced worker cleanup or error retirement.

Native command (login user, new owned 0700 fixtures with short socket paths):

```
clang -std=c11 -Wall -Wextra -Werror -O2 -I native/lkl -I native/macos native/macos/tests/guardian-service-control.c -o /private/owned/service-control
/private/owned/service-control /private/owned/new-fixtures
```

Evidence root: `target/goal-validation/native-fresh-20261127/hosts/macos/validation/guardian-service-1/`.
The retained controller records exact inputs, commands, compiler, binary and log
hashes for each attempt; raw logs remain separate.

| Attempt | Command/log reference | Result |
| --- | --- | --- |
| 1 | `retained/guardian-service-1/attempt-1/compile.log` | Strict compile failed: missing `sys/random.h` declaration for `getentropy`; no service ran. |
| 2 | `retained/guardian-service-1/attempt-2/control.log` | 446 checks, nine cases passed; setup temporary-close handling then strengthened before freeze. |
| 3 | `retained/guardian-service-1/attempt-3/control.log` | 477 checks, ten cases passed; native system-library linkage only. |

Final cases: normal and busy retry completed with actual worker exit 0; three
lost-reply send errors retried cleanly; partial public input was refused without
poisoning the session. Startup hang, partial worker hang/EOF and completion hang
remained failures after the dummy later exited 0. A test controller forged a
valid-looking clean status and injected refusal of later error persistence; actual
worker exit 1 still produced a live error. Injected post-spawn source-duplicate
close EIO retained the owned child/source and refused completion. Actual original
source FD 3 and guardian status FD 4 exercised the dup2 alias boundary; workers
verified no inherited status FD 5 or non-CLOEXEC sentinel 100.

All owned children were reaped. Four cases used normal completion. Six fault
fixtures used explicitly labeled recovery after the dummy became terminal;
recovery is not a normal-close or durability pass. Only private 4 KiB regular
files and sockets were touched. No signals, reboot, USB writes or host changes
were needed. Mount identity values were control inputs, not native mount proof.
Remaining gates are production supervisor/driver integration, component/install
trust, actual native identity callbacks, provider lifecycle and core capability
fencing; platform signal policies must stay distinct.
