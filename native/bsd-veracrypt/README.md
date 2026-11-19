# Private BSD VeraCrypt build

`python scripts/build-bsd-backends.py` runs natively as a normal user on amd64
FreeBSD 14+ or OpenBSD 7.8+. It builds pinned upstream VeraCrypt and a static
wxWidgets **base** library, then stages `lurker-veracrypt-bsd`, private shared
libraries, notices and `bsd-veracrypt-source.json` in `target/release/backends`.
It never installs packages, loads kernel modules, elevates or tests storage.

The current source pin is official VeraCrypt 1.26.31 (`7afc449f2bc6`). Its Unix
fixes cover inherited service channels/files/devices, subprocess hangs and
mount-point ownership/validation. The reviewed BSD patches still apply without
changing encryption profiles. Engine source caches are revision-specific; an
older static wx cache is reused only when its artifact hashes, compiler/OS/wx
configuration and verified upstream build recipe still match. Its build prefix
stays in place through `WX_BUILD_DIR`.

Build prerequisites are a native C/C++ compiler, GNU make, Python 3.11+, yasm,
pkg-config, readelf/ldd/strip, FUSE2 headers and PC/SC headers. FreeBSD package
names are `gmake yasm pkgconf fusefs-libs pcsc-lite libiconv`; OpenBSD has FUSE
in base and offers `gmake yasm pcsc-lite`. Source archives and original source
inputs are checked against pinned SHA-256 values before cache reuse. Use
`--offline` after archives have been cached; build logs stay beside the output.
The reviewed BSD-only `auxiliary-path.patch` adds the private auxiliary mount
directory to verbose volume metadata and uses a fresh native CSPRNG 128-bit
nonce in each BSD auxiliary mount pathname. Atomic root-private directory creation
under fixed `/tmp` never reuses or removes a collision. Recovery proof v2 combines
that generation name with source/vnode identity, native mount fsid and owner UID0;
OpenBSD can recycle fsid and vnode numbers immediately. A present device with the
saved auxiliary generation absent is rejected. Legacy numeric helpers/proof v1
cannot establish the new recovery contract. No encryption or format primitive
is changed. The shared `native/veracrypt/utf8-console.patch`
fixes libc++ wide-stream listing output on BSD/macOS so a Unicode path cannot
truncate the identity record.

OpenBSD also applies `openbsd-vnd-io.patch`. The vnode disk retains the root
credentials that opened the backing image, while subsequent FUSE requests can
carry the application process's identity. The engine accepts those reads and
writes only on the kernel handle returned by an authorized open of the private
image. Directory traversal, opening the image and control-file operations keep
their existing checks. OpenBSD enforces permissions on the root-owned auxiliary
directory and image. The build and package manifests require the patch digest
for OpenBSD.

The OpenBSD vnode device requests synchronous backing writes, but its FUSE
write messages do not carry that request. Each authorized data-node write now
syncs the encrypted backing file before replying. The explicit FUSE fsync
callback also syncs that file under the existing data-node access check.
This preserves the vnode device's write ordering through the private engine.
The outer filesystem and storage device still determine media durability.

The staged executable uses upstream's text/stdin interface. It is intended for
Lurker's trusted, administrator-controlled private backend directory; a normal
user's build output must never be automatically elevated. FreeBSD private
libraries use `$ORIGIN/lib`; OpenBSD runtime libraries must come from base.
Before any backend execution, including version discovery, Lurker verifies the
bounded manifest, executable and private-library hashes, complete library names,
and administrator ownership of every companion path. Symlinks and writable or
unlisted loader files fail closed. Development installations must retain the
complete producer layout, including `bsd-veracrypt-source.json` and `licenses`;
copying only the executable and `lib` is insufficient. Installed packages use
`share/lurker/provenance/veracrypt-bsd/bsd-veracrypt-source.json` and
`share/lurker/licenses/veracrypt-bsd`, as emitted by `scripts/stage-unix.py`.
No separately installed VeraCrypt is required. Native FUSE/kernel availability
and authorization remain host prerequisites. Smartcard/PKCS#11 integration is
outside Lurker's selected credential interface.

This builder alone does not qualify a platform/backend. The CLI adapter must
validate file identity, own its sessions, format a qualified native inner
filesystem, reject unsupported operations and preserve busy/recoverable
resources. Physical devices, forced unmount and whole-machine tests are not
part of this recipe. Runtime tests belong in isolated `/tmp/lurker-*` storage
on the designated BSD guests.

`scripts/test-bsd-veracrypt.py --backend /absolute/path/lurker-veracrypt-bsd`
is an opt-in **engine-only** qualification suite for the designated guests.
It runs as root, creates unique `/tmp` files, exercises AES/Serpent/Twofish,
native filesystem writes and fsync/reopen, incorrect credentials and FreeBSD
read-only mappings. OpenBSD read-only attachment is explicitly unsupported by
this upstream adapter. Failures preserve owned resources and concise evidence;
the script never force-detaches them. Lurker CLI conformance remains a separate
gate; engine tests cannot substitute for it.

The Rust adapter is `crates/lurker-core/src/bsd/veracrypt.rs`, compiled only for
FreeBSD/OpenBSD. Its opt-in `native_private_veracrypt_lifecycle` test passed on
FreeBSD 15.1 amd64 and OpenBSD 8.0 amd64 with pinned VeraCrypt 1.26.31 and both
reviewed patches: AES, Serpent and Twofish; password and binary keyfile separately;
Unicode/spaced paths and passwords; incorrect credentials; native UFS/FFS2
writes, fsync, close/reopen and retained data. FreeBSD read-only attachment was
also verified for each case. OpenBSD read-only attachment fails explicitly.
The adapter checks private executable/library ownership, source-scoped listing,
auxiliary FUSE mount/permissions and native backing identity before cleanup.
Before acquisition it correlates every engine attachment with verified records
from the locked root-owned registry: source identity, device and native auxiliary
backing must match. Any unmanaged attachment blocks all new VeraCrypt work until
closed with its original tool; a printed pathname cannot prove the engine's
opened inode after replacement. Multiple verified managed sessions can coexist.
Cleanup remains source-scoped, so unrelated unresolved external sessions cannot
block closing an identified owned volume. New root journals save a recovery proof
while control is alive: source inode, native backing vnode and auxiliary mount
fsid. Normal close still uses the live engine. After daemon death, only matching
saved kernel identities permit ordinary device detach and auxiliary unmount;
busy failures retain state, partial cleanup can retry, and reused resources are
rejected. Old proofless journals require live control. Acquisition also rejects
native auxiliary mounts omitted by failed engine discovery. Native regressions verify unchanged
image SHA-256 and original external attachments after rejected alias operations.
The fresh 1.26.31 adapter lifecycle passed all six cipher/credential cells on
both guests with the final nonce patch (FreeBSD 169 s, OpenBSD 182 s), including serialized recovery proofs
and repeated normal close. A FreeBSD owned-daemon SIGKILL regression passed:
drained UFS data survived reopen; orphan discovery blocked acquisition; partial
cleanup retried; source replacement and actual device reuse were rejected;
ciphertext stayed unchanged. No forced unmount or filesystem repair was used.

OpenBSD **dead-daemon cleanup remains fail-closed**: its VNDIOCGET calls
VOP_GETATTR on the backing FUSE vnode, which returns ENXIO after daemon death
even while the vnd unit remains configured. ENXIO is not proof of absence.
Lurker preserves the journal for administrator reconciliation; a cached disklabel,
printed path or guessed unit cannot replace the missing identity. This differs
from normal live-engine close and must not be presented as successful automated
OpenBSD daemon recovery. Native OpenBSD partial-cleanup retry with live auxiliary control passed, including
actual reused-unit rejection, old-proof rejection after same-source reopen and
unchanged ciphertext. The opt-in fault tests are
`native_saved_proof_recovers_dead_control_and_partial_cleanup` on FreeBSD and
`native_saved_proof_retries_partial_cleanup_without_device_adoption` on OpenBSD;
run only on designated guests with `--ignored --test-threads=1`.

These are native **backend adapter** results; they do not claim whole CLI,
desktop or release-package qualification.

Upstream references:

- https://github.com/veracrypt/VeraCrypt/blob/7afc449f2bc69366e8b67b82ad66201325d318c3/src/Readme.txt
- https://github.com/veracrypt/VeraCrypt/blob/7afc449f2bc69366e8b67b82ad66201325d318c3/License.txt
- https://github.com/wxWidgets/wxWidgets/tree/v3.2.8.1
- https://ftp.gnu.org/pub/gnu/libiconv/libiconv-1.18.tar.gz
- https://github.com/veracrypt/VeraCrypt/releases/tag/VeraCrypt_1.26.31

OpenBSD identity ABI reference:

- https://github.com/openbsd/src/blob/master/sys/dev/vnd.c#L548-L567
- https://github.com/openbsd/src/blob/master/sys/dev/vnd.c#L315-L324
- https://man.openbsd.org/fuse_mount.3
