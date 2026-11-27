# Arch Linux fresh native qualification

Status: ordinary-route qualification complete. No prior-round runtime receipts
count. Applicable container and physical cells, final installed-package gates and
ordinary reboot are reconciled. Stock UFS1 remains blocked by its read-only
driver; custom kernel builds remain deferred by the round authority.

The synchronized pacman update and orderly reboot completed. Running kernel is
`7.3.6-arch1-1`; boot ID changed from `68eb9a41-7454-4f21-9dbf-165c9e7f6951`
to `0ba450f0-990b-418d-ae87-a0016f1db4e4`. Native packages include APFS DKMS
0.3.22, matching the running kernel. Ordinary external HAMMER2 source is
https://github.com/jdmanring/linux_hammer2 at
`09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0`. OpenZFS source is
https://github.com/openzfs/zfs at
`3b6d727ddd8d6d005774605964f044f364ee6456`, version 2.4.99 with Linux 7.3
support. Both matching external modules and native tools were built, installed
and exercised freshly; the old OpenZFS 2.4.4 prefix is preserved. No custom
kernel was built. Stock UFS module source identity matches its installed module
and `CONFIG_UFS_FS_WRITE` remains unset.

The complete baseline uses source `576505d554d798422840a06322dfa93b899365bc`.
Its exact workspace check/test/build passed 159 tests, zero failures, 15 ignored.
All nine writable filesystem rows passed all three native disk methods and all
three directory engines, both in containers and on the new physical scratch
disk: 54 positive cells in each dimension. The 130-cell ledger records 70
OS-inapplicable combinations and six UFS1 driver-blocked cells per dimension.
Directory engines use the named filesystem as their native backing, rather than
as a selectable inner format. These baseline counts retain their original
driver/source identities. The later unprivileged external-host-driver follow-up
is recorded separately below.

| Filesystem | Container cells | Physical cells | Native route |
| --- | ---: | ---: | --- |
| ext4 | 6 passed | 6 passed | kernel ext4; actual identity and journal verified |
| Btrfs | 6 passed | 6 passed | kernel Btrfs |
| XFS | 6 passed | 6 passed | kernel XFS |
| APFS | 6 passed | 6 passed | matching APFS writable module |
| UFS1 | 6 blocked | 6 blocked | stock driver mounts read-only; safe admission rejection |
| UFS2 | 6 passed | 6 passed | separately installed patched fuse-ufs; independent VERSION=2 proof |
| HAMMER2 | 6 passed | 6 passed | matching external writable module; explicit native type |
| NTFS | 6 passed | 6 passed | stock writable NTFS3; NTFS-3G failure retained separately |
| exFAT | 6 passed | 6 passed | kernel exFAT; private backing masks explicit |
| ZFS | 6 passed | 6 passed | matching OpenZFS 2.4.99 module and userland |

Final runtime source is `67894f68525c55d5cbb48c743e1a98a308bd970f`. Exact run
`20261127T000844Z-4b4285b2` passed workspace check/test/build: 168 passed,
zero failed, 15 ignored. This follow-up isolates only non-console sudo loop
leases into their own process group while preserving their session and cached
TTY authorization. Ubuntu reproduced the prior password frontend failure and
passed the unchanged frontend suite plus keyfile lifecycle with the fix; 168
process samples showed the same session, a separate group and no stopped tasks.
Arch GNU-sudo controls also passed. Unchanged baseline coverage retains its
original 576505d identities, as specified by LINUX_FOLLOWUP.md.

The final native Arch package is installed through an administrator-owned
isolated extraction; the existing system installation is preserved. Package
SHA-256 is `59b39ef1df0db51f222b1f60948074a1a1c72c0e697459ea83ce556106101f0a`;
staging archive is
`d1cb36edf329f8669c5fdc09e49410fd03a317fdc277194651da34b1ea713e62`.
Component SHA-256 values are:

- CLI: `9b1ef704eb1c3cac38127cc5caf6e26df177b4d3f14321c93416a557429dcf02`
- helper: `a156086e4b70afbbd39724b63b8e6a42264cb9162f8a768afb6a1b8ef8d31d6e`
- desktop: `d88c1b2d2d6030b12a5445e1a580b9416142eb1233c2beb98a7b27c28bf1325f`

Packaging policy is identified separately: producer `7d8f9a4` and consumer
`fa063ac`. The original verified VeraCrypt creator bytes remain
`7975cc4724d558fd10bafcc670627c56959a1c76d721f7f83dce855eb0052769`;
its original source/static-wx identity is retained. Its refreshed private closure
contains legitimate zlib, while signed host FUSE ABI4 resolves through the native
GNU loader. No FUSE library is bundled. The portable CryFS donor remains the
fresh Debian build at `195b56b`, validated against all three Linux patches and
16 license inputs; binary SHA-256 is
`2ec7b6f5bd7b999b81750fec75ffb3ca13174f4643d2326ba250143e9f444a14`.
Cached gocryptfs 2.7 and Linux-patched Cryptomator retain their independently
verified source and executable identities. Native runtime observations are new.

UFS2 uses https://github.com/realchonk/fuse-ufs at
`ee5c4a447da98220a11edb69a225d51379afc4fb`, BSD-2-Clause, with separate host
patches `94da64c`, `e97e480` and `9e27dcc`. Final driver SHA-256 is
`e6af0568c2039eb9d078e24f046384bf0b3d5f37ed693e22e3908d50b9cdbc9a`.
The 28 library tests passed. Native controls reproduce the old 204-of-512 listing
and omitted new entry, then verify all 512 entries and 513 after creation with
the corrected driver. File/directory backing-fsync forwarding and injected EIO
propagation/retry controls passed separately. All twelve affected UFS2 container
and physical product cases passed again on the installed final package, with
independent version/driver proof and complete downloaded-PDF normal reopens.
New physical folder logs are copied outside the backing before reformatting.

Final installed-package normal-user file lifecycles passed all six combinations
of LUKS1/LUKS2/VeraCrypt and password/binary keyfile, with Auto reopen and full
PDF/text write, close/reopen, append/new-file, close/reopen hashes. The unmodified
native frontend suite passed its interactive CLI, helper success/error and
terminal-survival checks. Strict helper physical samples passed all three disk
methods, wrong-key rejection/recovery and all 30 complete stdout JSON parses.
Three physical UFS1 creates safely reject read-only initialization without owned
mounts or children remaining. Each changed disk was exported and all one-GiB
bytes verified before the next overwrite. The LUKS2 UFS1 header confirms sector
size 512 and unchanged Argon2id memory 1,048,576 KiB. These are successful
negative controls, not writable-UFS1 passes. All 15 additional physical factors
passed: Serpent/Twofish passwords for three disk methods and AES/Serpent/Twofish
binary keyfiles for all three. The earlier AES-password physical cells retain
their 576505d identities.

The final installed desktop opened its real winit/software-rendered window;
the screenshot was visually inspected with the complete form visible. Actual
native-default gocryptfs and LUKS callbacks passed through the packaged helper,
without a test-only source overlay. Fresh strict helper file/folder preparation
passed all six methods. The seven-fixture cohort additionally includes the last
physical LUKS2/Twofish/keyfile volume. An ordinary reboot changed boot ID to
`e911aeb7-a447-4568-b1c2-ce48a5a41044` on the same updated kernel. All seven
reopened, verified prior hashes, appended and created a new file, closed normally,
reopened and verified all six hashes, then closed. The cohort records 92 complete
strict-JSON responses. HAMMER2 was initially unloaded after boot and loaded
through normal Auto reopen; it was not manually preloaded.

The physical factor's original payload is private UID0 data, while the filesystem
root belongs to UID1300 at mode 0700. The initial actual-user verifier correctly
received EACCES on that root-private tree; its failed receipt and full raw image
are retained. A separate UID1300 payload tree was written/fsynced/read and
verified across normal close/reopen and reboot, without chowning the old data.
Explicit UID0 verification covers the root-written tree. Both trees passed
post-reboot append/new-file and close/reopen hashes. A private test's additional
mode assertion on public paper files was corrected to record their actual modes;
those files remain UID1300/0644 under a private 0700 directory. Its receipt and
raw snapshot are retained, with no product/source change inferred.

The baseline additionally passed 18 Btrfs cipher/authentication/Auto variations,
36 kernel and 36 folder conformance cases, and an actual native desktop
winit/software-rendered window. Feature samples cover all ten filesystem rows
with commands and yes/no/unknown observations. CoW is not inferred false from
absent reflink. The final UFS2 feature sample confirms persistent chmod/chown;
ACLs and hardlinks remain unsupported, while atomic no-replace rename and
EEXIST collision preservation work. Feature samples are not attributed to every
crypto combination. GUI native-default coverage is distinct from the explicit
filesystem matrix through CLI/helper API; no all-filesystem GUI selector is
claimed. No crash or power-loss guarantee is inferred.

Formatter provenance was refreshed. A fresh unmodified build of
https://github.com/kusumi/makefs at
`607449619ef5f85b6a2b282e16182e342fdebeaa` reproduced the installed
makefs-freebsd SHA-256
`a03cc9ffd8f3645b37bfd72c961a11a4c2579f3030b93522f96a5618a5e0b4ce`.
Its project license is BSD-2-Clause with additional vendored notices.
https://github.com/kusumi/hammer2-utils at
`359b25bc2bb5d81e0bdbb5d48439314d80d13086`, version 0.5.0, has a BSD-3-Clause
copyright; cached archive commit metadata, source files and built formatter
match the installed executable. These host tools and drivers are not bundled.

The new one-GiB qcow2 is identified by domain `archlinux-core-01`, serial
`lurker-fresh-archlinux` and recorded guest rdev. Every destructive operation
checks identity, size, type, children and mounts. The initial HAMMER2 physical
backing probe stopped before engine execution because blkid lacks its signature;
`file` and header magic supplied independent proof after a verified raw snapshot.
A completed whole-disk exFAT format created a recursive MBR partition; the child
guard stopped the next setup before mutation, and that exact closed owned layout
was verified before clearing it. Neither was a product failure.

Complete failed ciphertext and available synthetic credentials are retained
privately. Earlier failed ciphertext with lost in-memory passwords is explicitly
unreopenable; later retaining wrappers save credentials at mode 0600. The prior
24 successful physical folder cases retained complete structured operations and
hash receipts but lost their console-log bytes when their backings were
reformatted. This limitation is retained; ZFS logs were recovered before its
format was cleared, and affected reruns preserve external logs. The retired
576505d reboot cohort's 327 files were fully archived and byte-verified before
three closed guest images were reclaimed. The completed final cohort
additionally has a complete 415-file archive with every byte/hash verified before
its three passed closed guest images were reclaimed. Transferred terminal
staging artifacts and passed closed synthetic ciphertext were reclaimed with
source/inode/resource guards. Unrelated storage, jobs and cached sources are
preserved.

Private evidence is under
`target/goal-validation/native-fresh-20261127/hosts/archlinux/`, including the
130-cell applicability ledger, exact source/build archives, package and component
manifests, driver/tool provenance, per-case operation/hash receipts, failed raw
snapshots, conformance/feature results and live checkpoint. Persistent remote
root is `/home/adjutant/.cache/lurker/native-fresh-20261127-archlinux`, exposed by
`/tmp/lurker-native-fresh-20261127-archlinux`. Final cleanup confirms no owned
mounts, loops, mappers or backend processes. The owned scratch disk and SCSI
controller were detached from both live and persistent domain configuration;
original disks/controllers match the pre-test XML. The qcow2 remains at
`/srv/build/lurker-native-fresh-20261127/archlinux/scratch-576505d.qcow2`, mode
0600, with temporary QEMU access removed and a clean qemu-img integrity check.
Its final guest raw SHA-256 is
`3e6a42aeeb53f568620e00cc32f3fdcef7db6e15e3e4f2f0f6ae1d30452dc0d5`.
The guest has approximately 1.9 GiB free after guarded cleanup. The remaining
ordinary-route limitation is the stock UFS1 writer; historical evidence limits
above remain explicit.

## Completed unprivileged follow-up and execution status

Arch-owned execution is complete and idle. A fresh read-only process/mount check
at 2026-11-27 06:28:31 UTC found no owned jobs across all UIDs, no owned mounts
and no temporary build disk. There is no remaining implementation or testing
blocker for the nine qualified filesystems excluding UFS1, and no agent task
waiting on the user, another host owner or the coordinator's final cross-host
audit. No additional build, matrix rerun or kernel experiment is queued.

The latest native functional batch finished at 2026-11-27 04:46:00 UTC using
source `e927e40bb235815343302677531d48f1afa4f939`. Its native workspace gates
passed 186 Rust tests, zero failures and 15 ignored, plus 101 Python checks.
The real pacman package `lurker 0.1.0.native20261127.e927e40-1` remains installed;
package verification reported zero altered files. All six AES/Serpent/Twofish
password/keyfile CLI lifecycles passed as UID1300 using Auto discovery and the
downloaded payload, including XFS, ext4 and Btrfs. Each completed write,
close/reopen, append/new-file, close/reopen and five-hash verification.

The package contains independent crypto protocol 4; its administrator-installed
filesystem prerequisite uses protocol 5. Three actual encrypted-host transient
FLUSH controls on XFS/Btrfs/ext4 produced caller EIO and two close refusals each,
preserving the session, raw transport identity and source lock. Original failed
ciphertext, keys, acknowledgements and logs were independently archived and
verified before separately labelled test-only manual recovery. Genuine
absent-acknowledgement crash recovery, strict helper calls, native GUI callbacks
and live CLI/helper/desktop-row close with both backend paths temporarily absent
also passed. The exact backend files were restored. The build-only disk was
normally unmounted, detached from live and persistent VM configuration and
retained privately; original disk sources match the pre-test inventory.

The configuration/Kbuild source-scope question is resolved by `7d0f183` and
`KERNEL_SCOPE_AUDIT.md`: existing upstream configuration selections and the
upstream generic-header selection are eligible integration. It is not a pending
Arch blocker. Stock UFS1 remains a separate capability gap; no novel UFS1 kernel
work or deferred experiment has started.

Private follow-up evidence is indexed in
`build/unprivileged-pilot/pilot-e927-completion-evidence-index.json`, with
controller provenance and cleanup supplements in the same directory. The
append-only `agent-terminal-status-7d0f183.json` records the final handoff and
supersedes earlier pending-eligibility wording without changing frozen indexes
or runtime identities. The historical 24 missing physical console logs and lost
early synthetic credentials remain missing evidence; this follow-up does not
reconstruct them or claim a new full matrix or reboot run.
