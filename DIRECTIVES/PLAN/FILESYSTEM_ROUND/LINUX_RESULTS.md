# Linux filesystem round results

Implementation in progress. Linux owns archlinux, debian, ubuntu, fedora, opensuse and gentoo exclusively until the jobs below are terminal. No SQLite, GUI, fault, physical-device, kernel replacement or upstream source modification tests were run.

## Source and running jobs

- `9fdca7f`: explicit ext4/Btrfs/XFS/NTFS/exFAT/FAT32 format and mount dispatch; root metadata belongs to the authorized caller.
- `2a23185`: standard Linux tool directories remain available when an ordinary login PATH omits sbin; explicitly empty PATH still disables discovery.
- `1cbeff1`: native APFS/HAMMER2, mapper-scoped inner ZFS pools, UFS1/UFS2 read-only import detection. ZFS uses no persistent cache, broad import or forced export.
- `6d10cfa`: bounded native matrix coordinator. Exact source deployed and CLI/helper built independently on all six distributions. Job paths: `/tmp/lurker-filesystem-round-HOST-6d10cfa/`. Each retains `identity.json`, `launcher.json`, build log and `cases/matrix.json`.
- `7270de9`: formatting only.
- Arch core/CLI/helper candidate checks: 105 passed, zero failed, 12 ignored. Full log: `target/goal-validation/filesystem-round/linux/final-candidate-check.log`. Final shared checks remain pending.

Receipts, inventories, install commands/logs, source archives, source/binary identities and diagnostic snapshots are under ignored `target/goal-validation/filesystem-round/linux/`. The source archive SHA-256 is in `candidate-identity.json`. Ubuntu's launch retry is in `ubuntu-retry/` (closed passed smoke ciphertext removed; the failed fixture preserved).

Current base matrix: OpenSUSE terminal, 42 cases, 30 passed, 12 failed; other five guests still running. OpenSUSE failures comprise six transport-3 userspace prerequisite selections, three ext4 formatter compatibility failures, one native password-quality rejection and two CryFS backing-store limitations. Corrections/replays are pending; these are not final product support claims.

## Confirmed prerequisites and bounded installation findings

All six have native ext4/Btrfs/XFS/exFAT/NTFS-3G/FAT tools installed. Linux privileged VeraCrypt retains the installed restricted private creator. Login-user VeraCrypt needs transport 4; `/usr/local/lib/lurker-qualification-2ebae8f/` has transport 3 and fails safely before create. All six have reusable transport-4 helpers with the Linux 6.12.118 stable source receipt in `~/.cache/lurker/final-14ede46/veracrypt-input/`; replay will use those, without rebuilding or changing LKL.

APFS `linux-apfs-rw` v0.3.22, unmodified commit `80f5db04ecd3b240fee4c67829a5b5465f11f63f`, built against the installed matching Debian 6.12.118, Fedora 7.2.11 and Ubuntu 7.3.0 headers and was installed into administrator-controlled module paths. Debian native and CLI PDF/text lifecycles passed in diagnostic smoke tests. APFS tools are installed on all six (distribution packages or unmodified apfsprogs v0.2.1). Native APFS writes require `readwrite`, and remain experimental upstream.

Ubuntu's unmodified upstream Linux HAMMER2 module builds on its installed 7.3 kernel; its upstream floor is 7.3, higher than the other five guests' kernels. The native formatter was built from unmodified hammer2-utils. Ubuntu's diagnostic full encrypted CLI lifecycle passed. This is a scratch-container result, not a reliability qualification of the new Linux port.

Ubuntu has packaged matching OpenZFS 2.4.4; Debian installed OpenZFS 2.3.9 DKMS against matching headers after enabling the official contrib component. Both modules load. Other distributions' package/header/build limitations are still being bounded and recorded.

Upstream realchonk/fuse-ufs 0.6.0 builds unmodified on all six after installing missing FUSE SDK headers where needed. Portable kusumi/makefs builds unmodified on Arch, Fedora, openSUSE and Gentoo; Debian/Ubuntu use packaged makefs. OpenSUSE's independent UFS2 fixture passes PDF/text write, normal close, reopen and hash verification through the experimental FUSE driver. CLI UFS2 adapter qualification remains pending. UFS1 retains the native kernel read-only route where that module is present.

Primary upstream references:

https://github.com/linux-apfs/linux-apfs-rw
https://github.com/linux-apfs/apfsprogs
https://github.com/jdmanring/linux_hammer2
https://github.com/kusumi/hammer2-utils
https://github.com/realchonk/fuse-ufs
https://github.com/kusumi/makefs
https://openzfs.github.io/openzfs-docs/

## Material limits observed so far

- OpenSUSE e2fsprogs 1.47.0 rejects `root_perms`; a private initialization mount will set the new ext4 root to 0700 while retaining supported `root_owner` formatting. No existing filesystem ownership is changed on unlock.
- CryFS backing exFAT/FAT32 fails upstream publication with `publish new encrypted block: Operation not permitted`. Those backing filesystems lack hardlinks used by the integrated atomic publication. Failed encrypted backing images and supervisor diagnostics are preserved, and normal unmount succeeds. gocryptfs/Cryptomator are assessed separately.
- The normal-user PTY wrapper can race sudo's nested PTY echo state. Privileged tests use the root-owned installed common harness with the system Python under sudo; CLI talks directly to cryptsetup. Login-user routes stay unprivileged. Logs redact credentials, and the rejected diagnostic fixture remains preserved.
- Success proves bounded content persistence and ordinary close/reopen only. ACLs, snapshots, complete metadata semantics, journal recovery and durability under failure are not tested.

## Next action and reviewer handoff

Finish the already running matrices, replay only failed prerequisite/product cases, test available supplemental APFS/ZFS/UFS2 routes and read-only imports, collect concise receipts/tool/source identities, reconcile only owned resources and replace this checkpoint with the final ten-filesystem table per host. No reviewer should access these hosts until implementation reports terminal status.
