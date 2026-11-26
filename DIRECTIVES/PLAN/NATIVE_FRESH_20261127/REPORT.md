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

## Work still required

Host owners are performing current update inventories, builds, applicable
container combinations, native controls and device preparation. Initial product
builds use `8adcdcf` or the recorded later script-only revision; record harness
and independent engine identities separately rather than rebuilding unchanged
product inputs or relabeling old engines as new builds.

The shared BSD lifecycle currently rejects physical devices, and BSD character
device classification also needs review. Native device controls must inform
adapter fixes; file-container results cannot close the device requirement.
DragonFly has newly demonstrated writable host-local FUSE controls that its
current adapter does not yet expose. On macOS, the owner is investigating an
eligible macFUSE VeraCrypt route after loading the installed driver without a
reboot; a FUSE-T SMB mount cannot satisfy the network-filesystem exclusion.

No complete host matrix, final candidate validation, scratch-device qualification,
whole-round cleanup or deferred customized-kernel assessment is claimed yet.
The round remains active. Per-host reports and private receipts are authoritative
for their explicitly recorded revisions and scopes.
