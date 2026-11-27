# Arch Linux fresh native qualification

Status: running; no prior-round receipts count toward this round.

The full synchronized pacman update completed on 2026-11-27 local time,
installing Linux and headers 7.3.6, tpm2-tss and zstd. An orderly reboot changed
boot ID from `68eb9a41-7454-4f21-9dbf-165c9e7f6951` to
`0ba450f0-990b-418d-ae87-a0016f1db4e4`; the running kernel is
`7.3.6-arch1-1`. ACL tools, attributes, gocryptfs 2.7.0, stable CryFS 1.0.3 and
DKMS were installed. APFS 0.3.22 from the official Arch DKMS package built and
loaded against that kernel. The ordinary external HAMMER2 module built from
https://github.com/jdmanring/linux_hammer2 commit
`09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0` and loaded successfully. These
installation observations are not filesystem lifecycle passes.

Fresh source `c5feaece7277dc4e0562fdcf26641aa63fa53642` passed remote
`cargo check --workspace`, `cargo test --workspace` and workspace binary build
with one Cargo worker: 147 tests passed, 0 failed, 13 ignored. Run ID:
`20261126T203728Z-80ad6623`. CLI SHA-256:
`cf7f2ae09fd482dd1a7b23f9d46941da034582003933e239975e411d7b0a5410`;
helper: `7c11ddf497d6d7690e560596f8de1631e456e39b80e6bfa63c34912178353599`.

The current package verifier accepted the cached gocryptfs, patched stable
CryFS and Linux-patched Cryptomator inputs. VeraCrypt/LKL was verified against
its original exact source `16aa1f4071fff516a8124bbd1f1cdd1937fc7fdb`, retaining
that engine identity; executable inputs are unchanged at the Rust candidate.
Fresh runtime tests use an isolated administrator-owned staging prefix, not an
installed system package. Existing LKL functionality is retained but excluded
from OS-native filesystem acceptance.

The complete initial container matrix recorded 56 attempts: 44 passed and 12
failed. Disk harnesses reported 24 of 30 attempts passed across selected ext4, Btrfs, XFS, APFS,
UFS2, HAMMER2, NTFS and exFAT. UFS1 and missing ZFS modules account for six
initial failures. Directory encryption passed 20 cases; CryFS failed on UFS2
and exFAT because its atomic publication requires unavailable hardlinks, and
Cryptomator returned EIO on UFS2 and NTFS-3G. UFS1 and ZFS backing setup failed.
The original source and binary identities above apply to all initial cases.
Selected ext4 cases were subsequently invalidated: canonical mkfs.ext4 invoked
mke2fs without its applet name and formatted ext2. These are filesystem-identity
failures, not ext4 passes. The corrected formatter regression verifies the ext4
journal and extents flags; final cases use the actual mount-type guard.

Matched native NTFS3 backing controls passed all three directory engines with
the same candidate, engines and downloaded payload. NTFS-3G Cryptomator failure
is therefore a driver-route limitation, with an eligible working alternative.
Native publication controls show exFAT supports atomic rename-without-replace
and preserves the destination on EEXIST; UFS2 fuse-ufs rejects both hardlinks
and rename-without-replace. Native permissions, ownership, ACL and reflink
observations are retained separately and do not establish every engine's
feature support. Stock UFS1 write support is absent; the loaded UFS module's
source version matches the new stock module. Custom kernels remain deferred.

Ordinary external OpenZFS development source
https://github.com/openzfs/zfs commit
`3b6d727ddd8d6d005774605964f044f364ee6456` advertises Linux 7.3 support. Its
2.4.99 module and userland built with one Make worker, staged unprivileged,
then installed into an administrator-owned isolated prefix. Module loading
succeeded; the old 2.4.4 prefix remains intact. All six supplemental ZFS cases passed: three disk containers and three folder
engines on a native ZFS backing, with the same initial Rust candidate.

Shared fixes committed after the complete initial attempt batch include kmod
applet-name preservation (`294e983`), explicit ext4 formatter selection
(`1b03e52`, corrected native-test import `f22ee9f`), verified native mount
readiness (`836bf84`), and preserving failed fixtures at their active backing
paths (`f78b88e`). Native module and ext4-superblock regressions each passed.
The readiness overlay passed 78 core unit tests and three NTFS-3G payload
lifecycles. It is precommit regression evidence, not final frozen qualification.
The first ext4 regression log contained a missing-trait compilation failure;
the corrected log and follow-up commit are retained without rewriting history.
NTFS3 preference, explicit HAMMER2 mounts and modern writable NTFS selection
were subsequently committed after matched native controls.

A first clean LUKS2/ext4 attempt with an incorrectly typed source field was
excluded; its logs remain under `cases/abandoned-metadata-typo/`. Original
backing setup failures predate the harness preservation fix; fresh native
controls retain their failed images. No native device passes are claimed yet.

Private evidence is under
`target/goal-validation/native-fresh-20261127/hosts/archlinux/`. The persistent
remote round root is
`/home/adjutant/.cache/lurker/native-fresh-20261127-archlinux`, exposed by the
prescribed `/tmp/lurker-native-fresh-20261127-archlinux` symlink. The initial
inventory was saved before reboot; the package transaction was recovered from
`/var/log/pacman.log` after volatile `/tmp` was cleared. The applicability
ledger retains unfinished native combinations as pending. Device tests, credential/cipher variations, reboot persistence, final
candidate validation and final cleanup remain incomplete. Native feature
observations for all ten filesystem rows are retained; UFS1 is blocked by
the stock read-only driver.


The earlier Linux core/CLI/helper candidate was frozen at
`ca7d2137fca741008648360e98acc06ce559de3d`. Its final exact workspace run is
`20261126T221423Z-4e5b59b2` and passed. The helper stdout repair
passed 84 Arch core tests (two ignored), followed by openSUSE native CLI/helper
checks and strict parsing of nine complete keyfile helper JSON responses across
LUKS2, LUKS1 and VeraCrypt. The fixed 512-byte LUKS2/UFS1 geometry is included;
Arch still requires an eligible writable UFS1 driver before runtime acceptance.

The current portable CryFS donor was freshly built on Debian at `195b56b`,
validated against all three Linux patches and its 16 license inputs, and passed
Arch exFAT/UFS2 downloaded-payload reopen tests. Its binary SHA-256 is
`2ec7b6f5bd7b999b81750fec75ffb3ca13174f4643d2326ba250143e9f444a14`.
The previous engine and prefix remain preserved.

Arch rebuilt fuse-ufs upstream `ee5c4a447da98220a11edb69a225d51379afc4fb`
with the scoped no-replace and barrier patches from `94da64c` and `e97e480`.
The 28 library tests passed. Native controls verified forwarding file and
directory fsync to the owned backing file, injected EIO propagation and
successful retry (three cases passed). Driver SHA-256 is
`2dd3df260eb9006f1696bb426ca8367522c81d0f8881600135bfa2ebbe307f7c`.
This driver is a separately installed host prerequisite. All six UFS2 product
routes passed at ca7d213; no crash or power-loss guarantee is inferred.


The complete ca7d213 sweep finished with 54 passed cases and four stock-UFS1
failures (three disk cases and directory-backing setup), without cleanup errors.
All nine other filesystem rows passed all six applicable native encryption
methods. The UFS1 failures exposed Linux mountinfo reporting VFS `rw` together
with superblock `ro` on encrypted mappers. Commit `576505d` reads both fields
in core and the payload guard. Its targeted checks passed 85 core tests (two
ignored) and nine Python tests. Native UFS1 create/reopen now reject the
read-only downgrade and clean up normally; a writable Btrfs keyfile volume
passed all downloaded-payload phases. The first native probe lacked its staged
helper; that staging failure is retained separately and was corrected before
counting the result.

The final follow-up source for all three products is
`576505d554d798422840a06322dfa93b899365bc`. Remote run
`20261126T223513Z-a9473495` passed workspace check, tests and build: 159 passed,
zero failed, 15 ignored. The native Arch package was built from the verified
six-backend staging archive, with current Rust dependency notices. Native
package SHA-256:
`6fa40a5e78f90b365bfe36c6c053fba03e507e7d57984d9416bb3038bc02318a`;
staging archive:
`b8a8220668ccd72813ba300ab6fbf725aa1a51c649552a64065d7496d46d9901`.
The full follow-up sweep uses an administrator-owned isolated extraction of
that actual native package; the existing system installation is preserved.
CLI SHA-256 is
`59f439699ebeea1c375f578a31cd8c0859610ece536f803bc200309f08a1be6c`;
helper is
`6e14d7dedd1607991060e7522dcbfdc0944418da53968b6b3d4e277b01aa041e`.
Its complete container, variation, device and reboot gates remain running or
pending; this is not a completed qualification claim.

Independent per-mount blkid observations bind all three follow-up UFS2 inner
cases to VERSION=2, together with the exact patched driver identity. A private
wrapper retains subsequent generated test passwords at mode 0600 and records
its own hash, the unchanged frozen harness hash and executed arguments in each
receipt. Earlier failed ciphertext with lost in-memory passwords is retained
honestly as unreopenable. Complete failed images were transferred to the
coordinator, byte-verified, then reclaimed only from closed owned guest
fixtures. No unrelated cached artifacts or storage were deleted.

Formatter provenance was refreshed without changing the active tools. A fresh
unmodified build of https://github.com/kusumi/makefs at
`607449619ef5f85b6a2b282e16182e342fdebeaa` reproduced the installed
makefs-freebsd binary exactly (SHA-256
`a03cc9ffd8f3645b37bfd72c961a11a4c2579f3030b93522f96a5618a5e0b4ce`).
Its project copyright is BSD-2-Clause, with additional vendored source notices.
https://github.com/kusumi/hammer2-utils remains at
`359b25bc2bb5d81e0bdbb5d48439314d80d13086`, version 0.5.0, with a BSD-3-Clause
copyright. The cached source archive's Git commit metadata and built formatter
match the installed binary; fresh upstream HEAD queries confirm those source
identities. Prior runtime receipts do not count as new qualification.


The complete 576505d container follow-up now passes all 54 applicable positive
cells across the nine writable filesystems. The six UFS1 cells remain blocked
by the unchanged stock read-only driver; its three inner create paths reject
that downgrade and release their resources. Three exFAT retention-wrapper
checks stopped before invoking a backend because the initial mask synthesized
mode 0700 for private files. Separate reruns with explicit `fmask=0177,dmask=0077`
passed all three folder engines. These were harness setup failures, with the
original receipts and closed backing images retained.

The additional Btrfs cipher/credential sweep passed 18 cases across all three
native disk methods, AES/Serpent/Twofish, password/binary keyfile and Auto reopen.
Native CLI/helper frontend checks passed. Kernel and folder conformance suites
each passed 36 cases. Actual desktop winit/software rendering passed, and six
strict-JSON helper fixtures are prepared for the pending reboot gate.

A new one-GiB qcow2 scratch disk was attached with serial
`lurker-fresh-archlinux`; every destructive operation checks its serial, size,
type, device number, mounts and children. All 27 physical inner cases passed:
nine writable filesystems times LUKS2, LUKS1 and VeraCrypt. The native physical
folder-backing sweep has passed 15 engine cases on ext4, Btrfs, XFS, APFS and
UFS2. Its initial HAMMER2 metadata probe stopped before backend execution because
blkid lacks that signature; `file` and the header independently identified
HAMMER2. The complete quiescent disk was exported and byte-verified before
correcting that private harness check and continuing. Remaining physical
backings, factor cases, stock-UFS1 negatives and reboot persistence are pending.

A separately identified loop-lease follow-up, commit `67894f6`, isolates only
sudo-elevated socket helpers into their own process group while preserving their
session and controlling terminal. The exact overlay passed 88 Arch core tests
(two ignored), native password frontend checks and a seven-action keyfile
payload lifecycle. Ubuntu reproduced the native password frontend failure with
576505d and passed the unchanged frontend harness plus keyfile lifecycle with
this overlay. Its 168 process samples show the same session, a separate lease
process group and no stopped processes. Existing 576505d storage receipts keep
their original binary identities; they do not claim to test the follow-up.


The 576505d physical baseline is now complete for the nine writable filesystem
rows: 27 encrypted inner cases and 27 directory-engine cases on native physical
backings passed. The whole-disk exFAT formatter created a recursive MBR entry;
the strict child guard stopped the following setup before mutation. The exact
owned partition geometry, absent holders/mounts and completed preceding case
were verified before clearing that scratch layout. ZFS then passed separately.
Stock UFS1's physical native backing mounted read-only and was rejected before
any folder backend ran; its complete raw image is retained and byte-verified.

The private folder wrapper embedded complete operation and hash receipts but
initially left successful console logs inside the subsequently reformatted
backing. This affects the prior 24 successful physical folder cases. Their
structured runtime receipts remain evidence; ZFS's 21 console logs were recovered
before its backing was cleared. Future affected gates copy logs outside the
backing before reformatting. No failed physical ciphertext was overwritten
without a complete verified snapshot.

Final Linux runtime source is now `67894f68525c55d5cbb48c743e1a98a308bd970f`,
with the targeted follow-up scope in LINUX_FOLLOWUP.md. Exact workspace run
`20261127T000844Z-4b4285b2` passed check/test/build: 168 passed, zero failed,
15 ignored. Native packaging currently stops at the host-FUSE closure policy:
the historical VeraCrypt closure included libfuse3.so.4 privately. The assigned
producer/consumer repair is pending; that block is not a runtime pass. Existing
576505d package and matrix identities are retained.

The UFS2 prerequisite now includes the pagination patch from `9e27dcc`, after
the prior no-replace and barrier patches. Arch reproduced the old driver's
204-of-512 listing and missing newly created entry. The rebuilt driver passed
28 library tests and returned all 512 entries, then 513 including the new entry,
with consistent repeated enumeration and normal unmount. Its SHA-256 is
`e6af0568c2039eb9d078e24f046384bf0b3d5f37ed693e22e3908d50b9cdbc9a`.
All 12 affected UFS2 container/device product cases will run on the final package.
The earlier six prepared helper fixtures were fully archived and byte-verified;
three closed encrypted images were reclaimed before preparing a fresh final
reboot cohort. They are marked retired before reboot, not reboot passes.
