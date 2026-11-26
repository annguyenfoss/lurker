# OpenBSD fresh qualification — in progress

Fresh observations began against product commit
`8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`; previous results are not passes for
this round. Private evidence is under
`target/goal-validation/native-fresh-20261127/hosts/openbsd/`.

The host is OpenBSD 8.0 amd64. `syspatch` installed `80-023_vnode` and
`80-024_swpgonly` successfully; `pkg_add -u` completed successfully with no
package changes reported. The updated kernel still needs activation after the
active build finishes. SSH key authentication as root provides scoped privilege;
no sudo/doas configuration was changed.

The exact-baseline workspace build/check/test job is
`20261126T203457Z-ddbc94db`, remote supervisor PID 85461. The private VeraCrypt
engine was rebuilt from verified pinned sources with all current OpenBSD
patches, including FFS shutdown. Its new SHA-256 is
`8b7be9ed8ec73edfe64ec950e9e596f8b6e67afed7e3063183934318294b1f55`.

The 130-cell applicability ledger records 12 pending container lifecycles:
CRYPTO, RAID1C and VeraCrypt each over UFS1, UFS2, NTFS and exFAT. They use the
three freshly downloaded PDFs and text through harness revision `c5feaec`.
Runtime, reboot persistence, physical devices and final validation are pending;
no container or device pass is claimed yet.

The installed NTFS-3G and exfat-fuse packages provide writable candidates;
libfsapfs and libfsxfs expose read-only readers. An ext4 formatter exists, but
the OpenBSD ext2fs reader does not provide the required writable ext4 route.
No native Btrfs, HAMMER2 or OpenZFS tools/drivers were found. Requested portable
crypto engines without an available OpenBSD route remain unavailable, not passes.

Upstream capability references reviewed this round:

- https://www.openbsd.org/faq/faq14.html
- https://man.openbsd.org/OpenBSD-8.0/softraid.4
- https://github.com/relan/exfat
- https://github.com/libyal/libfsapfs
- https://github.com/libyal/libfsxfs
- https://github.com/rfjakob/gocryptfs
- https://github.com/cryfs/cryfs
- https://github.com/cryptomator/cryptomator
