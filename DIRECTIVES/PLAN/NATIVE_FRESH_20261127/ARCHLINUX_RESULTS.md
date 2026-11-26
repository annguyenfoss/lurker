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

The initial container matrix is running through ten filesystem rows with
LUKS2, LUKS1 and privileged VeraCrypt, followed by directory encryption on each
backing filesystem. The shared downloaded Bitcoin, Ethereum and Transformer
PDFs and text are used. A first clean LUKS2/ext4 attempt with an incorrectly
typed source field was excluded; its logs remain under
`cases/abandoned-metadata-typo/`. Correctly identified tests restart in
`cases/baseline/`.

Known issues awaiting the complete initial batch: stock UFS1 omits
`CONFIG_UFS_FS_WRITE`; the shared HAMMER2 fixture size is below the formatter's
1 GiB minimum; stable OpenZFS 2.4.4 lacks Linux 7.3 compatibility. A current
OpenZFS development source advertising Linux 7.3 support has been fetched for
ordinary external-module assessment. Custom kernels remain deferred. Debian
also reproduced a shared kmod applet-name bug when canonicalizing modinfo and
modprobe; the Linux owner is preparing a narrow fix after initial attempts.

Private evidence is under
`target/goal-validation/native-fresh-20261127/hosts/archlinux/`. The persistent
remote round root is
`/home/adjutant/.cache/lurker/native-fresh-20261127-archlinux`, exposed by the
prescribed `/tmp/lurker-native-fresh-20261127-archlinux` symlink. The initial
inventory was saved before reboot; the package transaction was recovered from
`/var/log/pacman.log` after volatile `/tmp` was cleared. The applicability
ledger currently marks unfinished native combinations pending. Device tests,
feature probes, credential/cipher variations, reboot persistence, final
candidate validation and final cleanup remain incomplete.
