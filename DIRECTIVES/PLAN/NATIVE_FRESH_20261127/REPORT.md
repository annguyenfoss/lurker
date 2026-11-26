# Fresh native qualification status

Round: native-fresh-20261127. Status: in progress.

No previous result is accepted as a fresh pass. The acceptance contract and host
ownership are BRIEF.md and HOSTS.md. Private case receipts and checkpoints live
under target/goal-validation/native-fresh-20261127/.

## Verified initial milestones

- Documentation/handoffs: commit `8adcdcf`; all eleven host owners dispatched.
- Common payload: downloaded Bitcoin, Ethereum and Transformer papers plus text,
  with original URLs, sizes and SHA-256 values retained privately in
  `payload/manifest.json` under the evidence root. The papers are test data,
  not committed source. Sources:
  https://bitcoin.org/bitcoin.pdf
  https://ethereum.github.io/yellowpaper/paper.pdf
  https://arxiv.org/pdf/1706.03762
- Common lifecycle harness: `c5feaec` adds `--payload-dir`, rejects missing/invalid
  inputs and records all original input hashes. Its three focused Python tests
  passed on updated Arch with managed Python 3.15; retained output, interpreter
  identity and harness hashes are in
  `hosts/archlinux/build/shared-payload-tests.log`. Linux batch forwarding is
  `88e279c`. These checks establish harness behavior, not native storage support.
- Fedora native filesystem controls: coordinator retrieved and inspected
  `hosts/fedora/cases/native.json`. All ten fresh filesystem cases recorded
  successful write/close/reopen/append/close/reopen, original hashes equal to
  the pinned common payload, final hashes equal to the append-phase hashes,
  and no remaining mounts. These are unencrypted native controls, not Lurker
  encryption passes. UFS1 used an existing write-enabled external module and
  must not be presented as stock Fedora support. Quick feature observations
  remain qualified by actual filesystem settings and probe semantics.
- Harness variations: `22c7ecb` adds cipher, generated private binary-keyfile and
  Auto reopen options; focused checks passed remotely on Ubuntu. `cfcfd5f`
  preserves the actual Transformer filename in the mounted-payload conformance
  suite, with legacy Lamport compatibility. Both payload-name routes passed
  write/append/verify on separate actual openSUSE mounts with normal cleanup.
- First product fix: `a815e10` allows explicit filesystem selection through
  existing privileged Linux LUKS1/LUKS2/VeraCrypt device creation and existing
  macOS APFS partition creation. Remote formatting and focused core/CLI tests
  passed on Arch. Physical-device runtime qualification is still pending.
- Bounded native I/O: `45e5b9e` isolates payload syscalls in owned workers and
  records any worker that cannot be reaped. `aecdcde` records Linux mount and
  backing identities and rejects requested/observed filesystem mismatches
  before payload writes. Eight unit tests and actual mounted positive/negative
  controls passed remotely. Generic FUSE and UFS labels explicitly require
  separate driver/superblock evidence.
- NetBSD physical CGD: `a38fc83` creates and reopens checked GPT data wedges,
  records native device identity in the existing journal and preserves mandatory
  parameters by partition UUID under `/var/db/lurker-cgd/`. Native core tests
  passed (50 passed, 4 ignored). The preview's 11 CLI operations covered create,
  explicit/Auto opens, no-force unchanged-byte rejection, duplicate/busy/wrong
  credentials and source close, plus helper/native inventory assertions. The
  coordinator inspected the actual receipt and matched original and final
  payload hashes, no-force preservation and clean vnode/CGD inventories. Full
  seven-filesystem container/device coverage passed on the subsequent recorded
  candidate. `5b9939e` adds the native desktop partition fields; its ordinary-user
  helper callback and 100%/120% layout probes passed. The final source-specific
  rerun and receipt audit remain with the NetBSD owner.
- CryFS initialization: `3178144` uses the private runtime for the temporary
  plaintext mount on every platform. The macOS APFS backing regression passed.
  Other backing failures remain independently tracked.
- FreeBSD CryFS: `eface5b` adds the pinned native private build/stager and its
  verified runtime/license closure. `1c62f1f` exposes the route in the core and
  desktop capability model; scoped native core/desktop tests passed. The full
  backing-filesystem lifecycle round is still required.

## Subsequent fixes and native controls

- Linux LUKS2/UFS1 geometry: `93e0630` selects 512-byte logical sectors only for
  UFS1, preserving cipher, key size and KDF costs, including replacement of the
  usual Twofish sector argument. Arch core tests passed (83 passed, 2 ignored)
  and CLI/helper builds passed. Fedora's prior native AES/Argon2id 512-sector
  control passed the payload lifecycle. The exact changed product now passed
  all four AES/Twofish container/device payload regressions on Fedora. Header
  inspection confirmed 512-byte sectors, 64-byte XTS keys and unchanged
  Argon2id memory costs. Stock UFS write limitations remain explicit prerequisites.
- Linux CryFS publication: `166266f` uses native exclusive rename with the
  existing durability barriers; `195b56b` registers the Linux-only patch. The
  Debian-built portable engine's archive SHA-256 is
  `fa5e8f14e008b0df5bb5faff6247bbac0810af4e04c32dc1d074a5f9e5744db2`.
  Fresh affected UFS2/exFAT product payload regressions passed on Debian,
  Ubuntu and Gentoo under their separately recorded Rust component identities.
  Namespace correctness and backing-device fsync forwarding are separate host
  driver qualifications; a payload comparison alone proves neither barrier.
- FreeBSD physical native controls now pass for GELI and VeraCrypt on distinct
  checked GPT scratch partitions. `ff9f312` and `3130c21` add immutable GEOM
  inspection, separate idle guards and active-provider identity verification.
  `e116dcc` plus the required cross-platform fallback correction `b76d730` add
  product GELI device creation, filesystem selection and BSD CLI device-node
  classification. Native core tests passed (65 passed, 11 ignored), CLI tests
  passed (19 passed), and the actual device preview passed 14 operations. The
  coordinator matched all original payload hashes and final appended hashes,
  verified the invoking user's private filesystem root and clean final mount
  inventory. `2e93f78` adds desktop GELI partition controls. The actual UI exposed
  a pre-existing scroll-area sizing defect at 120% zoom, fixed in `ca14341`.
  Native desktop tests passed (35 product tests and one private software probe);
  ordinary-user helper/GUI execution passed. The coordinator inspected both
  120% scrolled screenshots and matched all five production source hashes to
  the executed inputs. Full device-matrix and installed-package gates remain.
- FreeBSD CryFS backing qualification: ext4, UFS1, UFS2, HAMMER2 and ZFS passed.
  Both NTFS and exFAT passed equivalent unmodified upstream CryFS payload
  controls, while the bundled durability patch failed during hardlink
  publication. Native no-replace rename controls show the stock FreeBSD FUSE
  layer rejects rename flags before calling either driver. This is an unresolved
  bundled-engine/host-primitive gap, not an upstream encryption failure or an
  inapplicable filesystem. Any custom kernel-module experiment is deferred
  until the ordinary routes have been fully assessed and reported.
- OpenBSD native CRYPTO/RAID1C device controls passed all eight applicable
  filesystem combinations. The private VeraCrypt device discovery and native
  whole-disk-alias guard are implemented in `6ab0f8b`; block, raw and symlink
  whole-disk rejection preserved the checked headers, and a real partition
  passed the full payload lifecycle. Shared product physical wiring is underway.
- macOS `175ea1c` adds the native exclusive CryFS publication primitive, and
  `065eaa2` registers its independent patch input. The patched engine passed
  direct UFS2 and APFS payload regressions. `383c47b` adds the core's anchored
  directory-publication fallback for native UFS2, preserving directory xattrs,
  exclusive file moves, durability barriers and configuration-last recognition.
  Four native unit tests and UFS2 CryFS/Cryptomator, APFS both engines and exFAT
  Cryptomator payload regressions passed. Existing normal rename behavior on
  other filesystems is retained. exFAT CryFS's engine-primitive gap remains open.
- openSUSE's actual login-user CLI/helper regression passed with the narrow sudo
  authorization probe change; the machine's `sudo -n -v` failure and
  `sudo -n true` success were preserved. `7d8827e` independently fixes the test
  harness's terminal echo-readiness race. Production authorization and harness
  synchronization have distinct receipts.
- NetBSD's final workspace and seven-container/seven-device batch passed on
  `dac5010`. The subsequent mounted-reboot batch exposed native ZFS import
  refusal. A persistent nonzero `kern.hostid` did not fix it: source inspection
  identified a native macro that shadows the global host identity with a local
  zero value. The stock service does not export pools during shutdown. Failed
  pools are preserved; no forced import, ineffective preflight guard or custom
  shutdown hook is used. A module-source correction remains deferred until the
  ordinary assessment is complete. Native package installation, packaged helper
  and actual desktop-window checks passed; an installed-artifact matrix rerun
  is still required because its binaries differ from the earlier matrix build.
- All eleven applicability ledgers now contain 130 unique normalized cells.
  The coordinator's private structural audit verifies coverage of the requested
  method/filesystem names only. It does not establish runtime completion or
  the correctness of any availability claim; source-build gaps and remaining
  native qualifications continue to be audited individually.
- Linux core/CLI/helper candidate `ca7d213` is frozen for all six distributions.
  Full workspace checks and the first installed native package gates have
  passed on several hosts; complete final container/device/reboot sweeps are
  running. The subsequent UFS1 negative cases exposed a mountinfo parsing gap:
  per-mount `rw` does not override superblock `ro`. The parser and payload-gate
  correction are being tested before any completed Linux qualification claim.

## Current candidate qualifications

The six Linux follow-up sweeps use `576505d`, including the mountinfo correction:
a filesystem is read-only if either its VFS flags or its superblock flags say so.
An actual encrypted UFS1 mount exposed `rw` VFS flags with a `ro` superblock;
create and Auto reopen now reject that state without publishing new storage.
The fresh full sweeps supersede the earlier `ca7d213` candidate only for cases
actually rerun. Debian and openSUSE have completed all 60 configured-host
container cells;
their UFS1 uses pre-existing non-stock write-enabled modules. Ubuntu and Gentoo
have completed 54 writable container cells each, with stock UFS1 limitations
recorded separately. Physical-device, conformance and mounted-reboot batches
remain in progress across the six distributions. Resource-pressure and harness
failures retain their original receipts and require explicit rerun results.
The coordinator independently inspected 222 successful embedded receipts from
Arch, openSUSE, Ubuntu and Gentoo: pinned inputs equaled initial writes, appended
data equaled final reads, the new file existed, all seven operations succeeded
and the plaintext mount was closed. The private
`linux-576-payload-receipt-audit.json` records source hashes and individual checks.
That audit does not establish driver provenance, application conformance,
physical-device coverage, reboot persistence or package completion. Fedora has
also reconciled its eight capacity-related retries to 60 successful container
cells; its original ENOSPC failures remain preserved.

macOS candidate `3bfed46` includes native Apple encrypted-image ext4/NTFS/ZFS
integration and guarded VeraCrypt external GPT data partitions. Its workspace
checks passed (144 tests, no failures, 12 ignored) and its DMG was built and
verified. Extracted-package qualification remains in progress. The native
desktop callback and 100%/120% form snapshots passed before the freeze. A third
small Toshiba sample passed the VeraCrypt payload lifecycle and restored native
device ownership after normal close. Buran has not been rebooted.

NetBSD's installed CGD artifact has now passed all seven container and seven
device filesystem cases, plus four representative mounted-reboot cases. This
does not remove the separate upstream ZFS reboot limitation. `7d00998` adds a
native process identity reader with live/zombie/reaped controls. The CryFS core
preview has passed a login-user PDF write/close/reopen/append/reopen lifecycle
using host ReFUSE. SQLite WAL fails because native shared writable mapping
returns `EOPNOTSUPP`; the stock ReFUSE implementation disables the puffs page
cache. The failed fixture and native mapping control are preserved. No cache or
durability requirement has been weakened. Factory packaging, desktop checks
and the remaining backing-filesystem combinations are not complete.

OpenBSD physical CRYPTO, RAID1C and VeraCrypt have each passed their four
applicable filesystem lifecycles. An additional RAID1C fixture passed explicit
one-member recovery. Native desktop physical forms are under validation.
DragonFly has booted its updated stock world/kernel and passed native physical
controls; product physical creation is still under integration.

BSD SDK/stager changes `782a088`, `88d9315` and `c7f7092` keep FUSE on the host
and validate its ABI instead of packaging it privately. FreeBSD's fresh package
passed workspace checks and install/delete/reinstall checks, but a runtime
symlink-alias comparison defect was found during the native regression. Fix
`d601bc5` validates the resolved provider through the trusted host ABI alias and
has a native versioned-provider/alias-update regression. New artifact
qualification is required before that package is accepted.

FreeBSD Btrfs/XFS controls using a separately installed in-process LKL FUSE
driver passed with direct I/O. They remain a separate eligibility assessment:
LKL embeds Linux kernel code in a native process. They do not establish native
product support, and new product integration is held pending the user's
clarification about that architecture.

## Work still required

Host owners are performing current update inventories, builds, applicable
container combinations, native controls and device preparation. Initial product
builds use `8adcdcf` or the recorded later script-only revision; record harness
and independent engine identities separately rather than rebuilding unchanged
product inputs or relabeling old engines as new builds.

NetBSD CGD, FreeBSD GELI/VeraCrypt and OpenBSD CRYPTO/RAID1C/VeraCrypt physical
dispatch are implemented with native proofs. Their final packaged matrices
remain distinct gates. DragonFly physical product dispatch is being qualified.
Native controls inform those adapters; file-container results cannot close the
device requirement. DragonFly's native NTFS/exFAT/ext4 adapters pass applicable
file-container payload flows with the documented host prerequisites. macOS uses
native macFUSE VeraCrypt and native hdiutil foreign-filesystem routes; final
extracted-package validation remains open.

DragonFly's discarded manual mount-label prototype caused a kernel panic by
violating the native FUSE vnode close contract. A fresh unmodified-helper
failed-mount control subsequently closed normally without a panic. This is
evidence against presenting the prototype failure as an upstream FUSE defect.
The owner is testing a corrected adapter using the actual FUSE device vnode.

Initial Linux payload completions were found insufficient for requested-format
qualification: on Debian, Gentoo and Ubuntu, resolving `mkfs.ext4` to `mke2fs`
could create ext2. Those specific receipts are reclassified as mismatches;
verified actual ext4 observations on other hosts are retained. The explicit
ext4 formatter fix includes follow-up `f22ee9f`; do not use its earlier
test-import-incomplete predecessor alone. Kmod applet dispatch, NTFS mount
readiness and native NTFS3 preference are also separate tested Linux fixes.

Host prerequisite fixes are not bundled filesystem stacks. The source-pinned
FUSE-UFS no-replace correction (`94da64c`) has native unit, collision/rename and
payload evidence on several Linux hosts. Configured native UFS kernel routes
remain distinct from stock distribution modules. NetBSD NTFS now has a passing
native control with the required socket-buffer setting; matching product runs
are in progress. macOS NTFS-3G/macFUSE supports the directory workloads that its
other installed driver could not support privately. Exact prerequisites and
remaining limitations belong to each host's report.

macOS native XFS failed persistence qualification and is guarded before writable
operations (`8a401a3`); an automatic network-filesystem fallback is not used.
The authorized Toshiba USB samples for APFS encryption and Cryptomator on exFAT
passed and closed normally. Buran was not rebooted.

No complete host matrix, final candidate validation, scratch-device qualification,
whole-round cleanup or deferred customized-kernel assessment is claimed yet.
The round remains active. Per-host reports and private receipts are authoritative
for their explicitly recorded revisions and scopes.
