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
failed. Disk containers passed 24 of 30 attempts across ext4, Btrfs, XFS, APFS,
UFS2, HAMMER2, NTFS and exFAT. UFS1 and missing ZFS modules account for six
initial failures. Directory encryption passed 20 cases; CryFS failed on UFS2
and exFAT because its atomic publication requires unavailable hardlinks, and
Cryptomator returned EIO on UFS2 and NTFS-3G. UFS1 and ZFS backing setup failed.
The original source and binary identities above apply to all initial cases.

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
succeeded; the old 2.4.4 prefix remains intact. The first fresh ZFS/LUKS2
payload lifecycle passed; remaining ZFS cases are running.

Shared fixes committed after the complete initial attempt batch include kmod
applet-name preservation (`294e983`), explicit ext4 formatter selection
(`1b03e52`, corrected native-test import `f22ee9f`), verified native mount
readiness (`836bf84`), and preserving failed fixtures at their active backing
paths (`f78b88e`). Native module and ext4-superblock regressions each passed.
The readiness overlay passed 78 core unit tests and three NTFS-3G payload
lifecycles. It is precommit regression evidence, not final frozen qualification.
The first ext4 regression log contained a missing-trait compilation failure;
the corrected log and follow-up commit are retained without rewriting history.
NTFS3 preference and backend/native-driver fixes remain under validation.

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
ledger retains unfinished native combinations as pending. Device tests,
feature probes, credential/cipher variations, reboot persistence, final
candidate validation and final cleanup remain incomplete.
