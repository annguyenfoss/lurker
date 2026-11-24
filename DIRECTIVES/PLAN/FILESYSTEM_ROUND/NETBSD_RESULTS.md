# NetBSD filesystem round — implementation checkpoint

Host: `adjutant@netbsd-core-01`, NetBSD 11.1 GENERIC amd64, 2 CPUs,
4 GiB RAM, 16 GiB root free at preflight. No existing CGD/vnd sessions or ZFS
pools were present. SSH root authorization was already configured; no sudoers
or temporary authorization changes were made. Native fixtures are under
`/tmp/lurker-filesystem-round-netbsd-20261125/`.

CGD is the only integrated encryption route on this OS. Directory backends are
not implemented here, so directory-on-filesystem tests are structurally outside
this implementation. No additional encryption engine was added.

## Implementation and installation

Own commits: `31c2853`, `5770ed4`, `805c0c2`, `11a7e5b`, `ceb1c50`, `cfcbd35`, `326f77b`, `efb63c6`, `58c97b0`.
Existing FFSv2/default CGD sidecars retain their FFS verifier and unchanged
AES-XTS512/Argon2id profile. Explicit non-FFS selection uses the same crypto
profile with strict native disklabel verification and one offset-2048 `a`
partition. The sidecar still accompanies the image. Arbitrary key generators
and verifier `none` remain rejected. WAPBL is required for writable FFS only.

Added native formatter/probe/mount/identity dispatch for UFS1, UFS2, restricted
ext4, exFAT, NTFS, FAT32, HAMMER2 and ZFS. FUSE ownership comes from per-mount
uid/gid/mode options for filesystems without POSIX owners. NetBSD librefuse
identifies its mount as `/dev/puffs` plus `puffs|/dev/cgdNa`; NTFS libperfuse uses
the actual source plus `puffs|perfuse|fuseblk`. ZFS import scans a private
directory containing only the exact block-device symlink; scanning `/dev`
selects conflicting raw/block aliases on this host. Cleanup exports only a pool
with its verified single device and no mounted datasets.

Installed official packages: `fuse-exfat-1.4.0`, `fuse-2.9.9`,
`libntfs-2026.2.25`, `fuse-ntfs-3g-2026.2.25`, `ntfsprogs-2026.2.25`,
`e2fsprogs-1.47.4nb2`, `fuse-ext2-0.0.11`, and dependency `libblkid-2.42.3`.
Package URL used:
https://ftp.netbsd.org/pub/pkgsrc/packages/NetBSD/x86_64/11.0/All/

`pkg_add -v fuse-exfat fuse-ntfs-3g fuse-ext2 e2fsprogs` with the existing CDN
configuration stalled; the FTP HTTPS mirror exposed a pkg_add crash/partial
registration. Downloading the same official archives and reinstalling locally
completed installation. The incomplete new e2fsprogs registration was removed
with `pkg_delete -f e2fsprogs`, then `pkg_add -v` on the local archive restored
all files. No preexisting package was removed. Logs remain remote.

HAMMER2 upstream source is clean at
`64095c3947f246f3eb6a8d7341ba856e057b6a6c`:
https://github.com/kusumi/netbsd_hammer2

Installed matching NetBSD 11.1 syssrc and the two unchanged mount utility source
headers/helpers from the 11.1 src set. Exact source archives:
https://cdn.netbsd.org/pub/NetBSD/NetBSD-11.1/source/sets/syssrc.tgz
https://cdn.netbsd.org/pub/NetBSD/NetBSD-11.1/source/sets/src.tgz

Build invocation, using standard BSD make variables without source edits:
`make HAMMER2_WRITE=1 LDADD='-lz -lcrypto -lutil'`.
The write-enabled module loads on the matching GENERIC kernel; utilities are
root-owned under `/usr/local/sbin`. The module is root-owned at
`/stand/amd64/11.1/modules/hammer2.kmod`. No kernel replacement or upstream
patch was used. Source/build and install hashes are retained with receipts.

## Native results

All payload cases use the common CLI harness and a fresh 0.5 GiB encrypted
container (2 GiB for HAMMER2): real PDF plus UTF-8 text, normal close/reopen, original hash check,
append text/add second file, normal close/reopen, final three hashes. These
content checks do not establish complete ACL, journal, snapshot or metadata
feature parity. No reboot, fault, SQLite, GUI or physical-device tests ran.

| Filesystem | Installed route / metadata scope | Create + CGD lifecycle | RO | Limit |
| --- | --- | --- | --- | --- |
| ext4 | e2fsprogs + fuse-ext2 0.0.11 | passed | unexercised | Experimental writes; disables 64bit, metadata_csum and orphan_file. No modern ext4 feature-parity claim. |
| Btrfs | No established native/package route | unavailable | unavailable | No invented bridge or kernel patch. |
| XFS | No established native/package route | unavailable | unavailable | No established NetBSD mount driver. |
| APFS | No established native/package route | unavailable | unavailable | No native NetBSD APFS formatter/mount route. |
| UFS1 / FFSv1 | Base newfs -O 1 + mount_ffs | passed | unexercised | Writable WAPBL enforced. |
| UFS2 / FFSv2 | Base newfs -O 2 + mount_ffs | passed | unexercised | Existing default; writable WAPBL enforced. |
| HAMMER2 | Unmodified upstream write module + tools | passed | unexercised | Explicit partition-bounded size; at least 1025 MiB. Native probe recognizes public header magic because base fstyp fails its ioctl. |
| NTFS | ntfsprogs + NTFS-3G 2026.2.25 | passed | unexercised | Per-mount owner/mode; no POSIX ownership parity. Isolated daemon terminal session and native mount readiness wait. |
| exFAT | fuse-exfat 1.4.0 | passed | unexercised | Buffered block formatter required; raw formatter fails also on plain vnd control. |
| ZFS | Base ZFS module/tools | passed | unexercised | Restricted block-device import avoids raw/block alias collision; scoped export before CGD close. |

Additional existing enum FAT32: native newfs_msdos/mount_msdos lifecycle passed;
not an additional required table cell.

## Evidence and next action

Exact final source: `58c97b0188ee2d604a094acb9ffe0e10ce84de23`, generated with
`git archive`, checked/tested/built natively in
`/tmp/lurker-filesystem-round-netbsd-20261125/exact-58c97b0/` with existing
Rust/Cargo 1.100.0 and cache
`/home/adjutant/.cache/lurker/remote-test/cargo/65b12bedef1ed835ded9/target`.
No workstation build/test was used. Check/build include CLI, core and helper;
`cargo test -p lurker-core -p lurker-cli -p lurker-helper` reports 69 passed,
0 failed, 4 ignored. Ignored privileged/fault fixtures were not part of this
round. Root coordinates final workspace/desktop checks.

The final exact-source seven-pair CGD CLI run passed: 49 CLI operations,
0 failures, no remaining mounts. Receipts, build/test logs and hashes are in
`target/goal-validation/filesystem-round/netbsd/summary.json` and its sibling
evidence files. Final CLI SHA-256:
`1de769160ae8ce9ad15580613b966f687914940c23d3d7077140b1c5022195e3`.
RO options exist in the tools/code but RO was not separately exercised for
these RW-qualified pairs, as directed by the coordinator. None of the available
NetBSD rows is limited to RO. Feature/metadata claims are limited to upstream
capabilities; payload evidence establishes content persistence only.

All original failed fixtures/logs are preserved remotely. Some intermediate
receipt commit labels name development snapshots; the final summary selects
only full exact-source commit receipts. Earlier bugs included synthetic
`disklabel` read exit 102, raw-device exFAT writes, FUSE mount identity/terminal
lifetime, ZFS alias scan, and HAMMER2 whole-disk size ioctl. Ordinary native
controls distinguished adapter defects from upstream behavior. No upstream
source edit was made. Owned interrupted initialization mounts were normally
unmounted and the CLI closed their verified recovery sessions.

Final cleanup matches preflight: only the original root FFS, ptyfs and tmpfs
mounts; all four vnode and eight CGD units unconfigured; no ZFS pools; registry
contains only its lock. No candidate CLI, harness or backend workers remain.
No force/lazy unmount was used. Root has 15 GiB available after qualification.
Packages, source headers, HAMMER2 module/utilities and isolated root-owned
candidate prefix remain installed intentionally. Failed fixtures and ciphertext
are preserved remotely. No temporary authorization exists. All jobs are terminal.

Reviewer next action: read local `summary.json`, inspect final diffs and selected
seven full-commit receipts, then rerun only missing/failed meaningful cases.
Avoid replaying intermediate development failures already resolved. The base
fstyp limitation and size ioctl behavior require the small HAMMER2 adapter;
no upstream patch or repair is pending.
