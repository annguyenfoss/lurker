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
`20261126T203534Z-2d1d70ae` (one Cargo worker). This is still running; it is not
a pass. Payload copies use the round's downloaded Bitcoin, Ethereum and
Transformer PDFs and text. Runtime, container matrix, device matrix, reboot
persistence and final-candidate checks remain incomplete.

Current-source manifest verification accepted the cached private gocryptfs,
CryFS and Cryptomator inputs and complete runtime hashes. These are reusable
build inputs only, with fresh runtime tests still required. Native VeraCrypt
creator verification is being assessed separately from the LKL helper; the
LKL filesystem route does not count toward native acceptance.

The required applicability ledger is ten filesystems × thirteen methods.
LUKS1, LUKS2 and VeraCrypt require independent inner-filesystem cases; gocryptfs,
CryFS and Cryptomator require independent backing-filesystem cases. Apple's
hdiutil/diskutil, GELI, PEFS, softraid CRYPTO/RAID1C and CGD are outside the Linux
native adapter scope and must each retain an explicit not-applicable entry.
No runtime cell has yet been credited as passed in this fresh round.

Next: finish exact-source build, execute all ordinary container cases including
native controls for missing capabilities, batch demonstrated fixes, then attach
and identify a new `lurker-fresh-ubuntu` qcow2 for device cases. Existing disks,
fixtures, sessions and installed Lurker artifacts are preserved.
