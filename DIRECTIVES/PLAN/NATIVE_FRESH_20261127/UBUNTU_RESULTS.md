# Ubuntu fresh native qualification — in progress

The fresh baseline and corrected host-prerequisite followups currently qualify
**51 of 60 applicable filesystem/encryption cells** for the required PDF/text
write, normal close, reopen, append and final-hash lifecycle. Seventy further
cells are explicitly not applicable to Linux's native routes. Final-candidate,
cipher/keyfile/conformance, destructive-device and OS reboot validation remain
outstanding. The private 130-cell ledger is
`target/goal-validation/native-fresh-20261127/hosts/ubuntu/cases/applicability.json`.

| Filesystem | LUKS1 | LUKS2 | VeraCrypt | gocryptfs backing | CryFS backing | Cryptomator backing |
| --- | --- | --- | --- | --- | --- | --- |
| ext4 | Actual ext2; retest | Actual ext2; retest | Actual ext2; retest | Pass | Pass | Pass |
| Btrfs | Pass | Pass | Pass | Pass | Pass | Pass |
| XFS | Pass | Pass | Pass | Pass | Pass | Pass |
| APFS | Pass | Pass | Pass | Pass | Pass | Pass |
| UFS1 | Stock read-only | Stock read-only / sector policy | Stock read-only | Stock read-only | Stock read-only | Stock read-only |
| UFS2 | Pass | Pass | Pass | Pass | Pass with fresh engine | Pass with patched driver |
| HAMMER2 | Pass | Pass | Pass | Pass | Pass | Pass |
| NTFS | Pass | Pass | Pass | Pass | Pass | Pass with native ntfs3 |
| exFAT | Pass | Pass | Pass | Pass | Pass with fresh engine | Pass |
| ZFS | Pass | Pass | Pass | Pass | Pass | Pass |

For each filesystem, hdiutil Apple disk-image encryption and diskutil APFS
encryption require the macOS native adapters; GELI/PEFS require FreeBSD's native
facilities, CGD requires NetBSD, and softraid CRYPTO/RAID1C require OpenBSD.
Those seven method/OS exclusions are recorded individually in the ledger.
Directory columns name the native backing filesystem, rather than an alternate
inner format.

Host: exclusive `adjutant@ubuntu-core-01`, Ubuntu 26.10, running
`7.3.0-11-generic`, initial boot `09e433ed-a944-4a3d-809c-bf5f81f3e0a2`.
A fresh apt refresh/full upgrade and ordinary kernel-meta/header/tool installation
completed successfully; subsequent full-upgrade simulation had no pending
upgrades and no reboot-required marker. No custom kernel was built.

Baseline product source is `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
Exact-source workspace run `20261126T203534Z-2d1d70ae` passed check, tests and
build with one Cargo worker: **147 tests passed, 0 failed, 13 ignored**.
CLI SHA-256 is
`44fe7d0ff20f5e780c3c98a5fc837f680012fadc216f739806cdc5ddd5175a1a`;
helper SHA-256 is
`e1dcf92659a19c9115fa0bee2cde6b6b4c52fa9c93007995577f43184d1632a6`.
Current-source manifests verified the reused private engine build inputs and
runtime bytes. The new CLI/helper were tested from an administrator-controlled
private prefix; system-installed Lurker was preserved. These are staged-artifact
results, with no installed-system-package claim.

Ordinary native prerequisites include Ubuntu APFS DKMS 0.3.21 and ZFS 2.4.4,
matching the running kernel. The newly built HAMMER2 module uses upstream
`09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0` (1.0.112, BSD-3-Clause).
The fuse-ufs host driver uses upstream
`ee5c4a447da98220a11edb69a225d51379afc4fb` (0.6.0, BSD-2-Clause), Fedora's
reviewed FUSE_RENAME2/destination-check patch, and a FreeBSD-only test-import lint
fix. Its local source commit is
`aeaa67a2cdce89432e00de27d306d527f975004d`; installed SHA-256 is
`48f7cf2d19938a39ba18e767c573d9d43bd7ce11c0283b5ebe7cacbbb52f6f0e`.
Twenty-eight library tests, upstream `make prepare`, the prescribed Makefile
build, native no-replace rename/collision controls and full payload reopens
passed. The temporary nightly formatter was removed after checks. Drivers remain
host prerequisites. Sources:
https://github.com/jdmanring/linux_hammer2
https://github.com/realchonk/fuse-ufs

The initial container batch retained **58 rows, 47 raw passes and 11 failures**.
Four raw passes are excluded: two requested-ext4 cases mounted actual ext2,
and two UFS1 cases inherited an old custom write module. With zero UFS references
and no mounts, the owner restored the stock module: loaded/installed source
version `201E180B8E6BE4EDF81A513`, empty per-module taint. All three stock UFS1
disk retests and the native UFS1 control now demonstrate the ordinary-route
limitation. APFS, ZFS and HAMMER2 loaded source versions match installed modules.

Two manual staging mistakes were corrected and retained as distinct failures:
a nested VeraCrypt creator copy and root-owned Java libraries retaining private
0600 modes. All four affected Cryptomator backings passed independent followups.
The same original Cryptomator engine also passed UFS2 with the corrected host
driver and NTFS with kernel ntfs3. The old NTFS-3G EIO and unpatched-UFS2 EIO
remain in original receipts. The fresh Debian-built CryFS donor from source `195b56b`, including all three
current Linux patches, passed independent exFAT and corrected-UFS2 payload
retests. The UFS2 rerun used the common harness's explicit `fuse.ufs` alias
acceptance and independent driver/superblock proof.

Native feature controls passed nine filesystems, including real ext4 journal
and extent flags, Btrfs CoW identification and XFS reflink/internal-log flags.
ACL, chmod/chown, hardlink and atomic no-replace observations are retained per
control; unsupported or unavailable feature queries are reported explicitly.
Stock UFS1 mounts read-only. The real desktop-helper JSON lifecycle passed all
six implemented Linux backends with payload append/reopens. Three initial folder
helper requests had the harness's wrong source kind, were correctly rejected,
and were replaced by successful directory-kind requests.

Dedicated persistent fixture storage is a new 16 GiB qcow2 at
`/srv/build/lurker-native-fresh-20261127/ubuntu-fixture-storage-20261127.qcow2`,
domain `ubuntu-core-01`, serial `lrkf-fixture-ubuntu`, guest by-id
`/dev/disk/by-id/virtio-lrkf-fixture-ubuntu`. Domain, source, serial, size,
empty guest inventory and holders were checked before formatting. Its ext4 UUID
is `0656b0c1-ede9-4986-98f4-cbe2c3f0d0c9`, mounted under the host owner's
persistent home cache. Eight identified failed images moved there with source
and destination SHA-256 checks before original removal; aliases retain receipt
paths. Unrelated retained fixtures are untouched. A separate small destructive
scratch disk is still required.

The persistent reboot harness passed a six-backend Btrfs pilot with private
0600 test credentials, correct backing mounts, all payload hashes and normal
closure. **OS reboot is still unexercised.** The exact final Linux candidate
`ca7d2137fca741008648360e98acc06ce559de3d` passed fresh workspace check/test/build:
156 tests passed, none failed, 15 ignored; the explicitly enabled native kmod
applet regression separately passed. Its CLI SHA-256 is
`17e6a478bef121eddc581f58cbfd0bf6514c166000ceb509f55a5677204ebbbe`,
helper `57ad52b994cfa6c3e77b3b2208175d6264c6204e05b22574b804e8843f575fb4`,
and desktop `802fd54f71e6e946b1644b8e83cabaec2e5574210f975488c3ecac0c24337df3`.
The formal native-selection package includes verified private engines and Rust
notices; its separately installed private prefix preserves system Lurker. A real
X11/winit window and the real gocryptfs GUI/helper callback lifecycle passed as
UID 1300. The frozen stager emitted group-writable generated metadata under the
login umask; VeraCrypt correctly rejected it. Tightening only those modes to
0644 is recorded, and the already committed stager correction `7c48a25` will
produce the replacement archive. This is packaging evidence, not a crypto failure.

The host UFS2 driver now also includes the reviewed file/directory barrier patch
SHA-256 `292718aec4842afc10668c293dc5d3137abaad77bae49fed8e539be5325932a7`.
Upstream `make prepare`, 28 library tests and native payload reopens passed.
Native file/directory fsync forwarding, injected EIO propagation and retry all
passed. Installed SHA-256 is
`4a30f1c6f3d874d67fe06e8a9341f0996faf711fc69284dfdc1ec15d53975f71`.

The final complete container matrix is running as PID 147126; sequential
variation/conformance/helper followups wait as PID 162747. Next: finish those
container gates, prepare the 54 persistent fixtures, perform an ordinary reboot
and normal hash-checked reopen/close, then attach and test the separately
identified destructive scratch disk and inspect final cleanup. Full private evidence and resumable state remain under
`target/goal-validation/native-fresh-20261127/hosts/ubuntu/` and its checkpoint.

The coordinator subsequently froze follow-up source
`576505d554d798422840a06322dfa93b899365bc`, including the Linux mount
read-only parser correction and desktop form integration. The already live ca7
sweep continues under its own identity. Fresh follow-up workspace job
`20261126T223704Z-d691528f` is running; the idle ca7 variation queue was cancelled
before any case. Remaining runtime gates will use the follow-up package.
