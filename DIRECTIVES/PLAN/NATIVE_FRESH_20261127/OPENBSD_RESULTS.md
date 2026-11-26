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

Cipher/credential/error conformance passed eight suites and 102 assertions
with zero failures: CRYPTO AES, VeraCrypt AES/Serpent/Twofish each with password
and binary keyfile credentials, and RAID1C. Three additional real-helper JSON
UFS2 lifecycles, one per backend, passed the downloaded-payload reopen flow.

Two new 512 MiB qcow2 members were attached to this domain as vdb/vdc, serials
`lurker-fresh-openbsd1` and `lurker-fresh-openbsd2`. An orderly reboot enumerated
them as sd1/sd2. Independent first-sector UUID sentinels matched both exact
images before any destructive operation. All eight native physical controls
passed: CRYPTO and two-member RAID1C each with UFS1, UFS2, NTFS and exFAT,
including the downloaded payload and two normal reopen verifications. Baseline
product device requests failed before changing those sentinels; native control
successes are not Lurker device passes.

The private VeraCrypt device control exposed a pinned upstream
`CoreOpenBSD::GetHostDevices` stub containing a bare `throw`, which aborted its
privileged child. A syscall-only trace and a backtrace with frame arguments
hidden established the cause; private cores remain mode 0600. Commits
`49c7729` and `20efd78` replace the stub with real kernel disk inventory and
checked disklabel partition discovery, preserving the upstream refusal to
create over a partitioned whole disk. The frozen patch SHA-256 is
`3385c98e25cbb7be03ab814dbfc984dd77705e256cb14c79393213e654887130`.
Its rebuilt private engine SHA-256 is
`1f360e2066ddc9efffd7d7f381cab4d8c7083b56ac1566be1f4c102df22b3754`.
Eight builder provenance tests passed. Stager verification of the new
OpenBSD-only patch inventory landed separately in `906102a`.

A new native VeraCrypt physical-partition control then passed AES/SHA-512/PIM0
password creation, native FFS2 read/write, downloaded payload, normal close,
reopen/compare, append/new file and final reopen/compare. The outer disklabel
DUID remained unchanged. This control uses sd1a starting at sector 2048. Earlier
trial partitions at sectors 0 and 64 overlapped the outer label and disappeared
on reopen; both failed ciphertext images and diagnostics were preserved before
reinitialization. Here the OpenBSD slice starts at 64 and its disklabel is at
65, so merely using a nonzero partition offset is insufficient.

Owned adapter commits `ce72060`, `ad18cf3` and `7cf6293` add checked physical
backing identities, idle/mounted/swap/native-holder guards, shared CRYPTO/RAID1C
chunk preparation, and partition geometry with explicit outer-label overlap
rejection. Four pure native parser tests passed; a separately root-owned test
executable verified the scratch partition inspector/idle check and rejected the
mounted system disk using read-only queries. No destructive command targeted
the system disk.

Commit `6ab0f8b` additionally protects raw and symbolic whole-device aliases in
VeraCrypt's existing partitioned-disk guard. Three alias guards preserved the
first MiB and the positive physical partition lifecycle still passed. The final
discovery patch SHA-256 is
`d7f3ce8e8b1ee274c455f2b029d044cacaad20b69c56e12458de8ca2152b6d1e`;
the final engine SHA-256 is
`d4cd476c3e1328dccac41aec48a61f87026abfe765ac28e3f6f5b4283c2e1a01`.
Six additional native physical controls passed AES/Serpent/Twofish with password
and binary-keyfile credentials. A separately produced manifest now explicitly
advertises physical-source protocol 1; old SDKs remain file-compatible but are
refused for physical work before credential I/O.

Physical product CRYPTO creation landed in `1a90ba5`. Its first reopen exposed
an existing 56-byte raw metadata read that physical disks reject with EINVAL.
The failed ciphertext image and receipt were preserved; sector-sized reads in
`87215b8` repaired the route. The exact immutable CLI/helper build
`20261126T221654Z-33fb8434` then passed UFS1/UFS2/NTFS/exFAT physical lifecycles,
including wrong credentials, duplicate refusal, busy-close preservation, Auto
reopen, append/new file and final hashes. A private harness revision-label typo
is explicitly corrected by `device/product-crypto-provenance.json`; the
immutable build result identifies the actual source.

Commit `5235cbf` enables guarded physical VeraCrypt create/open and persistent
physical recovery proofs on OpenBSD and FreeBSD. The OpenBSD preview passed
all four writable filesystems with the same downloaded-payload and negative
cases. Its native partition remains outside the outer disklabel sector.
These are source-overlay preview results; final immutable candidate reruns
remain required. FreeBSD qualification is reported by its exclusive owner.

Commit `5b251f4` enables physical RAID1C pairs with independent member proofs,
sector-aligned probes, native UUID validation, and persistent root-owned
`/var/db/lurker-raid1c/<array-UUID>.raid1c.json` recovery metadata. A CLI
file-only admission restriction was found after successful physical creation;
both failed member images and diagnostics were preserved before replacing the
fixture. The corrected preview passed all four writable filesystems, with
wrong-password authentication, duplicate refusal, busy-close preservation,
Auto inference and final hashes. An additional UFS2 physical fixture passed
normal two-member creation and two explicit one-member degraded recovery
opens, append/new file, final hashes and normal close. Independent bioctl and
dumpfs observations confirmed degraded health and FFSv2. No peers were rebuilt;
no arrays or plaintext mounts remained. Native core tests passed 71 cases,
CLI unit tests passed 19, and the designated block/raw parser probe passed one.

Complete workspace tests and frozen-candidate container/device/reboot validation
remain pending. The earlier native workspace-test attempt hit the login shell's
1.5 GiB datasize limit during desktop LLVM code generation; no full pass was
claimed. Exact source `5b251f4` is running the scoped 3 GiB-datasize, one-worker
workspace check/test/product-build recipe in `20261126T224319Z-87bcd67d`.
Do not reboot until its processes terminate and all logs/artifacts are retained.
No complete host qualification is claimed.

The installed NTFS-3G and exfat-fuse packages provide writable candidates;
libfsapfs and libfsxfs expose read-only readers. An ext4 formatter exists, but
the OpenBSD ext2fs reader does not provide the required writable ext4 route.
No native Btrfs, HAMMER2 or OpenZFS tools/drivers were found. CryFS is pending a native source assessment, not excluded because no package
exists. Stable 1.0.3 was fetched at verified commit
`5d0cc36785d93e12250696a3ffc928042f2e8d61` with only the common fsyncdir patch.
Official native CMake 4.3.5, Ninja 1.11.1, Boost 1.91, range-v3 0.12 and spdlog
1.17 dependencies were installed. Native configuration found base libfuse 2.8.0
and succeeded. An isolated actual source compile failed at the upstream
get_total_memory.cpp unsupported-platform guard; OpenBSD's native physical-page
and page-size sysconf values are available, so a small explicit port is being
assessed rather than declaring OS impossibility. The installed fuse_mount(3)
manual requires root for mount/unmount; login-user folder policy therefore also
needs a trusted privileged integration if source/runtime controls succeed.
No native CryFS lifecycle or product pass is claimed yet. Other requested
portable engines retain their explicit per-cell assessment in the ledger.

Upstream capability references reviewed this round:

- https://www.openbsd.org/faq/faq14.html
- https://man.openbsd.org/OpenBSD-8.0/softraid.4
- https://github.com/relan/exfat
- https://github.com/libyal/libfsapfs
- https://github.com/libyal/libfsxfs
- https://github.com/rfjakob/gocryptfs
- https://github.com/cryfs/cryfs
- https://github.com/cryptomator/cryptomator
