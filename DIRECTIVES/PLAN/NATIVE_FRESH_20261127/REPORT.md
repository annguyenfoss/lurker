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
  frozen-candidate seven-filesystem container/device coverage is in progress;
  desktop partition controls remain a separate validation step.
- CryFS initialization: `3178144` uses the private runtime for the temporary
  plaintext mount on every platform. The macOS APFS backing regression passed.
  Other backing failures remain independently tracked.
- FreeBSD CryFS: `eface5b` adds the pinned native private build/stager and its
  verified runtime/license closure. `1c62f1f` exposes the route in the core and
  desktop capability model; scoped native core/desktop tests passed. The full
  backing-filesystem lifecycle round is still required.

## Work still required

Host owners are performing current update inventories, builds, applicable
container combinations, native controls and device preparation. Initial product
builds use `8adcdcf` or the recorded later script-only revision; record harness
and independent engine identities separately rather than rebuilding unchanged
product inputs or relabeling old engines as new builds.

NetBSD CGD physical dispatch is implemented. FreeBSD, OpenBSD and DragonFly
physical create dispatch and BSD character-device classification still need
integration and native validation. Native controls inform those adapters;
file-container results cannot close the device requirement.
DragonFly has newly demonstrated writable host-local FUSE controls that its
current adapter does not yet expose. On macOS, the owner is investigating an
eligible macFUSE VeraCrypt route after loading the installed driver without a
reboot; a FUSE-T SMB mount cannot satisfy the network-filesystem exclusion.

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
