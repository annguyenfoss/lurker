# Arch Linux fresh native qualification

Status: final follow-up running. No prior-round runtime receipts count. All
applicable container and physical baseline cells are reconciled; stock UFS1 is
blocked by its read-only driver. Physical cipher variations, final desktop and
reboot gates remain in progress.

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
as a selectable inner format. LKL filesystem routes are excluded from native
acceptance.

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
negative controls, not writable-UFS1 passes. Physical cipher/keyfile factors,
final window/default callbacks, fresh helper file cohort and ordinary reboot
verification are still running or queued.

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
three closed guest images were reclaimed. Only fully transferred terminal
staging artifacts and successful closed synthetic ciphertext have otherwise
been reclaimed. Unrelated storage, jobs and cached sources are preserved.

Private evidence is under
`target/goal-validation/native-fresh-20261127/hosts/archlinux/`, including the
130-cell applicability ledger, exact source/build archives, package and component
manifests, driver/tool provenance, per-case operation/hash receipts, failed raw
snapshots, conformance/feature results and live checkpoint. Persistent remote
root is `/home/adjutant/.cache/lurker/native-fresh-20261127-archlinux`, exposed by
`/tmp/lurker-native-fresh-20261127-archlinux`. Final reboot, cleanup and scratch
detachment are pending; this report does not claim completed qualification.
