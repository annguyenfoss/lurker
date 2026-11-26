# Ubuntu fresh qualification — work in progress

Owner: Ubuntu host agent, exclusive `adjutant@ubuntu-core-01`.
Product baseline: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
Private evidence: `target/goal-validation/native-fresh-20261127/hosts/ubuntu/`.

Ubuntu 26.10 is running `7.3.0-11-generic`, boot
`09e433ed-a944-4a3d-809c-bf5f81f3e0a2`. Fresh apt index refresh and full upgrade
completed, followed by ordinary `linux-generic`, matching headers, filesystem
tools, ACL tools and APFS DKMS installation. The combined update/install job
returned zero; subsequent full-upgrade simulation reports no pending upgrades.
No reboot-required marker was present. APFS DKMS 0.3.21 and Ubuntu ZFS 2.4.4
load against the running kernel. Stock UFS write configuration is disabled;
UFS1 writable qualification remains unresolved. UFS2 has a separately installed
FUSE driver requiring fresh control and product tests.

Fresh exact-source workspace check, workspace test and workspace build run:
`20261126T203534Z-2d1d70ae` (one Cargo worker). `cargo check --workspace`
passed in 4m53s. Workspace tests passed **147, failed 0, ignored 13**; the
workspace binary build also passed. Runtime qualification remains incomplete. Payload copies use the round's downloaded Bitcoin, Ethereum and
Transformer PDFs and text. Runtime, container matrix, device matrix, reboot
persistence and final-candidate checks remain incomplete.

Current-source manifest verification accepted the cached private gocryptfs,
CryFS and Cryptomator inputs and complete runtime hashes. These are reusable
build inputs only, with fresh runtime tests still required. Native VeraCrypt
creator source/archive/patch/binary/private-library verification passed
separately from the LKL helper; the
LKL filesystem route does not count toward native acceptance.

Source candidates for ordinary external drivers are pinned to HAMMER2
`09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0` (1.0.112, BSD-3-Clause) and
fuse-ufs `ee5c4a447da98220a11edb69a225d51379afc4fb` (0.6.0, BSD-2-Clause).
Both advertise experimental write routes suitable only for the authorized
scratch-media assessment; fresh matching builds and runtime tests are queued.
Sources: https://github.com/jdmanring/linux_hammer2 and
https://github.com/realchonk/fuse-ufs

The initial UFS1 LUKS1/VeraCrypt observations used an inherited custom write
module and are **excluded from ordinary-route qualification**. Its loaded
source version `7932A71D9C37C40AAC2BEE3` matched the old cached custom module,
while the stock module is `201E180B8E6BE4EDF81A513`. With zero references and no
UFS mounts, the owner unloaded that module and loaded the stock module. The
stock loaded/installed versions now match and its module taint is empty.
`updates/ufs-stock-restoration.json` preserves the evidence. Fresh stock UFS1
retests and controls are still required. APFS, ZFS and HAMMER2 loaded versions
also match their currently installed modules. No custom kernel was built.

The required applicability ledger is ten filesystems × thirteen methods.
LUKS1, LUKS2 and VeraCrypt require independent inner-filesystem cases; gocryptfs,
CryFS and Cryptomator require independent backing-filesystem cases. Apple's
hdiutil/diskutil, GELI, PEFS, softraid CRYPTO/RAID1C and CGD are outside the Linux
native adapter scope and must each retain an explicit not-applicable entry.
No runtime cell has yet been credited as passed in this fresh round.

The fresh CLI SHA-256 is
`44fe7d0ff20f5e780c3c98a5fc837f680012fadc216f739806cdc5ddd5175a1a`;
helper SHA-256 is
`e1dcf92659a19c9115fa0bee2cde6b6b4c52fa9c93007995577f43184d1632a6`.
Full identities and checks are in the private `build/result.json`.

Next: complete the queued matching driver builds, execute ordinary containers including
native controls for missing capabilities, batch demonstrated fixes, then attach
and identify a new `lurker-fresh-ubuntu` qcow2 for device cases. Existing disks,
fixtures, sessions and installed Lurker artifacts are preserved.
