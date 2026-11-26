# OpenBSD fresh qualification — in progress

Fresh observations began against product commit
`8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`; previous results are not passes for
this round. Private evidence is under
`target/goal-validation/native-fresh-20261127/hosts/openbsd/`.

The host is OpenBSD 8.0 amd64. `syspatch` installed `80-023_vnode` and
`80-024_swpgonly` successfully; `pkg_add -u` completed successfully with no
package changes reported. The updated GENERIC.MP#14 kernel is running after an orderly reboot. SSH key authentication as root provides scoped privilege;
no sudo/doas configuration was changed.

The exact-baseline `cargo check --workspace` passed in job
`20261126T203457Z-ddbc94db`. Its test compilation was explicitly cancelled
through the runner to prioritize native runtime; no test pass is claimed. The
CLI/helper build passed in `20261126T204853Z-493f2f25`. The private VeraCrypt
engine was rebuilt from verified pinned sources with all current OpenBSD
patches, including FFS shutdown. Its new SHA-256 is
`8b7be9ed8ec73edfe64ec950e9e596f8b6e67afed7e3063183934318294b1f55`.

The 130-cell applicability ledger now records 12 freshly passed container
lifecycles: CRYPTO, RAID1C and VeraCrypt each over UFS1, UFS2, NTFS and exFAT.
Each created its own encrypted image, mounted an actual native filesystem,
wrote the downloaded Bitcoin/Ethereum/Transformer PDFs and text, closed
normally, reopened and checked hashes, appended/created another file, then
closed/reopened and checked all five hashes. Each final normal close succeeded.
The staged CLI SHA-256 is
`8fa88244b87a7f4500f5dc311a9ab0e319ebde4e5ac318c6e52a3ccf1d494ac8`.
These are staged-artifact results, not installed-package qualification.

Three additional new UFS2 fixtures, one per encryption backend, remained mounted
through an ordinary reboot. All three reopened directly through `--type auto`,
matched the five expected hashes, and closed normally without repair or manual
reconciliation. Boot identity changed from 2026-11-26 20:53:46 UTC to 21:00:28 UTC.
The initial container source/log/receipt archive is retained locally as
`logs/initial-container-evidence.tar`. Runtime source/logs and fixtures persist
under `/root/lof27` on the guest.

The executed filesystem harness uses `c5feaec` as its base with an in-progress
bounded-payload update; its exact retained source SHA-256 is
`36a20914bbe39396cc6420b9fd51fc975e54d62a970da68669677191c376384d`.
Initial `dumpfs -s` feature queries were unavailable on OpenBSD and are explicitly
marked unknown; payload persistence results do not depend on those queries.
The first kernel-activation reboot cleared OpenBSD `/tmp` (`/var/tmp` links
there). Both build result receipts and source archives survived locally, but
remote full compile logs were not collected before that reboot and are missing.

Cipher/credential/error conformance is running. Scratch disks are newly created
and still unattached. Physical native controls, product device fixes and final
workspace tests remain pending. No complete host qualification is claimed.

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
