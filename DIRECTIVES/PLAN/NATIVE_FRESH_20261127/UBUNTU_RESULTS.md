# Ubuntu fresh native qualification — completed

Ubuntu qualification completed on the installed native package built from Rust
source `67894f68525c55d5cbb48c743e1a98a308bd970f`, with Linux ELF producer
`7d8f9a4` and stager consumer `fa063ac` recorded separately. All current positive
filesystem receipts execute this installed candidate. Historical baseline and
576 receipts retain their original identities.

| Scope | Passed | Unavailable | Not applicable |
| --- | ---: | ---: | ---: |
| Container / native filesystem backing, 10 × 13 cells | 54 | 6 | 70 |
| New qcow2 device / native partition backing, 10 × 13 cells | 54 | 6 | 70 |

Each positive cell covers actual read-write mounts, downloaded Bitcoin,
Ethereum and Transformer PDFs plus text, normal close, reopen/hash verification,
append and creation of a second text file, another reopen/hash verification and
normal closure. Directory engines are identified as encryption on a native
filesystem backing. Physical backing receipts use an actual scratch partition.
LKL compatibility routes are outside these OS-native counts.

## Installed artifact and validation

- Native package: `0.1.0+native20261127.67894f6`, installed successfully through
  dpkg. No prior system Lurker package or `/usr/lib/lurker` payload existed.
- Debian package SHA-256:
  `21dc7691ab777ce062ab104ef3ce47a97bed0337572f51947c14733e175803b7`.
- Staged archive SHA-256:
  `c431fac8feaa390ac756c43c0e23019881d914f2a58c760a17e219a72ed39830`.
- CLI SHA-256:
  `dca53903837f75aef85a277837dfbc17a49b753825cf88e8721a5f096f7f35e0`.
- Helper SHA-256:
  `6b6cb0a14c38973605289563d37ed12b2d57130537ecd1044e3446c4058a5487`.
- Desktop SHA-256:
  `842b395c950fb4952210019d9433c0c05c56cdf84507ecfc5839701750e9af87`.

Exact-source workspace check/test/build passed **168 tests, zero failures,
15 ignored**; the explicitly enabled native kmod regression separately passed.
Committed producer/stager/native-package Python checks passed **73/73**.

Normal-user password and binary-keyfile CLI file lifecycles passed for all three
disk methods, including terminal survival. The six flows also passed through
`/usr/bin/lurker` after dpkg installation, explicitly exercising installed helper
discovery and the loop lease; those receipts are separate from the earlier
byte-identical private-prefix runs. Strict full-stdout helper JSON and
full-payload samples passed on files and physical partitions for all three disk
methods. Physical helper tests also verified unchanged block-node ownership and
modes, and decrypted roots owned by UID 1300 with mode 0700. All twelve additional
physical Serpent/Twofish password/keyfile variations passed. Earlier fresh
cipher, wrong-credential, duplicate/alias, Auto and busy-close conformance receipts
remain source-specific; the unchanged branches are identified explicitly.

The actual installed desktop opened a real X11/winit software-rendered window.
Default GUI-to-installed-helper callback lifecycles passed all six Linux
backends. Callback fixtures use the exact-source test binary; the shipped window
probe and installed helper identities are recorded separately. The two temporary,
unshipped qualification scripts were removed from the system package prefix.

## Persistence and resources

All **54 container/backing fixtures passed nine ordinary OS reboots**, one
filesystem cohort of six at a time. All **three physical Btrfs partitions** then
passed a tenth ordinary reboot with binary keyfiles, Auto reopen, all five hashes
and normal closure. Every cohort records its mounted preboot inventory, both boot
identities, unchanged product/harness/host-tool hashes and normal postboot close.
No forced recovery or encryption-profile changes were used.

The 4 GiB guest has no swap. A measured six-fixture pilot observed approximately
133 MiB steady folder-daemon RSS and a released CryFS startup/KDF peak near
1 GiB. The preserved initial projection incorrectly multiplied transient
high-water marks across mounted folders. Reviewed admission separates steady
RSS from one sequential 1,120 MiB operation peak, includes a native driver budget
and retains a 1 GiB reserve. Larger 12/18/54 cohorts failed conservative admission;
only six-fixture cohorts were armed. Live create/open checks enforce that reserve.
All 18 file-cohort preparation/verification resource receipts passed; their
minimum available RAM was **2,133,440 KiB**.

Initial boot: `09e433ed-a944-4a3d-809c-bf5f81f3e0a2`.
Final boot: `e4d75e19-6027-47ff-9b33-6f4add5a81f3`.
The complete ten-reboot chain is retained in the ledgers. Controller syntax and
SSH-polling failures were preserved and corrected without repeating live cases.
The first physical reboot pilot's symlink create targets were correctly rejected
before encryption. Its receipts/layout/credentials remain intact; the successful
follow-up reused the same verified partitions, created through canonical nodes
and retained PARTUUIDs for reboot identity.

## Ordinary host prerequisites and remaining limitation

The host completed ordinary apt updates and runs Ubuntu 26.10, kernel
`7.3.0-11-generic`. Native prerequisites include cryptsetup `2:2.8.7-1ubuntu3`,
APFS DKMS `0.3.21-1ubuntu1`, apfsprogs `0.2.1-4` and Ubuntu OpenZFS
`2.4.4-1ubuntu4`, with matching loaded/installed modules. Cold APFS and HAMMER2
loading passed. HAMMER2 uses Linux-port source
`09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0`; a fresh hammer2-utils build at
`359b25bc2bb5d81e0bdbb5d48439314d80d13086` reproduced the installed formatter.
Fresh unmodified makefs source `607449619ef5f85b6a2b282e16182e342fdebeaa`
replaced an older unmanifested formatter; affected UFS2 cases were retested.
Brief journaling, CoW, ACL, permissions and ownership observations are retained;
unsupported or unavailable feature queries remain explicit.

UFS2 uses upstream fuse-ufs
`ee5c4a447da98220a11edb69a225d51379afc4fb` plus the reviewed NOREPLACE,
file/directory barrier and READDIR continuation prerequisite patches. Upstream
format/lint, 28 library tests, native rename/collision, fsync/EIO/retry and
512-then-513-entry pagination controls passed. The old driver listed only 204
of 512 entries and omitted the new file. Installed driver SHA-256 is
`78c7e182b87d1e7ad7d3ba21ad49d79444e55db70c9a740788efff57a936b7cb`.
Its clean source at `51f0f3cc5c1c68b2b1e93d23cfdd6e0d720c841f`, exclusive
UFS2 magic gate, actual process executables and superblock VERSION=2 evidence are
bound independently of generic FUSE labels. All twelve affected UFS2
container/device disk/folder cells passed with this prerequisite.

**UFS1 read-write remains unavailable on this ordinary Ubuntu kernel.**
`CONFIG_UFS_FS_WRITE` is unset. The stock module has source version
`201E180B8E6BE4EDF81A513` and empty taint. All three physical creates correctly
rejected read-only new filesystems and closed their mappers; the raw VERSION=1
partition control mounted read-only, so its three directory-engine cells were
not attempted. Complete failed encrypted images and the raw backing image are
retained with SHA-256 identities. No custom kernel/write module was built.

The initial fresh baseline contained four excluded raw passes: two requested
ext4 cases actually mounted ext2, and two inherited an old custom UFS write
module. Those receipts are retained and never counted as current qualification.
The final candidate includes the formatter/module invocation, read-only guard,
physical filesystem API and sudo-rs loop-lease process-group corrections. The
latter preserved session/TTY authorization; exact-source password/keyfile controls
and 168 process samples proved a separate background group and no stopped tasks.

The native creator now uses administrator-controlled host FUSE, verified native
architecture/SONAME and actual loader resolution. Creator bytes remain
`7975cc4724d558fd10bafcc670627c56959a1c76d721f7f83dce855eb0052769`;
private zlib and complete static-wx notices remain verified. Host ABI4 provenance
and current provider hashes are separate; compatible administrator updates are
accepted. No FUSE runtime or filesystem driver is bundled in the new package.
Installed notices were verified at the stager's declared package relocation paths.

Prerequisite sources:
https://github.com/jdmanring/linux_hammer2
https://github.com/kusumi/hammer2-utils
https://github.com/kusumi/makefs
https://github.com/realchonk/fuse-ufs

## Retained evidence and final state

Local evidence: `target/goal-validation/native-fresh-20261127/hosts/ubuntu/`.
Key receipts are `cases/applicability.json`, `device/applicability.json`,
`validation/reboot-cohorts/ledger.json`, `device/physical-reboot.json` and
`validation/final-owned-resource-audit.json`. Complete source/package manifests,
component hashes, command logs, feature observations and preserved failure
identities remain alongside them. Remote evidence is physically persistent at
`~/.cache/lurker/native-fresh-20261127-ubuntu/fixture-storage`.

The separate 16 GiB evidence disk has serial `lrkf-fixture-ubuntu`, ext4 UUID
`0656b0c1-ede9-4986-98f4-cbe2c3f0d0c9`, host source
`/srv/build/lurker-native-fresh-20261127/ubuntu-fixture-storage-20261127.qcow2`.
The new 1 GiB destructive scratch has serial `lurker-fresh-ubuntu`, target vdc,
host source `/srv/build/lurker-native-fresh-20261127/ubuntu-scratch-20261127.qcow2`.
Domain UUID, source, size, serial and guest by-id identity were checked before
formatting. Both disks and private 0600 test credentials are retained for review;
unrelated work and storage remain preserved.

Final audit passed all **1,105 package inventory entries**, both public launchers
and `dpkg -V`. There are **zero owned plaintext/backing mounts, loops, crypto
mappers or backend/helper processes**. Only the identified evidence-storage
filesystem remains mounted. Scratch partitions are encrypted and closed. Root
free space is approximately 2.4 GiB and auxiliary free space 3.4 GiB. Complete
package archives and failed fixtures remain; only hash-verified duplicate passed
artifacts and zero ranges with unchanged logical SHA-256 were reclaimed.
