# DragonFly native XFS formatter construction

The source archive and 33 official patches are pinned in `recipe.json` to the
ordinary DPorts recipe. Apply those patches and its documented post-patch steps
before these DragonFly patches. The upstream source is
https://git.kernel.org/pub/scm/fs/xfs/xfsprogs-dev.git/ and the exact DPorts recipe is
https://github.com/DragonFlyBSD/DPorts/blob/91731d0e53e99b4ed0ab75677c9325d741a85123/filesystems/xfsprogs/Makefile .

The official recipe changes the upstream AIO `lio_listio` probe into
`timer_create`. DragonFly has genuine native AIO but no POSIX timer_create
implementation. Patch 0001 restores the original AIO probe. The native
`tests/native-aio.c` control, run as real UID/EUID 1300, checks synchronous list
write, fsync, readback, ownership and invalid-FD per-request errors. Compile on
DragonFly with `gcc13 -Wall -Wextra -Werror tests/native-aio.c -o native-aio`,
then supply a new file path beneath a caller-owned directory.

This work targets `gmake -j1 V=1 mkfs` and its required libraries, including the
mandatory CRC self-test. It does not claim the entire utility suite builds:
`xfs_repair` independently uses the unavailable POSIX timer API. No substitute
symbol is supplied. Configure succeeds after patch 0001; the first compiler
attempt then diagnoses the missing DragonFly platform adapter. No native XFS
format/mount/payload or product support is established by these checks.

Patch 0002 adds a distinct DragonFly platform header and implementation, using
native BSD UUID/file APIs without defining another operating system's macros.
Character-device geometry requires a validated `DIOCGPART` result; the returned
size uses libxfs's 512-byte basic-block units. Native fsync errors are fatal,
and unsupported discard returns EOPNOTSUPP. The inherited BSD mounted/writable
checks are corrected in this DragonFly implementation.

The native formatter and required CRC self-test now build. A new 512 MiB regular
image formatted with `-m crc=1 -K` has native XFS superblock magic, 4096-byte
blocks, 512-byte sectors and the expected total size. `tests/platform.c` verifies
regular-file geometry and that a bad-FD native fsync failure exits unsuccessfully.
Compile it against `libxfs/dragonfly.o`, with upstream headers as system includes
and the native intl library; run it as UID/EUID 1300 with a new image pathname.
Native character-device geometry and mounted payload remain separate gates.
