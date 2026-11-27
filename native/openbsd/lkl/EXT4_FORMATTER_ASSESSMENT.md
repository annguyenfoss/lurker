# Native ext4 formatter inventory — no storage experiment

The smallest native formatter candidate is already installed: OpenBSD's signed
`e2fsprogs-1.47.3` package supplies `/usr/local/sbin/mke2fs` and its hardlink
`/usr/local/sbin/mkfs.ext4`. Both are root:bin, mode0755, 156,992 bytes. Native
`mke2fs -V` reports 1.47.3 (8-Jul-2025), with EXT2FS library1.47.3. The executable
SHA-256 is `309c039ae53f9698f99da720f41d926c5c64a892ece31c7bdce002c81ed70b53`.
No new formatter port, binary build, kernel modification or installation is
needed merely to propose a first regular-file ext4 control.

The actual package database identifies `sysutils/e2fsprogs`, native amd64,
`@signer openbsd-80-pkg`, with the official package URL
https://cdn.openbsd.org/pub/OpenBSD/8.0/packages/amd64/e2fsprogs-1.47.3.tgz .
Its recorded mke2fs SHA-256 agrees with the native executable. The database and
native loader inventory bind the installed libext2fs/com_err/e2p/blkid/uuid
closure and base libc/pthread, with separately installed native intl/iconv.
The command used for package ownership is native `pkg_info -q -E FILE`; the
initial attempted -W query was rejected and retained as a diagnostic.

Read-only inventory captured `/etc/mke2fs.conf` and the installed mke2fs manual.
The current ext4 configuration includes journal, extents, huge_file, flex_bg,
metadata_csum/metadata_csum_seed,64bit,dir_nlink,extra_isize and orphan_file.
The manual documents private regular-file formatting, explicit block/inode
sizes, `root_owner` and `root_perms`, disabled lazy inode/journal initialization,
and `nodiscard`. Any actual future command must freeze the exact config,
feature selection, image identity/size and those native options before review;
no guessed option, foreign preformatted image or implicit root ownership
substitution is proposed here. A native checker/dumpe2fs header inventory would
be part of that later formatter control, not evidence that it ran already.

The historical LKL .config enables EXT4_FS, JBD2 and ext4 POSIX ACL/security;
that is only configuration evidence. Its local shutdown-hook/UAPI dependency
makes the existing LKL libraries ineligible for new route admission under the
clarified no-bespoke-kernel rule. An installed eligible native formatter does
not cure that dependency or qualify Create/Read/Write. A first mounted pilot
remains held until an eligible published kernel basis and live guardian
completion authority are established and reviewed.

No mke2fs command except the read-only -V query was executed. No filesystem was
formatted, mounted or checked by this assessment, and no raw device was opened.
The complete native package/config/manual/loader and command receipts are
`build/lkl-native/ext4-formatter-assessment/` under the OpenBSD host evidence
root. The ordinary ledger and historical artifact identities remain unchanged.
