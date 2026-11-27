# Fedora fresh qualification — completed with recorded host prerequisites

Qualified follow-up source:
`67894f68525c55d5cbb48c743e1a98a308bd970f` passed workspace check, tests and build;
all 408 archived source files retain their hashes. The corrected native RPM is
verified in its administrator-controlled test prefix, and its changed-runtime
and prerequisite gates pass. Ordinary reboot persistence is reconciled for 66
file/directory cases plus the physical helper sample; final package/resource
audit passes. The earlier
`576505d` package completed 60 container cells, 75 physical cases, six direct
helper samples and 204 conformance subcases. Those results retain their original
artifact identity; the older private-FUSE closure does not satisfy the final
packaging gate. The corrected package has no private FUSE runtime or filesystem
driver. Historical sections below describe observations at their named stage.

The applicability ledger reconciles 60 qualified Linux combinations and 70
explicitly not-applicable combinations. Qualification uses the recorded native
host prerequisites: stock Fedora does not enable UFS writes, while the existing
configured matching module passes; APFS and FUSE-UFS upstream writing remains
experimental. No new customized kernel build was started. Quick filesystem
properties remain unknown where untested; this is not a full filesystem audit.

Host owner: `fedora`, exclusive `adjutant@fedora-core-01`. This report concerns
fresh execution only. Previous installed artifacts supply inspected backend and
host-tool inputs, never pass receipts. Windows and nested compatibility systems
are outside the round.

## Initial frozen inputs and host

- Product source: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
- Exact Git archive SHA256: `9dcfa043e7ebc527abfb31c211b2bc49a24247243cae02fd20b8ff090de4709d`.
- Filesystem payload harness: `c5feaec`; Linux coordinator forwarding: `88e279c`.
- Fedora Linux 45 Cloud, running kernel `7.3.4-300.fc45.x86_64`.
- Initial boot ID: `22554395-9739-4c4c-b0ab-a3e43be6180c`.
- `dnf upgrade --refresh` and prerequisite installation completed successfully.
  Cryptsetup upgraded to 2.8.9 and SELinux policy to 45.25. Running kernel is the
  newest installed kernel. `dnf needs-restarting -r` reported no reboot needed.
  SELinux remains Enforcing. No broad security-policy change was made.
- Fresh `cargo check --workspace`, `cargo test --workspace`, and workspace build
  passed with one Cargo worker. CLI/helper were also freshly built using an
  inspected warm Cargo cache to begin runtime work during the desktop test build.
- Staged CLI SHA256:
  `fc629e7cc01d1174b07f2b242f276de58a834bba5d942f2f6195b15b6472043e`.
- Staged helper SHA256:
  `4fafc77a4c057e791ad8db8c7663bb97da3bf37a5636e038a0bc47774431f8c0`.
- Reviewed binaries and harnesses are installed in the administrator-controlled
  `/usr/local/lib/lurker-native-fresh-20261127-fedora`. Backend inputs resolve to
  the existing administrator-controlled private backend tree; their hashes are
  retained separately. This is a staged-artifact run, not an installed-package
  qualification.

## Filesystem route assessment and native controls

Fresh native-tool container controls passed Create + Read/Write for all ten
requested filesystems: ext4, Btrfs, XFS, APFS, UFS1, UFS2, HAMMER2, NTFS, exFAT
and ZFS. Each copied the downloaded Bitcoin, Ethereum and Transformer PDFs plus
text, closed normally, reopened and checked hashes, appended text and created a
second file, then closed/reopened and checked again. Every write was gated by a
real OS mount. These observations are native controls, not Lurker passes.

Ordinary distribution facilities provide ext4, Btrfs, XFS, NTFS and exFAT.
Additional native host-local candidates are:

| Filesystem | Actual route and current eligibility evidence |
| --- | --- |
| APFS | Fresh external `linux-apfs-rw` module built/loaded at commit `6cf6de8ad6fa4cb634a700b5d0436d73ee01942c`, upstream commit date 2026-11-19; GPL. Existing `mkapfs` supplies formatting. Write support is upstream experimental and remains a limitation despite this representative pass. |
| UFS1 | Existing loaded external matching `ufs.ko` with write support, kernel 7.3.4, taint `OE`; stock Fedora kernel configuration has `CONFIG_UFS_FS_WRITE` unset. No custom kernel/module build was started this round for UFS. |
| UFS2 | Existing `fuse-ufs` 0.6.0 host-local FUSE driver and `makefs-freebsd` formatter. This is an independent filesystem driver, not a bundled Lurker filesystem stack. |
| HAMMER2 | Existing kernel-matched external Linux HAMMER2 module and `newfs_hammer2`; BSD/GPL module. The inspected source cache identifies 1.0.110. Native mount and payload lifecycle succeeded. |
| ZFS | Existing kernel-matched OpenZFS 2.4.99-1 source-built module and host utilities. Stable 2.4.4 is also present; the matching running module is the newer candidate. |

Upstream source/maintenance references (fresh API commit observations and actual
installed input hashes are retained in private evidence):

- https://github.com/linux-apfs/linux-apfs-rw
- https://github.com/linux-apfs/apfsprogs
- https://github.com/realchonk/fuse-ufs
- https://github.com/jdmanring/linux_hammer2
- https://github.com/kusumi/hammer2-utils
- https://github.com/openzfs/zfs

Quick native feature observations used `setfacl`, `chmod`, `chown`, `stat`,
`cp --reflink=always`, and applicable native property commands. Ownership/mode
changes persisted as `1300:1300 640` on every filesystem except exFAT, whose
native root mount retains synthetic `0:0 755`. ACL succeeded on ext4, Btrfs, XFS
and NTFS. ZFS initially had `acltype=off`; enabling `posixacl` on the disposable
pool made the ACL probe succeed. APFS, UFS1, UFS2, HAMMER2 and exFAT rejected the
ACL operation. Reflink succeeded on Btrfs, XFS, APFS and ZFS. A failed reflink
probe does not establish absence of underlying CoW; unprobed CoW properties
remain unknown. ext4 journal features and XFS metadata were captured; other
journal/CoW details remain explicitly unknown or not applicable in receipts.

## Applicability ledger

Private `applicability.json` has all 130 filesystem × requested-backend cells.
For each filesystem the applicable Linux candidates are LUKS1, LUKS2, VeraCrypt,
gocryptfs, stable CryFS and Cryptomator. Directory rows name their native backing
filesystem and have no selectable inner filesystem.

The other 70 cells are explicitly not applicable: Apple encrypted disk images
require macOS `hdiutil`; APFS encryption requires macOS `diskutil` and is APFS
only; GELI and PEFS require FreeBSD kernel facilities; softraid CRYPTO/RAID1C
require OpenBSD; CGD requires NetBSD. No compatible native Linux provider for
those system facilities has been established. Filesystem creation support alone
does not provide those encryption backends.

## Live work and evidence

Remote persistent root: `/home/adjutant/.cache/lurker/native-fresh-20261127`.
Private local evidence:
`target/goal-validation/native-fresh-20261127/hosts/fedora/`.

- `native-container/native.json`: first 10/10 native controls.
- `native-container-features/native.json`: fresh 10/10 with explicit ownership
  changes and scoped ZFS ACL retry.
- `product-container/matrix.json`: initial required matrix finished **57/60**.
  All 30 combinations were attempted for privileged inner filesystems and all 30
  directory/backing combinations. The three failures are LUKS2/UFS1, CryFS/UFS2,
  and CryFS/exFAT. Every other required cell passed the downloaded-payload cycle.
- Incidental LKL comparison attempts reject the old userspace helper. These
  are excluded comparison evidence and are outside native acceptance.
- `logs/backend-cache-validation.json`: newer VC creator/LKL cache verified
  against its original exact `16aa1f4` source; relevant native/build input diff
  from that source to `8adcdcf` is empty. gocryptfs, stable CryFS and Cryptomator
  cached artifacts validate directly against `8adcdcf`. Original envelopes stay
  unchanged. New backend input copy is administrator controlled. LKL will not
  receive additional comparison work this round.
- `logs/build.log`: workspace check, tests and build passed.
- `logs/credentials.log`: CLI/helper prompt and password/keyfile slot suite passed.
- `logs/desktop-window.log`: fresh real X11/winit software-rendered window passed,
  with frame evidence `/tmp/lurker-desktop-window-bs4lma9d/result.json`.
- `logs/updates.log`, `logs/inventory.log`, `logs/native-provenance.log`,
  `logs/backend-input-hashes.log`, `logs/candidate-fresh-upstreams.log` preserve
  host/update/input details.

At the initial container checkpoint no qcow2 had been attached or overwritten.
All applicable container cases preceded scratch-device work. The device runner verifies the unique scratch
serial and active children immediately before destructive product creation.
Native LUKS2/UFS1 research after the initial batch **passed** with explicit
512-byte sectors and unchanged approved AES-XTS/plain64, 512-bit key and Argon2id
cost requests (1048576 KiB, 5000 ms). Metadata confirms 512-byte sectors and
1048576 KiB Argon memory; all downloaded/append hashes matched after normal
reopens. This is geometry research, not an additional product pass.

Both CryFS failures report `publish new encrypted block: Operation not permitted`.
The bundled durability patch currently publishes new blocks through hard links.
A fresh native no-clobber rename control succeeds on exFAT, including an EEXIST
collision preserving both original files. UFS2/fuse-ufs refuses absent-target
`renameat2(RENAME_NOREPLACE)` with EINVAL; its collision probe preserves both
files. These findings have been sent to the shared Linux/core owners. No
unsupported publication/durability fallback has been accepted.

VeraCrypt AES, Serpent and Twofish password suites and binary-keyfile/Auto suites
all passed: six suites, 72 subcases, including wrong credentials, duplicate and
aliased sources, destination protection and busy close.

Additional fresh research assessed MIT-licensed fstool: encrypted DMG creation
and rekeying exist as library APIs, but its encrypted payload backend is read-only.
This does not establish Create + Read/Write. Historical `cgd-linux` is marked
Pre-Alpha with its last published files dated 2004-02-23. References:

- https://github.com/KarpelesLab/fstool#encryption
- https://sourceforge.net/projects/cgd-linux/files/
 A retained final-sweep wrapper and post-reboot verifier are
prepared; generated test-only credentials remain private mode0600 outside logs.
Device, reboot persistence, broader runtime conformance and final frozen-candidate
validation are pending; this report does not claim completion.

## UFS2 prerequisite follow-up and intermediate product evidence

The fresh raw kernel-UFS2 control also passed using the existing configured
write-enabled module. That observation is separate from stock Fedora and from
the FUSE route used over 4096-byte LUKS mappings. A scoped exact-kernel depmod
override now selects the inspected existing write-enabled module after reboot;
its contents are tracked in `FEDORA_UFS_PREREQUISITE.conf`.

The fresh upstream FUSE driver source is pinned to
`ee5c4a447da98220a11edb69a225d51379afc4fb`. Native tests exposed an incorrect
rename destination lookup and an omitted FUSE ABI feature. The host-only
`FEDORA_FUSE_UFS_NOREPLACE.patch` (SHA256
`c9d71d6a6c24f69b0e1d64653983ad9995e9fd8e7b0b3409dc8deabdc352dc13`)
fixes those two issues. Seven native rename/collision checks and the full raw
payload cycle pass. Cryptomator/UFS2 now completes its ordinary payload cycle.

The additional host-only `FEDORA_FUSE_UFS_BARRIER.patch` (SHA256
`292718aec4842afc10668c293dc5d3137abaad77bae49fed8e539be5325932a7`)
forwards file and directory fsync through the driver cache to the scoped
backing-file `File::sync_all`. Errors propagate to callers. The two-patch driver
SHA256 is `e7c2e1686387c66c3b13310486cbd8120972d9be0d8a64f37d48e5a1dc45dd27`;
28 rufs unit tests pass. Native strace observations confirm a new backing-file
fsync for both client operations; injected EIO reaches the client and a retry
succeeds. Fresh full raw payload write/append/verify and normal reopens pass,
with no remaining mount. This proves the observed barrier forwarding, not an
exhaustive filesystem durability audit. An exploratory destroy-time sync caused
a daemon teardown/reopen race and was removed before this qualified patch.
The installed source commits are `1dd06ad` (rename) and `b9779da` (barriers).
These remain explicitly configured host prerequisites; upstream labels writing
experimental. No filesystem implementation is bundled into Lurker.

The intermediate exact `8714ef5` product physical sweep completes **29/30**;
only LUKS2/UFS1 retains the pre-change geometry rejection. All other cells pass
full real-payload normal close/reopen cycles. A narrowly scoped native VeraCrypt
Quick Format confirmation was added to the device harness; it responds only
for the verified scratch device, create, VeraCrypt and explicit force. Earlier
interrupted device attempts remain forensic evidence, not completed sweeps.
The final source qualification and reboot evidence remain pending.

## Product UFS1 geometry verification

Exact committed source `93e0630` was freshly archived, built on Fedora, and all
373 archive files rehashed unchanged after the build. Four new product cases
pass: AES and Twofish LUKS2/UFS1 containers, and AES and Twofish LUKS2/UFS1 on
the verified `lurker-fresh-fedora` scratch disk. Each executes create plus three
mount/payload/normal-close cycles, including append and a second new file.
All headers identify 512-byte sectors, AES or Twofish XTS plain64 and 64-byte
keys. Argon2id retains 1048576 KiB memory; actual calibrated time costs and CPU
counts are recorded in the dumped JSON. No KDF/cipher reduction was needed.
The existing configured write-enabled native UFS prerequisite remains required.

Private complete evidence is `device/ufs1-geometry-93e0630.tar.gz` and
`validation/ufs1-geometry-93e0630-summary.json`. The latter four-case proof is
separate from the earlier raw native-tool geometry experiment. The physical
controller is terminal with both cases passed and the scratch device closed.

## Final Linux sweep started

Final Linux core/CLI/helper source is frozen at
`ca7d2137fca741008648360e98acc06ce559de3d`. A fresh exact archive is built on
Fedora and staged separately at
`/usr/local/lib/lurker-native-fresh-20261127-fedora-ca7d213`. The final retained
60-cell container sweep uses the complete UFS2 driver prerequisite, the fresh
validated CryFS donor, bounded common payload workers and actual native
filesystem identity checks. The initial 57/60 count records operational payload
outcomes; its older harness did not prove every selected filesystem identity.
The final guarded matrix is the acceptance evidence and remains in progress.

The repaired CryFS engine independently passed UFS2 and exFAT downloaded payload
cycles on Fedora. A fresh repeat of all three directory engines on these two
backings passes 6/6, with no remaining mounts or cleanup errors. The first repeat
had two Cryptomator launch failures because private cached runtime files retained
0600 permissions when copied into a root-owned staging tree. Only that new
staged tree was made readable/executable as appropriate, with root ownership and
write restrictions preserved; the fresh six-case repeat then passed. Runtime
bytes and source identity were unchanged.

## Combined follow-up candidate

The `ca7d213` retained sweep completed 59/60 with no cleanup errors. Its sole
failure was Cryptomator's first payload write on a native NTFS-3g backing
(`EIO`). The failed backing image and copied logs remain retained. The Fedora
retention recipe still selected NTFS-3g; the shared Linux recipe already chooses
the available native kernel writer. The owned retention/reboot recipes now use
that same selector and record the selected driver. Fedora's stock `ntfs3`
module is present and loaded, and its independent writable controls pass.

A shared mount-admission follow-up checks both VFS and superblock read-only
flags. Final combined source is now
`576505d554d798422840a06322dfa93b899365bc`, including the updated desktop.
The completed `ca7d213` receipts remain separate historical batch evidence.
A fresh combined workspace build/package and a new guarded 60-cell sweep will
precede final device/reboot gates. The idle old-source device queue was stopped
before it executed any device step.

Public header metadata for all ten `ca7d213` LUKS2 containers is retained. The
requested profile arguments stayed unchanged. Nine headers calibrated Argon2id
at 1048576 KiB; UFS2 calibrated at 891632 KiB during host load. The native
`--pbkdf-memory` argument is a calibration limit, so the observed value is not
relabeled as 1 GiB. A quiet follow-up will retain a separate comparison; no
product flags are being changed to force a value. Compiler and PBKDF work will
be serialized on this 3.8-GiB VM.

## Verified RPM and capacity follow-up

The combined `576505d` workspace check, test and build pass; all 386 archived
source files rehash unchanged. The native RPM SHA256 is
`0dfc01a676f46fe6c35d1389ce133078c216eaaa9a23d880a5063c8016a22118`;
staging archive SHA256 is
`65576e6cf3af9d4ae2b2d2bfd46e10359e19df20b9b4f3d7794c01b0d942f3d4`.
The actual RPM was extracted, every package-manifest file verified, copied to
an administrator-controlled prefix, and verified again. This is extracted native
package qualification on a prepared host; the system package was not replaced.
The packaged desktop opened a real winit/software X11 window, and the complete
CLI/helper credential suite passes. All three directory engines pass on the
selected native NTFS3 backing. A separate quiet UFS2/LUKS2 downloaded-payload
control passes and records Argon2id memory 1048576 KiB/time 9; the earlier
891632-KiB observation remains separate.

The full 576 sweep reached 52 passes and eight create failures when the guest
root filesystem filled. LUKS logs explicitly report ENOSPC; VeraCrypt returned
an incorrect-size error in the same exhausted environment. Failed fixtures and
all original results are retained. The same package is retesting those eight
cases after capacity repair; current passes will be reconciled with the unchanged
52 successful case receipts, with the initial failures retained as history.

A separate 32-GiB qcow2 now supplies persistent fixture space, independently
identified by serial `lrkf-fedora-store` and ext4 UUID
`52bc440f-db46-4924-b708-1bd20bfc96c6`. The qualification scratch disk remains
serial `lurker-fresh-fedora`. After the sweep became terminal, no round mounts
or loop sources remained. `FEDORA_FIXTURE_STORE.py` verified 9563 entries
(26.7 GB logical, approximately 12 GiB allocated), file hashes, owners and modes
before/after a closed-source copy and private bind at the unchanged round path.
UUID mount and ordered bind configuration persist through reboot. The original
underlay was removed only after verification; all failed files remain in the
verified copy or their original quarantine. Root has 23 GiB free and the fixture
store initially had 20 GiB free, covering approximately 2 GiB of retries plus a
9-GiB device-snapshot reserve and 2-GiB margin. No live mapper/backing source was
moved, and no product or KDF arguments changed for this repair.

## Current final container and device milestone

All eight capacity-affected creates pass fresh retries with the unchanged native
RPM. The canonical `final-container-576505d-reconciled/matrix.json` contains
60 current passing cases: 52 original complete successes plus eight fresh complete
successes. The immutable initial matrix, retry receipts, failed fixture history
and supersession references remain retained. All ten final LUKS2 container
headers are independently dumped: UFS1 uses sector 512, every other filesystem
uses 4096, and this final batch calibrated all ten Argon2id headers at
1048576 KiB. These are observed values, separate from requested profile limits.

The final native inner-filesystem scratch-device sweep passes **30/30** on
`576505d`: LUKS2, LUKS1 and VeraCrypt across all ten filesystems. Each has the
real downloaded-payload write, normal close/reopen, append/new file and final
reopen/compare cycle, bounded I/O, actual filesystem identity and independent
UFS version evidence. All nine binary-keyfile device variations also pass,
covering AES, Serpent and Twofish for all three native methods. Additional
password-cipher variants and physical filesystem backing/directory-engine
cases are still running; reboot and final conformance gates remain pending.

## Complete physical matrix and helper milestone

The final device gate completes **75/75** passes on the actual `576505d` RPM:
30 inner-filesystem cases, nine binary-keyfile variants, six additional
password/cipher variants, and 30 directory-engine cases on native physical
filesystem backings. The latter use the identified scratch disk's single fresh
partition, record native mounts/superblocks/drivers, and test all three directory
engines on each of the ten filesystems. No failed device fixture was hidden or
converted into a pass. Complete compact evidence is retained privately as
`device/final-576505d-75cases-evidence.tar.gz`.

The normal login user directly exercised the packaged internal helper with native
elevation, strict whole-stdout JSON parsing and bounded real payloads. All three
methods pass binary-keyfile/Btrfs lifecycles as file containers and on the fenced
scratch partition: **6/6**. The helper's noninteractive VeraCrypt path needs no
terminal Quick Format confirmation. The interactive CLI/helper password frontend
suite also passes, including negative responses and terminal-session persistence.
The last physical VeraCrypt/keyfile/Btrfs sample remains for ordinary reboot
verification. The committed `FEDORA_HELPER_REBOOT.py` checks its identity and
payload without formatting it. Packaged lifecycle conformance and ordinary
reboot verification were still pending at that helper checkpoint.

## Completed conformance and UFS2 pagination prerequisite

The `576505d` conformance queue subsequently completed 204 passing subcases:
108 native kernel-route cases, 36 directory-route cases, and 60 VeraCrypt
binary-keyfile cases across AES, Serpent and Twofish. There were no failures or
skips. Compact logs and JSON are retained in
`validation/576505d-runtime-conformance-evidence.tar.gz`.

Debian's retained physical UFS2 fixture exposed a separate host-driver defect:
create, fsync and stat succeeded, but subsequent directory enumeration omitted
the new name. The pinned FUSE driver incorrectly returned EOF for every nonzero
directory offset. The minimal host-only `FEDORA_FUSE_UFS_READDIR.patch` corrects
that pagination check (commit `9e27dcc`, SHA256
`e5749cfe9b04b036843c43d5a75ccdb03af2f28c46add783dccdcca64ee41134`).
Apply it after the rename and barrier patches to the same pinned upstream source.
The external source clone records the third change as `6f8dab2`.

All 28 rufs tests and the release build pass. On Fedora the old two-patch driver
enumerated 204 of 512 long names and hid a newly created entry. The corrected
driver enumerates all 512, then all 513 after creation, with equal repeated scans.
Seven rename/collision controls and three normal/injected-EIO fsync controls also
pass. The administrator-controlled installed driver SHA256 is
`237e0467fa4da053c721dd171b71243aee4c7e056a9b66054d6e4ed938c70f30`;
installation occurred after all prior UFS sessions closed. Both prior binaries
and all failure receipts remain retained. This driver is never bundled in Lurker.

The follow-up gate will exercise all twelve UFS2 slots using this driver: three
disk methods and three directory engines for containers and physical storage.
The earlier physical directory backing used the configured kernel UFS2 writer;
that remains separate coverage. The new physical backing explicitly selects the
corrected host FUSE driver. New normal-user password/keyfile fixtures will also
join the ordinary reboot cohort, retaining their creation and reopen identities.

## Corrected host-FUSE packaging

Packaging uses producer `7d8f9a4275f24fc1c303afe52c33d862e190e2db` and
consumer `fa063ac5292a8b47ebfae2dafa083b9700c249ef`, separately from Rust
source `67894f6`. Fedora's native dependency export correctly refused to publish
an incomplete license inventory: installed `zlib-ng-compat` omits notices that
belong to its uninstalled sibling source package `zlib-ng`. That failed fresh
export remains preserved; no notice was omitted or fabricated.

The licensed Debian export archive SHA256 is
`5282c969dffbdfbb3bdefb3ca3ffb1320ba82c58677d7d60da9398dc1accc2e0`.
All eight private-library/license inventory files and the creator SHA256
`7975cc4724d558fd10bafcc670627c56959a1c76d721f7f83dce855eb0052769`
remain unchanged. The qualified producer measured Fedora's actual native FUSE
provider and a new derived envelope records that binding without introducing
Debian path aliases. Native validation confirms `/lib64/libfuse3.so.4` resolves
to administrator-controlled `/usr/lib64/libfuse3.so.3.18.3`, supplied by
`fuse3-libs-3.18.3-1.fc45`, SHA256
`d6e5f6fefbdc478161cf437ccfa3f4519d81c5bc57a0221ebe166327e60e5f2b`.
The corrected manifest SHA256 is
`d64461c6084e0df799ab9c5fa47b149045d43e2a733c647f83212762bc46d38b`.
No FUSE runtime or filesystem driver belongs in the new package. Final package
inventory, actual loader resolution and runtime tests remain separate gates.

The new RPM SHA256 is
`4e9ab64ae6219ad05bb398de5a1e96f333b3f32bb9c67b5b6f595ec29e4e1742`;
its staged archive SHA256 is
`3d31b8b1e04d17587a037181fba6e728ac72984ee2f61d67459c03e093512a58`.
All 1083 manifest entries, including exact declared Java license symlinks and
their containment, pass extraction and administrator-copy verification. The
first verifier stopped on those symlink records before any runtime fixture was
admitted; the corrected verification and original failure are retained.
The extracted native RPM runs from
`/usr/local/lib/lurker-native-fresh-20261127-fedora-package-67894f6` on the prepared
host; the system installation remains untouched. Its inventory contains zero
private FUSE runtime or filesystem-driver files, and the creator's actual loader
resolves the measured host ABI 4 provider.

| Component | SHA256 |
| --- | --- |
| CLI | `4be8699d321f4e4babbe40d290d5395700aaffd19fd09733de8dfa17e16c95a5` |
| Helper | `e96950f0a61f78614618e63f31df89e11f344b1528c4b127597bd094a82d4ff0` |
| Desktop | `5246e5ab9a9fdbaccf71a4726e25bb6f3db5512ff0d5beb9e94edc0f168e6004` |

All six normal-user password/binary-keyfile CLI file lifecycles pass, covering
LUKS2, LUKS1 and VeraCrypt with Auto reopen, full downloaded payload hashes,
normal close and terminal-session survival. Three normal-user direct-helper
binary-keyfile file lifecycles pass strict whole-stdout JSON and the same payload
cycle. CLI/helper password frontend checks and a real native desktop window pass.
Visual inspection of the captured 1280×860 Create view at 100% confirms visible
default controls and the reachable bottom action. Native default-filesystem
desktop storage callbacks, twelve UFS2 reruns, helper device samples and reboot
persistence were still pending at that frontend checkpoint.

All six native desktop storage callback flows subsequently pass, with default
filesystems, the actual packaged helper and no Rust fixture overlay. All three
strict-JSON helper device/keyfile samples pass. The new workspace run has
168 passed tests, zero failures and 15 ignored tests; the ignored native desktop
callback test was then explicitly exercised for all six methods.

The corrected three-patch UFS2 prerequisite passes all twelve affected slots on
the `67894f6` package: three disk methods and three directory engines for each of
containers and physical storage. Every payload cycle has independent UFS2 and
actual FUSE-route evidence; the driver hash is unchanged throughout. Container
phases additionally record the driver and superblock at each phase. An initial
physical-folder harness import error stopped before creating its directory or
partition; only that unadmitted gate was retried using the proper common module.
Both receipts remain retained. `logs/ufs2-all12-678-summary.json` reconciles the
twelve unique role/source-kind/backend combinations and payload hashes.

The scoped loop observer records 2649 process samples and 487 lease/sudo
relationships: every relationship has the caller's session, a separate process
group and a background group; no owned task stopped. A historical stopped sudo
supervisor from the interrupted `8714ef5` harness had only a defunct child and no
device descriptors. After preserving its identity, an exact-pidfd SIGCONT let it
reap that terminal child and exit normally.

## Ordinary reboot admission

The new runtime prepared 66 retained file/directory cases plus the last new
helper/physical-partition sample, with all payload hashes verified before boot.
The cohort preserves 54 `576505d` creation identities and twelve `67894f6`
identities (six corrected-UFS2 and six normal-user credential cases); every reopen
uses the new `67894f6` CLI. Exactly 77 native mounts were observed. No compiler job
or stopped owned task remained, and package/driver hashes matched.

An ordinary `systemctl reboot` changed boot ID
`22554395-9739-4c4c-b0ab-a3e43be6180c` to
`857a23c4-7a9f-4315-bea1-f1ce6424375e`. SSH returned within fifteen seconds.
The persistent fixture UUID mount and ordered private bind restored automatically;
there were no owned vault mounts before reopening, no custom shutdown hook, and
no manual action was needed for that initial mount restoration. Kernel, installed
components, corrected FUSE driver and configured UFS module hashes were unchanged.
The subsequent ZFS close checks exposed the fixture propagation issue below.

## Reconciled reboot result and cleanup

The first boot's 66-case run produced 63 passing payload/lifecycle cases and three
ZFS-inner close refusals. Its ZFS backing pool also reported a native export
cleanup error. Native mountinfo showed the cause: the persistent bind restored
as shared, so nested mounts appeared both at the round path and its backing-store
alias. Lurker correctly refused the additional ZFS root mounts. These failures
remain failed receipts; the guard was never relaxed.

An owned disposable fstab control verifies `bind,rprivate`, private target
propagation and absence of nested source aliases. The persistent recipe fix is
`eb6425f`. Scoped reconciliation recorded the exact aliases, removed only those
aliases normally, and used normal product close/reopen/close. Three closed
failed encrypted images and the closed backing image were copied and hash-checked
before re-baselining. First-attempt generated reboot files remain preserved by
rename, allowing the repeat to create another new file. The 60 unaffected
non-ZFS receipts were retained without repetition.

Six affected ZFS routes and the pending physical helper sample then prepared
eight private mounts. A second ordinary `systemctl reboot` changed boot ID
`857a23c4-7a9f-4315-bea1-f1ce6424375e` to
`cc9d2572-2a9c-424c-bd23-36cd2811e325`. The UUID fixture mount and private bind
restored automatically, with no source aliases, repair or reconciliation on that
boot. All six ZFS routes and the strict-JSON physical helper sample pass complete
verify/append/new-file/close/reopen/verify/normal-close cycles, including normal
backing-pool export. No product or crypto-profile change was needed.

Canonical reboot coverage is **66 unique file/directory cases plus one physical
helper sample**: 60 unaffected first-boot cases and six affected second-boot ZFS
cases. Their creation identities remain 54 `576505d` and twelve `67894f6`; the
runtime is always the qualified `67894f6` package. Private evidence
`logs/final-reboot-678-reconciled.json` records each case's boot and artifact
identities. Original failures and supersession evidence remain intact.

The final audit re-extracts the manifest directly from the SHA-verified actual
RPM and compares it with the installed prefix, then verifies all 1083 file/link
entries and administrator ownership. It confirms zero private FUSE or driver
files, zero owned vault mounts, mappers, loop devices, pools or processes, the
unchanged host-driver/provider/configured-UFS hashes, private fixture propagation
and SELinux Enforcing. The last verified one-GiB scratch disk was detached from
both live and persistent libvirt configuration after exact serial/size/parent,
holder, mount and descriptor checks. Its qcow2 remains preserved at
`/srv/build/lurker-native-fresh-20261127/fedora/scratch.qcow2`, SHA256
`14b0db1442d593aed88380cd9758527d46bdb6b5d08702e792b9fab947b3ee22`.

The separate persistent fixture disk and ordered private bind remain attached
by their recorded UUID to retain evidence and failed storage. Final measured
free space is approximately 22 GiB on root and 9.5 GiB on the fixture filesystem.
No unrelated installation or storage was replaced. Prepared-host qualification
uses the actual RPM's verified payload installed in its private administrator
prefix; it does not claim a replacement of the system RPM installation.

Final private artifacts are `final-summary.json`, the 130-cell
`applicability.json`, `validation/67894f6-installed-gates-evidence.tar.gz`,
`validation/67894f6-final-reboot-and-audit.tar.gz`, and
`build/lurker-0.1.0.native20261127.678.x86_64.rpm` beneath the local Fedora evidence
root. Source, engine, prerequisite and harness identities remain separate.
