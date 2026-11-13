# Private BSD VeraCrypt build

`python scripts/build-bsd-backends.py` runs natively as a normal user on amd64
FreeBSD 14+ or OpenBSD 7.8+. It builds pinned upstream VeraCrypt and a static
wxWidgets **base** library, then stages `lurker-veracrypt-bsd`, private shared
libraries, notices and `bsd-veracrypt-source.json` in `target/release/backends`.
It never installs packages, loads kernel modules, elevates or tests storage.

Build prerequisites are a native C/C++ compiler, GNU make, Python 3.11+, yasm,
pkg-config, readelf/ldd/strip, FUSE2 headers and PC/SC headers. FreeBSD package
names are `gmake yasm pkgconf fusefs-libs pcsc-lite libiconv`; OpenBSD has FUSE
in base and offers `gmake yasm pcsc-lite`. Source archives and original source
inputs are checked against pinned SHA-256 values before cache reuse. Use
`--offline` after archives have been cached; build logs stay beside the output.
The reviewed BSD-only `auxiliary-path.patch` adds the private auxiliary mount
directory to verbose volume metadata. Lurker uses it to verify that the native
device still backs this volume's FUSE image before cleanup. No encryption or
format primitive is changed. It also fixes libc++ wide-stream UTF-8 listing
output so a Unicode path cannot truncate the identity record. The manifest
records the patch digest.

The staged executable uses upstream's text/stdin interface. It is intended for
Lurker's trusted, administrator-controlled private backend directory; a normal
user's build output must never be automatically elevated. FreeBSD private
libraries use `$ORIGIN/lib`; OpenBSD runtime libraries must come from base.
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
FreeBSD 15.1 amd64 and OpenBSD 8.0 amd64 with pinned VeraCrypt 1.26.30 and the
reviewed patch: AES, Serpent and Twofish; password and binary keyfile separately;
Unicode/spaced paths and passwords; incorrect credentials; native UFS/FFS2
writes, fsync, close/reopen and retained data. FreeBSD read-only attachment was
also verified for each case. OpenBSD read-only attachment fails explicitly.
The adapter checks private executable/library ownership, source-scoped listing,
auxiliary FUSE mount/permissions and native backing identity before cleanup.
These are native **backend adapter** results; they do not claim whole CLI,
desktop or release-package qualification.

Upstream references:

- https://github.com/veracrypt/VeraCrypt/blob/8172bc55f834485f67be271e02c1167981423621/src/Readme.txt
- https://github.com/veracrypt/VeraCrypt/blob/8172bc55f834485f67be271e02c1167981423621/License.txt
- https://github.com/wxWidgets/wxWidgets/tree/v3.2.8.1
- https://ftp.gnu.org/pub/gnu/libiconv/libiconv-1.18.tar.gz
