# Directory backend integration

The Rust `folder` adapter shares initialization, credentials, duplicate locks,
process supervision and session persistence. Native tools implement encryption;
Lurker does not implement their cryptographic primitives. Directory workflows run
as the login user and never elevate. Only approved backend/OS combinations run.

Private executables live in `libexec/lurker/backends` (or macOS app Helpers).
`gocryptfs` and `cryfs` are direct children; Cryptomator keeps its entire
`cryptomator/` image, including its private Java runtime. Explicit unprivileged
development builds can set the absolute `LURKER_BACKEND_DIR`. There is no implicit
PATH fallback to a separately installed encryption application.

Build `python scripts/build-folder-backends.py gocryptfs cryfs`; Go, a C++
compiler/CMake, uv and native FUSE development files are build prerequisites.
The script obtains pinned upstream source and Conan recipe revisions, keeps
dependency identities/licenses, writes binary hashes and native dependency
inspection to individual manifests, and retains verbose logs under
`target/folder-build`. Go executables use the upstream Go-only crypto route;
Conan dependencies are static, but CryFS still links platform libraries and
the native FUSE ABI. The pinned CryFS **1.0.3 uses FUSE 2**, unlike newer commits
on its moving `release/1.0` branch. Qualify the actual binary's dependencies on
each target. See `cryptomator/README.md` for the private Java build.

FreeBSD's Go FUSE route needs the base `fusefs` kernel module, access to
`/dev/fuse`, `/sbin/mount_fusefs` and native permission for user mounts. Its CLI
suite passed on the dedicated guest with temporary `vfs.usermount=1` and
`/dev/fuse` restricted to the test user; both were restored afterwards. Lurker
does not change sysctl/devfs policy, grant device access or elevate this backend.
An administrator must prepare that host prerequisite for ordinary user sessions.

Creation refuses existing targets, sizes, force, keyfiles and alternate cipher
families. A private target directory is reserved with atomic mkdir, initialized
in an internal staging directory, synced and published with the native vault
configuration last. Failure retains the new incomplete directory for inspection;
it never overwrites a preexisting vault. CryFS initialization temporarily mounts
the staged vault and waits for its foreground daemon to finish flushing before
publication. Do not signal a CryFS or gocryptfs daemon immediately after native
unmount: native detach may precede userspace destructor/metadata completion.

Per-user `/tmp/lurker-folders-UID` state contains no credentials. It records source
device/inode, mount identity, process birth identity and readonly mode. Busy or
uncertain sessions remain recoverable. Changed mount identities refuse detach;
reused PIDs refuse signalling. Mount timeout does not kill an uncertain mounted
filesystem. If a mount appeared after its readiness timeout without a confirmed
identity, manual native inspection is required. No forced/lazy unmount is used.
Boot-time removal of `/tmp` state intentionally prevents speculative recovery of
unrecognized mounts. Concurrent access from another machine or outside Lurker is
not coordinated by this local registry.

`scripts/test-folder-backends.py /path/to/lurker BACKEND...` runs the actual CLI
with PTY credential prompts in private `/tmp` directories. It tests initialization,
existing targets, bad credentials, aliased duplicate sources, Unicode/binary
fsync, SQLite WAL, rename, busy close, durable reopen and readonly. Successful
fixtures are removed; failed ones remain and results contain concise references.
Run runtime tests only on dedicated test hosts, never the active endeavour host.
These tests do not claim crash/ENOSPC qualification or substitute for native tests
on each selected platform. CryFS retains its upstream power-loss/full-storage
limitations. macOS and FreeBSD FUSE and application semantics need independent
qualification.

Upstream references:

- https://github.com/rfjakob/gocryptfs/tree/v2.7.0
- https://github.com/rfjakob/gocryptfs/blob/v2.7.0/Documentation/CLI_ABI.md
- https://github.com/cryfs/cryfs/tree/1.0.3
- https://github.com/cryfs/cryfs/blob/1.0.3/README.md
