# Ubuntu fresh native qualification — in progress

The frozen follow-up candidate now qualifies **54 of 60 applicable native
container/backing-filesystem cells** for downloaded PDF/text write, normal
close, reopen, append/create and final-hash verification. All six UFS1 cells
are unavailable with Ubuntu's stock read-only driver; 70 other combinations
are explicitly not applicable to Linux. Cipher/keyfile/conformance, helper/GUI,
OS reboot and destructive-device gates are still in progress. The private
130-cell ledger is
`target/goal-validation/native-fresh-20261127/hosts/ubuntu/cases/applicability.json`.

| Filesystem | LUKS1 | LUKS2 | VeraCrypt | gocryptfs backing | CryFS backing | Cryptomator backing |
| --- | --- | --- | --- | --- | --- | --- |
| ext4 | Pass | Pass | Pass | Pass | Pass | Pass |
| Btrfs | Pass | Pass | Pass | Pass | Pass | Pass |
| XFS | Pass | Pass | Pass | Pass | Pass | Pass |
| APFS | Pass | Pass | Pass | Pass | Pass | Pass |
| UFS1 | Stock read-only | Stock read-only | Stock read-only | Stock read-only | Stock read-only | Stock read-only |
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
`20261126T223704Z-d691528f` passed workspace check/test/build: **159 tests passed,
0 failed, 15 ignored**, plus the separately enabled native kmod regression
passed. The idle ca7 variation queue was cancelled before any case. The ca7
sweep finished with 53 passes and five failures: stock UFS1's three native
cases and backing setup, plus the original metadata-mode rejection.

The formal follow-up native-selection package SHA-256 is
`1e1a151753102e6baf514245ef2e8a545007591ccf5e2301df2a326a96ee23e6`.
Its CLI is `f0075e015f0387315aeb232d1bbbd7be2a1ca7e23c09bb978843b57ebadde617`,
helper `daa70a13c28230ab21db85f9f718bc1489fcf5bae10943fddd18501e850b99b5`,
and desktop `9b2ee9e2c39e8f485774df6e9f52fbc9522c55d27eedf3352962610810ea15c5`.
All 1,101 package files match the separately installed prefix's bytes and modes.
The native matrix is running as PID 200458; all three requested-ext4 native
cases have passed. Remaining container, GUI, reboot and device gates use this
package. Completed build artifacts and logs were retained with full hash
verification on persistent storage. Only duplicate expanded staging payloads
and terminal compiler incremental caches were removed, retaining archives and
manifests. The planned new destructive scratch disk is 1 GiB; the separately
identified 16 GiB disk remains fixture/evidence storage.

The completed follow-up sweep has 54 passing receipt rows and four expected
UFS1 failure rows (three native cases and one backing setup representing three
folder cells). A fresh build of hammer2-utils `359b25bc2bb5d81e0bdbb5d48439314d80d13086`
version 0.5.0 exactly reproduces the installed formatter SHA-256
`c86edb153ec1f20d74c3f7f28e0b06e6d34234cea6d6e20baf22558a7bdb5566`.
The older makefs formatter lacked a source manifest, so its binary was retained
and replaced during an idle formatter interval by unmodified makefs r16 source
`607449619ef5f85b6a2b282e16182e342fdebeaa`, fresh build SHA-256
`1aca69ac4434b9433d9d3a0bc8b180e097fa8ba3957bb579734c067f52b09c37`.
All three affected UFS2 native container cases passed independent retests;
subsequent directory backings use the refreshed formatter. Fresh native controls
independently verify superblock VERSION=1/read-only and VERSION=2/read-write.
The patched FUSE driver also checks the exclusive UFS2 magic `0x19540119` before
mounting; its source/installed-byte proof supplements the generic FUSE labels.
Formatter sources/notices remain host prerequisites, separate from Lurker:
https://github.com/kusumi/makefs
https://github.com/kusumi/hammer2-utils

The idle 54-fixture preparation queue is held for explicit resource sizing.
After the remaining conformance/GUI gates finish, a current-candidate six-backend
mounted pilot will measure daemon RSS, native memory overhead and task/FUSE
limits. Cohort size will retain a 1 GiB modeled RAM reserve and 2 GiB storage
reserve; smaller ordinary-reboot cohorts will be used if needed. Eight closed
owned raw failure/control images were made sparse by removing only allocated
zero ranges; before/after SHA-256 checks preserved every logical byte and
released 3,058,233,344 bytes. Failed encrypted images remain intact.

The final Linux follow-up is now `67894f68525c55d5cbb48c743e1a98a308bd970f`.
Its exact-source workspace build/check/test passed **168 tests, zero failures,
15 ignored**, plus one explicitly enabled native kmod regression. The earlier
576 matrix retains its original identity. That matrix's separate follow-up
queue passed 27 groups and retained one normal-user terminal failure. Scoped
read-only TTY/process evidence isolated competing sudo-rs terminal interaction
from the loop lease. The exact process-group source preview passed unchanged
normal-user password frontends and a binary-keyfile lifecycle, with 168 samples
showing separate background groups, unchanged session identities and no stopped
tasks. Arch committed the narrow correction in the designated 678 source.

The newer packaging audit correctly rejected the older creator closure's private
FUSE library. Producer `7d8f9a4` and consumer `fa063ac` now retain FUSE as an
administrator-controlled native prerequisite, with exact architecture/SONAME,
stable alias, actual dependency resolution and separate historical/current
provider hashes. Ubuntu's native producer tests (17), actual GNU loader control,
full six-backend stage, root creator boundary checks (17), and normal-user strict
helper binary-keyfile lifecycles (21 JSON actions across all three disk methods)
passed. Creator SHA-256 remains
`7975cc4724d558fd10bafcc670627c56959a1c76d721f7f83dce855eb0052769`;
zlib remains a legitimate private dependency with its notices. No FUSE payload
is included. Packaging revisions are separate from the frozen Rust source.
New CLI SHA-256 is
`dca53903837f75aef85a277837dfbc17a49b753825cf88e8721a5f096f7f35e0`,
helper `6b6cb0a14c38973605289563d37ed12b2d57130537ecd1044e3446c4058a5487`,
and desktop `842b395c950fb4952210019d9433c0c05c56cdf84507ecfc5839701750e9af87`.

The host UFS2 prerequisite now also includes the reviewed READDIR continuation
patch, SHA-256
`e5749cfe9b04b036843c43d5a75ccdb03af2f28c46add783dccdcca64ee41134`.
Upstream formatting/lint checks, 28 library tests and a fresh release build
passed. With no old-driver sessions, the corrected root-controlled driver was
installed at SHA-256
`78c7e182b87d1e7ad7d3ba21ad49d79444e55db70c9a740788efff57a936b7cb`
(local vendor commit `51f0f3c`). The old-driver native control listed only 204
of 512 files and omitted a newly created entry. The corrected driver listed all
512, then all 513 including the new file, with stable repeated enumeration and
normal closure. The temporary nightly formatter was removed afterward.
The six affected UFS2 container rows and remaining final-user/desktop gates are
queued sequentially as PID 305007; desktop gate coordinator PID 314208 waits.
Ordinary OS reboot and the new destructive scratch disk remain pending.

The final 678 follow-up user queue completed **nine groups passed, zero failed**:
six normal-user password/keyfile full-payload CLI lifecycles, the unchanged
terminal-survival suite, strict password helper samples, and the corrected-UFS2
six-case container sweep. The latter passed all three disk methods and all three
directory engines with the completed host prerequisite. The six corresponding
applicability cells now point to those exact 678 receipts; the other unchanged
cells retain their 576 identities. A real 678 desktop window and gocryptfs/LUKS
GUI-to-helper callbacks also passed. Native Ubuntu Python tests of the committed
producer, consumer and Debian package builder passed **73/73**.

The committed-input staging archive SHA-256 is
`c431fac8feaa390ac756c43c0e23019881d914f2a58c760a17e219a72ed39830`.
All 1,088 installed private-prefix payload files match that archive's bytes and
modes. The separate preview archive remains identified independently. There is
no pre-existing system Lurker package or `/usr/lib/lurker` payload, so the new
native `.deb` will be installed through dpkg and its installed runtime qualified
before reboot preparation. Native package build PID 322132 and installer/runtime
coordinator PID 326213 are live; resource pilot PID 327361 waits for their gates.
The complete final build evidence has moved from tmpfs to persistent storage,
with 497 entries checked before removal of the duplicate and a compatibility
symlink retained. Only obsolete ca7/576 product executable copies were reclaimed:
all six matched preserved complete archives and had no running users, releasing
1,055,459,976 bytes. Their historical package archives and metadata remain intact.

The native Debian package SHA-256 is
`21dc7691ab777ce062ab104ef3ce47a97bed0337572f51947c14733e175803b7`.
Dpkg installed version `0.1.0+native20261127.67894f6` successfully, with all
1,105 installed manifest entries verified and both public launchers resolving
to the correct administrator-controlled products. Actual installed-package
checks passed strict helper binary-keyfile lifecycles for all three disk
methods, the public CLI VeraCrypt password/full-payload lifecycle, a real public
desktop window, and gocryptfs/LUKS default GUI-to-helper callbacks. These are
separate from the earlier private-prefix receipts. The resource pilot now uses
`/usr/lib/lurker`; all six Btrfs fixtures have completed their payload cycles
and are mounted while daemon/kernel memory is sampled. After normal pilot
closure, LUKS1/VeraCrypt/CryFS/Cryptomator installed default callbacks will
complete the six-backend callback gate before any reboot cohort is armed.
