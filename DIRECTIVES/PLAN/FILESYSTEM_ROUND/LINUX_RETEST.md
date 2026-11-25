# Linux filesystem retest checkpoint, 2026-11-25

This retest adds only previously unachieved routes after guest updates. The
prior achieved filesystem/encryption pairs, read-only imports, and Ubuntu's
writable HAMMER2 cases were carried under their original receipt identities,
without rerunning them. All new payload tests use disposable CLI file containers
or directory sources on newly writable backings, a PDF and UTF-8 text, normal
close/reopen, append/new-file mutation, and SHA-256 verification. No GUI,
physical-device, SQLite, mounted-volume reboot, fault or upstream-patch test
is included.

## Added results at distro-owner handoff

| Host | New container passes | New directory passes | Remaining newly feasible cases |
|---|---:|---:|---|
| Arch | UFS1 2, HAMMER2 3 | UFS1 3, HAMMER2 3 | None |
| Debian | UFS1 2, HAMMER2 3 | UFS1 3, HAMMER2 3 | None; return to stable boot |
| Ubuntu | UFS1 2 | UFS1 3 | None |
| Fedora | UFS1 2, HAMMER2 3 | UFS1 3, HAMMER2 3 | None |
| openSUSE | UFS1 2, ZFS 3 | UFS1 3 | ZFS 3 directory cases; harness setup fix |
| Gentoo | UFS1 2, APFS 3, ZFS 3 | UFS1 3, APFS 3, ZFS 3 | HAMMER2 3 container + 3 directory cases |

This is **63 additional passing pairs**. The openSUSE ZFS directory backing
failed before any of its three engine cases because `sudo zpool` could not
find the newly installed tool outside sudo's secure path. The provider itself
passed all three container routes; the directory cases remain unexecuted.
Gentoo's HAMMER2 module and formatter are built and loaded on an alternate
stock kernel, with its six payload cases still unstarted at this checkpoint.

The two UFS1 container routes are LUKS1 and privileged VeraCrypt. The other
new filesystem container routes are LUKS2, LUKS1 and privileged VeraCrypt.
Directory cases are gocryptfs, CryFS and Cryptomator, using the filesystem only
as their encrypted source backing. All new container filesystem roots were
owned by invoking uid/gid 1300 with mode 0700.

## Implementation and providers

Product commit `21958ce70b7166707938725bd28375526d9276a9` enables UFS1
creation with `makefs-freebsd` and native `44bsd` mounts. Matching Linux UFS
modules require upstream `CONFIG_UFS_FS_WRITE`. New filesystem initialization
and writable imports reject the kernel's silent read-only fallback. Default
LUKS2/UFS1 creation is refused before destination mutation because its selected
4096-byte logical sectors cannot satisfy the native `44bsd` driver's initial
512-byte probe. The Arch preflight-refusal control passed. Existing crypto
profiles and filesystem defaults remain unchanged. Follow-up `befa5b2` retains
the pre-existing refusal of unqualified native UFS2 writes without `fuse-ufs`.
Harness commit `43a0938` adds newly writable UFS1 directory backings.

All six standalone UFS modules were built from unmodified upstream `fs/ufs`
source against their matching installed SDKs with the upstream write option.
No kernel was rebuilt or patched. The source versions were v7.3 on Arch,
Ubuntu and Fedora, v6.12.118 on Debian and openSUSE, and v6.18.64 on Gentoo.
Exact administrator-owned insertion was necessary where ordinary modprobe
selected the shipped read-only module. Each raw write/close/reopen control
passed before encrypted payload tests.

New HAMMER2 providers use unmodified Linux port commit
`f34c6a2cd5cf0f1a6e36aef6f255b5f52e8098a1` and hammer2-utils commit
`359b25bc2bb5d81e0bdbb5d48439314d80d13086`. Arch 7.3.5, Fedora 7.3.4 and
Debian's official backports 7.3.4 now meet its kernel floor. Gentoo's official
`~amd64` binary 7.3.6 is prepared for the outstanding cases. openSUSE's
configured official repositories still offer a 6.12 kernel.

Gentoo's new complete SDK enabled unmodified APFS v0.3.22 commit
`80f5db04ecd3b240fee4c67829a5b5465f11f63f` and OpenZFS 2.4.4 on stable
6.18.64. openSUSE also built and installed OpenZFS 2.4.4 unmodified: the prior
`lookup_bdev` configure failure was resolved by identifying its split SDK's
source and object paths correctly. A staging-only `lib -> usr/lib` link
resolved merged-/usr installation layout. This supersedes the earlier
classification of that result as a demonstrated kernel API incompatibility.

## Identities, checks and host state

Native executable identity remains product 21958ce. Each guest independently
built CLI/helper and installed them under root-owned
`/usr/local/lib/lurker-filesystem-retest-21958ce/`. The source candidate at
`c2a2538b9c1f0484331327e31d27767915fc22ac`, including the UFS2 follow-up,
passed final scoped Arch core/CLI/helper `cargo check` and `cargo test`:
**108 passed, zero failed, 12 ignored**. Archive SHA-256 is
`1f66f59d3e795f489cf6e2b75fbe242eaeab9278f5725b8b18d2bc26106bf1a2`.
No complete workspace/desktop bootstrap was performed for this CLI scope.

Ignored evidence and resumption instructions are at
`target/goal-validation/filesystem-retest/linux/HANDOFF.md`.
`HOST-handoff.json` contains complete per-case receipts, hashes, candidate
identities, provider status, boot IDs, worker and mount inventory. Remote
receipts are under `~/.cache/lurker/filesystem-retest/`: `cases.json` on all
hosts, `cases-hammer2.json` on Debian, and `cases-zfs.json` on Gentoo/openSUSE.
All custom workers were terminal at handoff; all inspected owned mounts,
`lurker_*` mappers and loop attachments were absent. Closed passed container
images were removed; earlier failed fixtures and provider evidence remain.

Debian and Gentoo retain their prior stable boot entries as explicit defaults.
Their newly installed kernels were selected with one-time GRUB entries; return
to stable kernels after the remaining cases so prior ZFS availability is
preserved. Debian's existing ZFS 2.3.9 DKMS cannot build for 7.3, so an
exact-kernel compatibility exclusion allowed kernel configuration to finish;
`dpkg --audit` is empty. The temporary exclusion file and return-to-stable
steps are recorded for its next owner. Gentoo's `~amd64` selection was scoped
to the installation process; persistent package keyword policy was unchanged.

The six historical default-LUKS2/UFS1 read-only failures and twelve CryFS
exFAT/UFS2 publication failures remain recorded limitations. This checkpoint
does not promote them or rerun their previously achieved neighboring pairs.
