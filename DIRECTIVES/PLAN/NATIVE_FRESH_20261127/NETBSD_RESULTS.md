# NetBSD fresh qualification — 2026-11-27

Status: ordinary native qualification is complete on exact installed source
`8e09ac8779bfd022bff05a8753d351db5611a552`. The 130-cell applicability ledger
contains 12 passed, 2 blocked, 56 unavailable and 60 OS-bound not-applicable cells.
The blocked cells are CGD/ZFS mounted-reboot persistence and CryFS on exFAT.
Prior-round receipts are not fresh passes.

## Final installed-artifact result

The actual installed package passed all 14 CGD container/GPT payload lifecycles
and 12 of 14 CryFS backing lifecycles. Every successful final payload writer was
actual UID 1300, with downloaded Bitcoin/Ethereum/Transformer PDFs and text,
normal close/reopen, append/new-file and final digest verification. Actual
UID 32767 was denied plaintext listing, reading, directory creation and opening
an existing payload file for writing through traversable test ancestors. Native
mount, filesystem/driver and GPT identities are retained. Directory rows refer
to the backing filesystem; they do not select an inner CryFS filesystem.

| Filesystem | CGD container + GPT | CryFS container + GPT backing |
| --- | --- | --- |
| FFSv1/UFS1 | Passed both | Passed both |
| FFSv2/UFS2 | Passed both | Passed both |
| ext4 | Passed both | Passed both |
| HAMMER2 | Passed both | Passed both |
| NTFS | Passed both | Passed both |
| exFAT | Passed both | Blocked both: native hardlink ENOSYS |
| ZFS | Payload passed both; mounted reboot blocked | Payload passed both |
| Btrfs | Native Create + Read/Write driver unavailable | Native backing unavailable |
| XFS | Native Create + Read/Write driver unavailable | Native backing unavailable |
| APFS | Native formatter/writable driver unavailable | Native backing unavailable |

The exFAT product engine diagnostic is `publish new encrypted block: Function
not implemented`; the independent native hardlink operation returned ENOSYS
(errno 78). The common durability and exclusive-publication patch was preserved.
The first exFAT attempt also lost a receipt write on the backing filesystem;
its metadata and logs were preserved before a fresh attempt moved only test
artifacts to host FFS and captured the actual product failure. The original
writer's runtime UID was not independently logged; the controlled retry recorded
UID/EUID 1300. No existing payload was chowned to make a test pass. A transient
NTFS outer close returned busy; normal cleanup succeeded, and a separate complete
ordinary-user append/reopen/hash continuation supplied the missing CGD proof.
The failed exFAT GPT partition was streamed off-host, decompressed-length and
SHA-256 verified against its native receipt, and retained with mandatory CGD
parameters and case evidence before scratch reuse.

Native workspace checks passed 131 tests with five ignored. Actual installed
helper JSON passed the physical CGD/NTFS and directory CryFS lifecycles as the
ordinary login user, including wrong password, duplicate and busy-close checks.
The real Slint callback passed 1/1 through that installed helper as UID 1300;
it did not invoke the CLI. The actual installed desktop opened and its collected
window was inspected. Fresh explicit and Auto CGD conformance passed 12/12 each;
CryFS credential/file-persistence subsets passed 12/12 each. The original CryFS
SQLite WAL `SQLITE_IOERR_SHMMAP` failure remains an upstream ReFUSE shared-memory
mapping limitation, not a passed application-workload gate.

Five final installed fixtures passed an ordinary mounted reboot: CGD NTFS,
ext4 and HAMMER2 containers, a fresh-GUID physical UFS2 partition, and product
CryFS on native FFS. All pre/post downloaded-payload hashes matched as UID 1300.
The system wedge changed from dk2 to dk1; GPT identity resolution and mandatory
parameter hashes continued to match. Package/engine and all three corrected
native-provider hashes were unchanged after boot. The stock `kern.sbmax=4194304`
setting persisted. CryFS reopen explicitly reapplied administrator grants:
`vfs.generic.usermount` had reset to zero and was manually enabled; putter group
1300/mode 0660 was also reapplied. Node metadata immediately before that latter
reapplication was not captured, so automatic device-grant availability is unknown
and is not claimed. These results are conditional on manual prerequisites.

The temporary digest-pinned helper sudoers rule was removed and visudo passed;
the base sudoers hash was unchanged. User-mount policy returned to zero and the
native character device to root:wheel 0600 with the same rdev. At completion of
ordinary qualification only base mounts remained, all eight CGD and four vnode
units were unused, and no force import or custom kernel change had been made.
The later, separately classified LKL experiment is recorded below. Owned failed
/tmp GUI/session evidence was copied
and archive-verified before reboot; persistent failed containers, snapshots,
credentials and parameter files remain private. The new round-owned qcow2 remains
attached with closed encrypted test data; the original system disk was preserved.

Quick feature observations apply to the tested native profiles: FFSv1/v2 mounted
with `log`; HAMMER2/ZFS showed their native CoW filesystem types. Other active
journaling behavior remains unknown. Owner selection and private caller admission
passed all seven filesystems. File chmod 0600 was effective on FFSv1/v2, ext4,
HAMMER2 and ZFS; NTFS/exFAT expose fixed caller-owned mode 0700 through their
mount masks rather than arbitrary chmod changes. Native named-user ACL setters
returned unsupported in all seven tested profiles; basic getfacl output alone
was not counted as extended ACL support. Commands and observations are in the
device receipts.

The remaining portable methods have concrete ordinary-source barriers: native
LUKS lacks a dm crypt target/module; a pinned gocryptfs build fails on missing
NetBSD go-fuse transport/layout/syscall definitions; locked jFUSE rejects NetBSD
as UNKNOWN and lacks native ABI bindings; and pinned VeraCrypt fails native
platform selection/build and has no NetBSD device adapter. Administrative
privilege does not supply these missing integrations. These are unqualified
current routes, not claims that future ports are impossible. OS-specific hdiutil,
diskutil, GELI, PEFS and softraid methods remain outside the NetBSD native scope.
ZFS's stock mounted-reboot hostid/import bug is separately retained; ordinary
close/reopen passes, persisted hostid did not solve the module bug, and no force
import was used.

Private final receipts are under `hosts/netbsd/validation/` (matrix, container
continuations, helper/GUI, installed-provider controls, reboot and integrity),
`hosts/netbsd/device/final-installed8e/` (per-device dispatch and retained failed
partition), and `hosts/netbsd/cases/applicability-final.json` (all 130 cells).
The executed harness hashes and exact source/package manifests are under
`hosts/netbsd/build/`. These are relative to the round's private evidence root.

Final post-boot integrity rechecked all 860 manifest entries plus the manifest
itself (861 packaged payload files), matching hashes, root ownership and safe
modes. Three successfully verified closed reboot containers were hashed before
removal; failed fixtures and all parameter/receipt evidence were preserved. The
host has 1.9 GiB free and no owned crypto processes remain.

## Retained ZFS source/configuration assessment

The `ee22369` retained-work authorization started a further source/configuration
assessment, without host, pool, policy or reboot mutation. Current NetBSD 11
and trunk still have the unqualified `zone_get_hostid` macro. The bounded
upstream lookup did not locate an established exact replacement patch. The
already failed persisted-hostid reproduction is retained, not rerun.

A private cache is not a solution: `zpool_find_import_cached` refreshes the pool
configuration before the same `do_import` active-pool/missing-hostid check.
The global cache loads the SPA namespace at module initialization; it does not
itself open pools. No established credential-bound manual-CGD cache/order recipe
was found, so global cache adoption is not a qualified alternative.

An eligible **graceful-shutdown-only configuration candidate** is the documented
NetBSD 11 `/etc/rc.shutdown.final` hook combined with normal `zpool export` of a
specifically authenticated pool. Exported state skips the missing-hostid import
gate. The stock zfs rc.d script has no shutdown keyword, and its stop handler only
attempts module unloading. The final hook runs after ordered shutdown handlers.
Direct `reboot(8)`/`halt(8)` bypass those handlers, and a crash does not run them.
A hook failure is not a shutdown veto: `rc.shutdown` ultimately exits zero.

The first released native batch passed on one fresh 512 MiB regular-file vdev on the
identified work FFS: pool GUID `606192775680998110`, actual UID/EUID 1300 common
payload write/append/reopen/hash checks, normal non-forced export/import, busy
export returning Device busy with the mount unchanged, and wrong pool GUID,
vdev inode and mount FSID rejection before export. The owned busy child was
terminated and reaped; the final pool is closed, all four labels are exported,
and no other pool or encrypted mapping is active. The native fixture vdev is
root-private; its separate caller-owned 0700 plaintext mountpoint is directly
searchable through `/tmp`. No root-only traversal workaround supplied the
ordinary-user proof. A separately reviewed single `shutdown -r now` reboot then
passed using the same fixture and the exact reviewed root-controlled hook.
The actual hook receipt recorded normal export of that pool; before any postboot
import all four labels showed exported state and the same GUID. A normal
non-forced import returned the full preboot PDF/text hashes as actual UID/EUID
1300. A further append and distinct new file survived normal export, non-forced
reimport and final hash verification. The repeated common append helper first
hit its existing-second-file O_EXCL assumption after appending text; that failed
receipt was preserved, and a bounded continuation used a distinct new filename.
No existing payload was removed or overwritten to accommodate that failure.

The temporary hook was removed only after matching its installed inode/hash,
and `/etc` was fsynced to restore prior absence. All pools and encrypted mappings
are closed; the native ZFS module was normally unloaded to its original absent
state. The temporary plaintext underlay was removed; only base mounts and the
previous work FFS remain. The installed CLI/helper/desktop and all three native
FUSE provider hashes remained unchanged. This is a standalone configured
graceful-shutdown proof, not encrypted CGD reboot qualification, crash recovery
or repair of the original failed active fixture. The CGD follow-up remains a
separate reviewed phase. Its initial native ABI/query gate compiled the exact
reviewed read-only probe with base GCC 12.5.0 in a protected root-owned work
directory. The ELF hash is
`f59ce9ad3834640579905d9a0292a1abda887a11904a8e54570dff610589caf1`.
Queries against unused CGD/vnd units 0/0 and 1/1 rejected with Device not
configured; no mapping, data fixture, hook or reboot was created. Native header
sizes, offsets, ioctl values and DISKUNIT/DISKPART observations are retained.
The root filesystem has 1,965,298 KiB free and the work FFS 3,723,676 KiB after
that gate; its mount flags were unchanged by this ABI gate. The work FFS had
been remounted without `log` after the earlier standalone reboot; that delta is
recorded for the next setup review rather than silently described as restored.
The first ABI compile did not explicitly set TMPDIR; its scratch placement was
not attested. Both unchanged baseline sources were then compiled with protected
work-local TMPDIR, producing byte-identical ELFs; verbose compiler traces retain
the actual work-local assembly/object/linker scratch paths. The first receipts
and that limitation were preserved.

Independent review corrected the proposed device export worker before storage
use: same core registry flock across validation/export, exact single-vdev ONLINE
topology, native unit/partition/profile checks, and an exception-proof guardian.
The revised query probe compiled natively as ELF
`7cb5ce8a88c676756642012cb0586061155a025aa920206869d1d081b5aeccd9`;
both unused-unit pairs rejected. Exact worker `e53fe41b...` passed ten native
no-storage mock cases: positive completion, deadline, guardian SIGTERM, receipt
ENOSPC, rename/fsync/log-write failures, output overflow, inventory timeout and
an exited leader with a remaining output writer. Competing same-inode flock
opens stayed blocked until all retained children and output streams completed,
including recording failures. Overflow was drained/discarded without child
signals; faults returned failure rather than false export success. All case
PIDs, native birth samples, terminal waits, lock samples and failed receipts
are retained under `hosts/netbsd/build/zfs-cgd-shutdown-config-proof/`.
These controls used a work-local mock registry and replaced inventory children;
real core-registry wiring, configured CGD identity and actual device export
remain unqualified. No encrypted fixture, mapping, real export, hook or reboot
was created. The real `/var/run/lurker-bsd` remained absent. Terminal native
inventories match the prior mounts/flags and all unused CGD/vnd units. Root
headroom is separately 1,965,296 KiB, with 3,716,630 KiB free on the work FFS.
The complete fault evidence archive has SHA-256
`96bcc848078e23b16707040fea5a95a97b283a5e82fd2dc529b0f67fc1be895f`.
A fresh encrypted lifecycle still requires separate engineering release.
The initial empty-pool `zpool list -H` uninitialized-return failure, unsupported
`status -P` option and mistaken busy-probe payload path were preserved; only the
harness was corrected, and no second fixture or kernel fix was introduced.
The tested hook targeted only independently authenticated pool GUID/vdev/mount
identities. No broad export, force, boot-kernel change or library change was used.
The earlier local kernel repair remains cut.

The private source hashes, exact findings and original proposed batch are
in `hosts/netbsd/build/zfs-retained-source-assessment/{inputs,assessment}.json`.
The native batch, its frozen plan, all failed attempts, exact worker/hook bytes
and terminal inventories are under `hosts/netbsd/build/zfs-shutdown-config-proof/`;
the final phase-1–4 receipt has SHA-256
`acad8988fc8abb399a62351b64f390453bb90f5d209fc6dbf1ec108842a219eb`.
The final standalone reboot receipt has SHA-256
`95462efb23af67079e02de7ed990301dddb290b049dc7e0ff5cdb495bca4ff1a`,
with the complete control/evidence archive SHA-256
`b47e82cc2e21e77550404856b3b076a7fa9eaf78c3e6debb5fe8b7c08d30e373`.
Primary references:

- https://man.netbsd.org/NetBSD-11.0/rc.8
- https://man.netbsd.org/NetBSD-11.0/zpool.8
- https://raw.githubusercontent.com/NetBSD/src/netbsd-11/etc/rc.shutdown
- https://raw.githubusercontent.com/NetBSD/src/netbsd-11/etc/rc.d/zfs
- https://raw.githubusercontent.com/NetBSD/src/netbsd-11/external/cddl/osnet/dist/cmd/zpool/zpool_main.c
- https://raw.githubusercontent.com/NetBSD/src/netbsd-11/external/cddl/osnet/dist/lib/libzfs/common/libzfs_import.c

## Subsequent LKL investigation — terminal and cut

A separate native LKL investigation followed ordinary qualification. It did not
change the installed `8e09ac8` product or add a Btrfs/XFS support claim. Novel
means kernel code or behavior invented by Lurker. Established pre-made patches,
including kernel-source patches applied for Lurker, and existing supported
parameters/configuration before a build remain allowed. Reproducible customized
kernel builds using established inputs are allowed with recorded configuration
differences, patch provenance and build steps; neither customized builds nor
ordinary userspace work are blanket-excluded.

The experiment used upstream LKL `d0f76a77...` with local native host adapters.
After stock `SIGEV_THREAD` delivery blocked startup, local work added a
`timer_quiesce` kernel ABI slot and shutdown call. The modified embedded kernel
passed 35 RAM-only suite cases and dependent credential/transport controls.
That ABI/shutdown behavior was invented locally and is not an eligible route
under the user's non-invasive mainstream-repository requirement. Independent
userspace controls and their exact identities remain separately recorded.
No native LKL FUSE mount, driver installation or product enablement occurred.

A later invalid-syscall control exposed a real out-of-bounds handler-table read.
Its locally authored repair reached a kernel rebuild. Verified supervisor
18864 and owned make group 15917 were cancelled with TERM then unconditional
group KILL while ownership was reserved; the supervisor recorded terminal
`-15`. No repaired library or boot executable completed or ran. Latest read-only
checks found zero owned build/runtime jobs, group members or descendants, and
no subagents were spawned.

The custom ABI/shutdown work, syscall repair/build/tests, proposed `page_free`
replacement or lifecycle relocation, dependent mounted-driver experiments and
any project to obtain upstream acceptance are cut from actionable plans. The
`page_free` idea remained source-only and was never implemented or tested.
Experimental WIP, partial objects, crash/core, archives and receipts are
preserved without further execution, deletion or reversion. Configuration/build
edits are judged separately; `MMU=n` is an existing upstream default.

The identified new FFS work/cache volume remains mounted solely for preserved
source and evidence. User-mount policy remains zero and putter root:wheel 0600;
the three installed native provider hashes are unchanged. Original scratch and
failed fixtures remain preserved. This later build mount does not alter the
ordinary qualification cleanup receipt above.

Owned archive disposition is in `native/netbsd/lkl/README.md`. Exact private
receipts are under `hosts/netbsd/build/lkl-native/`, especially
`new-scope-kernel-audit.json`, `bounds-build-cancel.json`,
`final-experimental-cut-verification.json` and `final-experimental-cut-status.json`.

## Earlier fresh investigations and artifact identities


The assigned host is `adjutant@netbsd-core-01`, running NetBSD 11.1 amd64,
GENERIC dated 2026-11-02. The current formal release and release-specific
security page were checked; the latter lists no advisories yet. `pkgin update`
refreshed the configured NetBSD 11.0 binary repository and both full-upgrade
and upgrade reported nothing to do. The initial root filesystem had 14 GiB free.
`adjutant` is not in sudoers; the existing root SSH inventory key supplies scoped
administration without changing authorization configuration.

- Frozen product source: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
- Fresh workspace run: `20261126T203531Z-27ee973f`.
- `cargo check --workspace`: passed.
- `cargo test --workspace`: 105 passed, 0 failed, 5 ignored.
- CLI/helper build: passed; reviewed binaries are installed in the isolated,
  root-owned `/opt/lurker-native-fresh-20261127-netbsd` prefix.
- Physical partition parser `c6fd18e`: native core check and five targeted tests
  passed. Shared product device lifecycle is implemented and natively proven at `a38fc83`;
  the final candidate including the NTFS prerequisite guard remains in progress.

Fresh CGD downloaded-payload results against the frozen baseline:

| Inner filesystem | Container outcome |
| --- | --- |
| FFSv1 | Passed |
| FFSv2 | Passed |
| ext4, restricted fuse-ext2 profile | Passed |
| exFAT | Passed |
| NTFS-3G | Failed: native PUFFS payload I/O stalled |
| HAMMER2, native matching external module | Passed after enabling upstream write option |
| ZFS, base kernel module | Passed |
| Btrfs | Unavailable: no identified native Create + Read/Write driver |
| XFS | Unavailable: formatter package exists, native mount driver not found |
| APFS | Unavailable: no native formatter and writable driver |

Each passing payload receipt covers create, real read/write mount, downloaded
Bitcoin/Ethereum/Transformer PDFs plus text, normal close, reopen and digest
verification, append/new file, another close/reopen and final digest verification.
Explicit and Auto CGD conformance each passed 12/12, including existing-target
protection, wrong password, duplicate/aliased source and busy close. CGD retains
its fixed AES-XTS/Argon2id password profile and mandatory parameter sidecar;
alternate ciphers and keyfiles are outside its implemented profile.

The latest upstream HAMMER2 v1.2.13, commit
`64095c3947f246f3eb6a8d7341ba856e057b6a6c`, was built against the running kernel.
NetBSD 11.1 requires explicit `-lcrypto` for the upstream tools' SHA-256 aliases.
The upstream `HAMMER2_WRITE=1` build option is necessary: the default module
returns EOPNOTSUPP from its write routine. The initial default-build failure is
retained separately; the enabled-write module passed the unchanged synced payload.
This is an ordinary matching external driver build, not a customized kernel.

The NTFS driver stall was recorded with exact owned worker/backend identities.
Scoped SIGTERM of the owned NTFS daemon released the blocked requests, after
which ordinary product cleanup completed. No forced unmount or pass credit was
used. A fresh bounded native control reproduced the stall and retained the encrypted
failed partition. The root cause was the stock 262144-byte `kern.sbmax` limit,
which cannot hold libperfuse's default 2162688-byte socket queues. Raising the
ordinary host limit to 4194304 made the unchanged native NTFS lifecycle pass.
The setting is persisted in `/etc/sysctl.conf`, with its original backup and hashes
retained. A base ReFUSE rebuild was also assessed; it compiled but crashed during
native mount, so the stock driver remains selected. Its crash core is retained.

Lurker now checks the socket prerequisite before NTFS creation and before starting
the mount driver. Native negative tests at the original limit verified no new
file, changed device/image header or changed recovery parameters; vnode and CGD
inventories stayed unchanged. Restoring the qualified limit passed the complete
NTFS container and physical-device payload flows with the new guard. It never
changes kernel configuration or weakens flush behavior automatically.

A new 2 GiB qcow2 was attached to `netbsd-core-01`, target `vdb`, serial
`lurker-fresh-netbsd`, at `/srv/build/lurker-native-fresh-20261127/netbsd/scratch.qcow2`.
The guest's unique 512-byte marker matched the preattachment host marker before
any GPT write. The initial GPT wedge was `/dev/dk2`, parent `ld1`, offset 2048,
4190208 sectors; partition UUID `50da81c6-56b2-4c62-9251-4cc1481e0e46`.
A direct native CGD/FFSv2 control passed the complete downloaded-payload lifecycle
on this partition and cleaned up normally. This is a native control, not a Lurker
physical-device pass.

The first ordinary reboot activated the new disk, but exposed a qualification
setup error: the host's default `clear_tmp=YES` deleted the round's `/tmp` root,
including the armed persistence image and remote failed fixtures. That reboot
persistence attempt is invalid/incomplete. Collected receipts/logs and installed
artifacts survive, but the failed NTFS image was lost. Subsequent fixtures and
reproductions use persistent `/var/tmp/lurker-native-fresh-20261127-netbsd/`.
The replacement persistent UFS2 fixture passed an ordinary mounted reboot and
post-boot payload verification. Wedge numbering changed: scratch became `/dev/dk0`
and the system root became `/dev/dk2`; every later destructive case resolves the
recorded GPT identity again instead of reusing a device basename.

The 130-cell applicability ledger records every requested filesystem/backend pair.
Stable CryFS product integration remains pending; a fresh native source and ReFUSE payload control is now proven separately below. Native driver
limitations and unavailable encryption engines are not counted as support.
Private evidence is under `target/goal-validation/native-fresh-20261127/hosts/netbsd/`;
its checkpoint identifies artifact hashes, receipts, resource identities and next work.
The common payload harness revision is recorded separately from the product source.

At frozen `a38fc83dfb3b430fb90444a624f314f9d1680db0`, the configured host passed
all seven container and all seven independent-GPT-UUID device filesystem cases.
The CLI artifact SHA-256 is
`9946178bad9c115e9ba9e5eb5046c7ed3e9af91b5fcf2cefbd5f4de617bc1f00`;
the helper is
`5e0fbe35fee9f18912f16b9a7869ef25b83e4b90a583bebfcd2ae30eef433f4a`.
Its workspace run `20261126T212311Z-2070af05` passed check and tests: 112 passed,
0 failed, 5 ignored. Root helper JSON physical UFS2 passed the downloaded payload.

Ordinary UID/GID 1300 helper JSON also passed physical creation, downloaded payload,
Auto reopen, wrong credentials, duplicate and busy-close checks, authorized listing,
and filesystem ownership 1300:1300 with mode 0700. Native desktop tests passed
34 product tests plus one test-only software snapshot probe; the ignored real GUI
callback test separately passed as UID 1300 through the real helper. A temporary
sudoers rule allowed only the exact root-owned helper SHA-256/path with `--as-root
run` or `--as-root list-native`. `visudo` validated it, unrelated commands remained
denied, and the rule was removed with policy restoration verified. This test-only
authorization does not establish a default ordinary-user sudo policy.

The frozen candidate `dac50106f0b39b10fe781fe84d2b50e443ed64d6` includes the
physical foundation, desktop GPT selectors and NTFS prerequisite guard. Workspace
run `20261126T215016Z-b440f1b1` passed check and tests: 114 passed, 0 failed,
5 ignored. Its staged CLI `6bfce2292334744a40af79e9368e36c426d79522a94c58e92425071e85eb41b4`
and helper `a8fdd2079a4c1066a1c290b35566c4492b9636ac8c1acddc290a728968923698`
passed all seven container and seven independently identified GPT device payload
lifecycles. These staged artifacts are distinct from the native package below.

A fresh native CLI/desktop development package for the same commit was created
with the ordinary NetBSD package tools: `lurker-0.1.0.tgz`, SHA-256
`7bcafe3ac4905f316b7ff12c5e47dde42f1c07f5642289c07ea4ad10945ae672`.
It contains 845 files and declares `libxkbcommon-1.13.2` and `sudo-1.9.17p1`.
The host's previously unmanaged xkbcommon libraries were replaced by the ordinary
current package. `pkg_add` installed Lurker into the isolated root-owned prefix
`/opt/lurker-native-fresh-20261127-netbsd/installed-dac5010`; this is an actual
native package installation, with default debug builds rather than release builds.
Its CLI, helper and desktop hashes are respectively
`9020aa50e27c0a820312b702cd9729231a074c0ba377e7ae91492167d2792bfc`,
`c29bac0f81f17c2f496689b4d5b310d987cf1bdcb8a235e0bb30955394b13433` and
`fb6068f747ca74becce3749849a8e68abdc267b256caca6f365627a3d7239d58`.
The installed CLI passed a UFS2 payload lifecycle; the installed helper passed the
complete physical UFS2 ordinary-user lifecycle and ownership/privacy checks.
The frozen-candidate real GUI callback test passed 1/1 through that helper, and
the actual installed desktop opened its Slint software window under Xvfb as UID
1300. The screenshot was collected and viewed. The second temporary authorization
rule was removed and both helper and unrelated sudo commands were denied again.
The installed CLI separately passed all fourteen container/device filesystem
lifecycles. Twelve passed initially; two exFAT cases were rerun after replacing
an argv-only driver identification assumption with kernel executable-path and
open-device descriptor evidence. Both fresh exFAT retests passed. The original
harness failures and their fixtures are retained; the staged 14-case result is
kept separate. Each installed case additionally captures native filesystem
identity: FFS version magic from dumpfs, ext4 extent/journal superblock flags,
exFAT/NTFS OEM signatures and driver identity, HAMMER2 magic, or matching ZFS labels.

The frozen staged candidate passed ordinary mounted-reboot persistence for NTFS,
ext4 and HAMMER2 containers and a physical UFS2 partition. The persisted NTFS
socket limit survived the reboot; physical CGD parameters retained the same hash
and UUID while the current wedge was resolved again. ZFS close/reopen succeeds,
but its mounted-reboot test is blocked by the ordinary native module: safe import
refuses the pool as potentially owned by another system. Configuring and persisting
a stable nonzero `kern.hostid` did not fix a fresh reproduction. Read-only `zdb -l`
inspection found no hostid field in any of the four pool labels. In the current
NetBSD 11 source, `zone_get_hostid` expands to `hostid`, while
`spa_config_generate` declares a local `hostid = 0` before invoking that macro;
the macro therefore reads the local zero. The stock module also has no unresolved
hostid reference. The stock rc.d zfs service only attempts module unloading and
has no pool export or shutdown hook. No forced import, custom hook, module source
change or ineffective product prerequisite guard was applied. Failed encrypted
ZFS fixtures remain intact. This cell is blocked for reboot qualification even
though its ordinary file/device lifecycle passes. A locally invented base-module
repair is cut from the plan; no such repair was started.

Stable CryFS 1.0.3, pinned commit
`5d0cc36785d93e12250696a3ffc928042f2e8d61`, was built with ordinary native
CMake/system dependencies after adapting the memory-query and pthread APIs.
The ordinary pkgsrc libperfuse engine built, but its user daemon first failed
on device access and then aborted while creating its mandatory trace under
`/var/run`. A stock system perfused daemon could mount it, but the root-owned
mount denied ordinary-user payload creation. These failures are preserved and
provide no product pass or privilege bridge. An owned worker left by the
failed system-daemon test was found in the full mount inventory and closed
with normal root umount after recording its source, fsid, process identities
and descriptors; no force flag was used.

The separate base ReFUSE API 29 route passed all downloaded-payload phases,
normal user umount, engine exit, reopen and digest verification as UID 1300.
A NetBSD-specific chmod adapter preserves the existing node type when ReFUSE
supplies only permission bits; regular-file 0600 and directory 0750 types,
ownership and modes then survived both reopens without assertion logs.
Its qualified engine is SHA-256
`ebe561341c63098ad8ccdb7fc2974c742309938109b6efb511c22d1b099f5077`.
The common durability patch is unchanged; platform adaptations are committed
in `32a04b4` and `7e8a7eb`. The actual native headers and `__fuse_main` dispatch
were verified, and ldd shows base librefuse rather than pkgsrc libfuse. Native
mounts report `/dev/puffs`, a truncated `puffs|cryfs@...` type, distinct fsids
and owner 1300; their plaintext root was 1300:1300 mode 0700.

This control temporarily enabled `vfs.generic.usermount=1` and gave only group
1300 access to the root-owned `/dev/putter` node with mode 0660. Both kernel
setting and original device owner/group/mode were restored afterward. It does
not establish default ordinary-user host policy. The ReFUSE engine has no
privileged execution route. Core lifecycle, packaging and backing-filesystem
matrix integration remain pending, so these native controls do not become ten
CryFS product passes. The actual installed CGD CLI also passed a separate ordinary mounted reboot
for NTFS, ext4 and HAMMER2 containers and the GUID-identified UFS2 partition.
The qualified native CryFS/ReFUSE fixture passed the same reboot on its FFS
backing store, including the appended text and new file. The boot changed from
22:12:36 to 23:03:13 UTC on 2026-11-26. All five fixtures reopened with identical
payload hashes and closed normally. Persistent CGD parameter bytes were stable;
`kern.sbmax=4194304` survived. Native user-mount policy and device permissions
were restored after the post-boot proof, and the full mount/mapping inventory
returned to baseline.

That checkpoint ledger recorded six CGD payload cells, one ZFS reboot block,
ten pending CryFS cells, 53 unavailable cells and 60 platform-inapplicable cells.
The later ordinary-user permission gate below remains required before final
cell counts or support claims are assigned.

CryFS core integration is now committed in `bd65b08`, its native factory and
stager registration in `d806711`, and desktop directory forms in `cca3c17`.
The native process adapter queries kernel PID, birth seconds/microseconds and
zombie state with `KERN_PROC2`; a bounded capture handles the observed transient
invalid birth fields only for the unreaped owned child. Invalid live queries
remain errors. Native core tests passed 55 with 4 ignored. The first test race
and raw state-transition observations are retained separately.

The official factory engine SHA-256 is
`e1b3f3c9663acdf3c933e0d40dbb352ccd279ac28b7b8cd135ee2413a8b5c7e8`.
It passed fresh ordinary-user product PDF write/append/verify, three native fsid
and owner observations, metadata preservation and normal close. Native builder
and stager Python suites passed 64/64. The manifest records fourteen notice
files, five native dependency identities and the two BSD inline-header notices;
base ReFUSE remains a separately provided host prerequisite.

The unchanged full conformance workload exposed a separate ReFUSE limitation:
SQLite WAL shared-memory mapping fails with `SQLITE_IOERR_SHMMAP`. A direct
`MAP_SHARED` probe returns EOPNOTSUPP; stock ReFUSE unconditionally sets the
PUFFS no-cache flag and the kernel rejects the required mapping. The failed
suite and encrypted fixture remain intact. Separate explicitly labelled
credential/lifecycle subsets passed 12/12 with explicit selection and 12/12
with Auto, covering wrong passwords, duplicate aliases, busy close and existing
target protection. Those subsets do not turn the failed SQLite workload into
a pass. No cache, flush, filesystem or SQLite mode was weakened.

Native desktop tests passed 40 with one ignored callback test. That callback
separately passed as UID 1300 using the real helper and official engine: create,
mount, synced one-MiB write, normal close, Auto reopen, read and close. Its
software-rendered CryFS create forms at 100% and 120% were inspected and fit.
The initial callback created the vault but failed its automatic native-volume
refresh because the default administrator policy denied `list-native`; its
failure and fixture are retained. The successful test temporarily authorized
only the exact root-owned helper digest/path with `--as-root list-native`.
CryFS operations stayed unprivileged. The rule was removed, visudo passed,
user-mount policy returned to zero, and the original root:wheel 0600 device
identity/metadata and full mount baseline were restored. This records a tested
administrator-policy prerequisite, not a default installed user policy.

The frozen `cca3c17` native workspace passed check and 124 tests, with five
ignored. Its actual native development package contains 861 files and selects
CGD and CryFS; package SHA-256
`8cb949f574a4940be52699782b998e596f61469ec92c5d95b785d85e2adb6121`.
It declares Boost, fmt, spdlog, xkbcommon and sudo native dependencies. Normal
package tools replaced only this round's prior registered test package, and
installed file hashes and root ownership match the staging manifest. Installed
CLI/helper/desktop hashes are respectively
`47381a4f60eeb59e1f7aab43a1efc63dbf5537676345de5ac908c7bf30c1d83d`,
`ac8c5ddd1f826468f78955577fa32a89ee4dd50f5adc382631d647189ce493eb`,
and `c288885dadfb03f0cb7d35fe7f9607f6b3891af4c5368a61c8a0edee10f3ee86`.
The actual installed desktop opened normally as UID 1300, and its collected
window was inspected. The frozen-source real GUI callback separately passed
through the installed helper. Installed helper JSON passed the full common PDF
lifecycle, wrong password, duplicate and busy close as actual UID 1300.

Backing-store tests then exposed a distinct native permission problem. Earlier
CGD payload matrix syscalls were UID 0; selecting ownership with `SUDO_UID` did
not make those writes ordinary-user operations. The earlier actual UID 1300
CGD/helper payload proof used UFS2. Fresh NTFS/exFAT backing tests reported
1300:1300 mode 0700 but denied the caller's initial directory creation. A normal
same-mode root update enabled writes without changing metadata or timestamps,
but stronger tests found that real unrelated UID 32767 could read private data.
That draft Lurker workaround was withheld and preserved privately. A native FFS
control correctly denied the unrelated user, confirming a PUFFS library issue.

An isolated build of exact NetBSD `netbsd-11` userland source
`4299530ca04c23eff6e740a3d97d29bca16c22fe` identified the permission-bit ABI
mismatch: kernel read requests use 0400 while libpuffs' public read bit is 04.
It also identified stale libperfuse cached attributes and missing ReFUSE access
and mounting-user admission. The tested userland correction normalizes native
and legacy access bits, keeps cached attributes current, and enforces standard
FUSE admission across credential-bearing operations with access/permission
fallbacks. No kernel, PUFFS cache flags, flush behavior or crypto profile changed.

The isolated library variants passed actual UID/EUID 1300 list, read, create,
synced write, append and read on native NTFS and exFAT. Actual UID/EUID 32767
was denied listing, reading and writing. Ext4 exposed an additional foreign-user
creation gap in the driver's default access callback. The final correction also
checks parent search/write access when standard `default_permissions` is
requested; the Lurker ext4 adapter now explicitly requests that option. Native
ext4 caller access and unrelated-user denial then passed.

The reproducible host-only builder and pinned four-file patch are committed at
`a90fcfd` and `091bd71`. Exact source notices remain in its output. Public builder
output manifest SHA-256 is
`97e17b6ef1332ded64d7b3328f80c7e7d723654803a9f4ce5af8cbc02828f680`.
With all FUSE daemons exited, mounts closed and CGD/vnd inventories empty, the
administrator preserved all three original versioned native libraries and
symlink/metadata/hash receipts in a root-private backup. The reviewed output was
then installed atomically at the existing native versioned targets. No libraries
were included in Lurker's private engine directory or package. Fresh processes
verified puffs/perfuse capability version 1 and ReFUSE policy version 2; native
ELF64/x86-64 headers, exact SONAMEs and driver/engine dependencies were recorded.

Fresh controls using the actual installed providers, without a library-path
override, passed caller access and unrelated-user listing/read/write denial on
NTFS, exFAT and ext4. Process maps confirmed the versioned `/usr/lib` providers.
The unchanged official CryFS engine then passed all common PDF phases and
metadata persistence while the unrelated user was denied all three actions
through deliberately traversable test ancestors. All closes and engine exits
were normal. Native mount owner was 1300; actual ReFUSE/puffs provider paths were
verified. Host user-mount policy and device permissions returned to baseline.

Lurker capability guards are committed at `8e09ac8`. They inspect root-controlled
native ELF paths and ancestry, use the native loader to enforce architecture,
verify the exact versioned capability, and require `dladdr` to identify that
same canonical library as the symbol provider. A dependency symbol cannot
qualify an unrelated library. Native core checks passed 60 tests with four
ignored. Before the host installation, fresh stock-provider negatives passed
three container and three physical creation cases without new files, parameter
sidecars, partition-byte changes or inventory changes. An actual UID 1300 CryFS
helper creation also rejected before vault mutation. The draft chmod refresh
remains removed. The exact frozen `8e09ac8` source passed native workspace check and 131 tests,
with five ignored. Its refreshed native package contains 861 files, selects CGD
and CryFS, and has SHA-256
`b5b886763cdd8edcc44e20db0332963e6c9be3e3568a9899da6f3ef8cb8803c0`.
The reviewed package replaced only the previous registered round-owned package
through ordinary package tools. Installed CLI/helper/desktop hashes are
`29c16368727ff1d776c9d32d133af85142ff8288cb83808ccc86327209ab1ba7`,
`6b95679559a70cc9accab82adaa3db5c98e441a66331ac438e1f6fefd1f3d8f2`,
and `6b9832cd907889d8c51e6c6cf313ed5c7eb6b87d19d3b6d517aa69a6edda12d4`.
The official CryFS engine remains byte-identical `e1b3f3c...`. Installed manifest
hashes and root ownership were verified, including absence of bundled native
FUSE providers. The completed actual-user container/backing matrix is reported
in the final installed-artifact section above; earlier or isolated receipts do
not supply those final artifact passes.

A subsequent shared folder snapshot fix `c3dd43bf` addresses transient extra
links observed during OpenBSD UFS record replacement. The exact concurrent
rename/fstat probe ran on this NetBSD FFS(log) host as UID 1300: 10,000 fsynced
renames, 2,069,369 one-link observations, nine unlinked held-inode observations,
and zero extra-link observations. Writer exit, file type, owner and private-mode
checks passed. The coordinator explicitly approved continuing the installed
`8e09ac8` qualification on this negative native applicability evidence; that
probe does not claim the behavior impossible on every NetBSD configuration.

An accidentally overlapping helper probe caused five fail-closed global-folder
lock rejections in the initial backing dispatch. The original failures are
retained. Fresh serial ext4, HAMMER2 and ZFS container cases and UFS1 device case
passed; exFAT independently reproduced the permission denial. Completed failed
scratch-partition captures were transferred to private coordinator evidence,
SHA-256 verified, and retained with their parameters, credentials, logs and
relocation receipts to preserve guest build space.

Eligibility references:

- https://www.netbsd.org/releases/formal.html
- https://www.netbsd.org/support/security/patches-11.1.html
- https://github.com/kusumi/netbsd_hammer2
- https://wiki.netbsd.org/zfs/
- https://man.netbsd.org/dm.4
- https://veracrypt.io/en/Supported%20Operating%20Systems.html
- https://github.com/rfjakob/gocryptfs
- https://github.com/cryfs/cryfs/tree/1.0.3
- https://github.com/cryptomator/fuse-nio-adapter
- https://github.com/sgan81/apfs-fuse

ZFS reboot diagnosis sources:

- https://mail-index.netbsd.org/source-changes/2021/12/28/msg136186.html
- https://raw.githubusercontent.com/NetBSD/src/netbsd-11/external/cddl/osnet/dist/uts/common/fs/zfs/sys/zfs_context.h
- https://raw.githubusercontent.com/NetBSD/src/netbsd-11/external/cddl/osnet/dist/uts/common/fs/zfs/spa_config.c
