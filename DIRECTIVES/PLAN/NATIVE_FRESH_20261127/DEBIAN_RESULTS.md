# Debian fresh native qualification — complete within the recorded host scope

The new 2026-11-27 round completes **60/60 container cells and 60/60 scratch
cells** across ten filesystems and six applicable Linux encryption methods.
Scratch cells comprise three direct disk-encryption methods and three directory
engines on an actually formatted native block backing. Directory engines do not
format or encrypt a whole block source; their direct block-source cells are not
applicable. Seven OS-specific encryption methods are not applicable on Linux,
with explicit reasons for all 70 method/filesystem rows per phase.

The configured-host result uses two official Debian kernel cohorts. UFS1's
12 runtime cells are supplemental evidence on an existing non-stock write-enabled
module; stock cloud-kernel UFS support is not claimed. No custom kernel or new
UFS write-module experiment was performed.

All execution evidence is fresh in this round. Under the approved follow-up
policy in `LINUX_FOLLOWUP.md`, the matrix retains 108 named `576505d` executions
for unchanged branches and uses 12 new `67894f6` UFS2 executions after correcting
its host driver. These older current-round receipts are not relabeled as runs of
the final package. Additional final-artifact CLI, helper, desktop and ordinary
reboot gates pass independently. No earlier-round runtime pass counts here.

## Final artifact and host identities

Rust CLI/core/helper/desktop source:
`67894f68525c55d5cbb48c743e1a98a308bd970f`.
Remote exact-archive workspace run `20261127T000845Z-7a8d559b` passed
`cargo check --workspace`, `cargo test --workspace --no-fail-fast` and
`cargo build --workspace`: **168 passed, zero failed, 15 ignored**, one worker.
Managed Rust/Cargo 1.100.0 and uv-managed Python 3.15.2 were used.

The independently versioned host-FUSE producer is
`7d8f9a4275f24fc1c303afe52c33d862e190e2db`; the named Linux stager consumer is
`fa063ac5292a8b47ebfae2dafa083b9700c249ef`. Debian passed 17 producer tests and
53 tests of that exact committed consumer, then staged, built and installed
`lurker 0.1.0+fresh20261127.67894f6`. Native-only creator packaging is qualified;
the optional LKL/userspace filesystem route is excluded.

| Artifact | SHA-256 |
| --- | --- |
| CLI | `a7201b54f69af28d8e15d269cd70d75f6a91b197ef91c40fd9e55b59c3427227` |
| Internal helper | `0c6f1f5bfced7c29d7a61a07c27d0a2fdf7c4ada3f296fd7f79a484f6659c6ed` |
| Desktop | `2cc96470a47202fb300508f508a216a6267a2eac998559db50c2082d11e8f1cb` |
| Named-stager archive | `bf7546838002c12ba16022da3ad3e81a7cbf2c851e4b6e098010f0f5addb94e4` |
| Installed Debian package | `14b481c4f4f8cd8ca351027f1f4cd440a27b40380c11597b82e1a23b96a7f145` |

Debian 13.8 uses glibc 2.41. APT refresh and supported full upgrade completed
with zero upgrades and no held packages; required ACL, tracing and filesystem
utilities were installed. Final/default kernel is official
`6.12.118+deb13-cloud-amd64`; current HAMMER2 is qualified on official
`7.3.4+deb13-cloud-amd64`. APFS, UFS1 and ZFS use the 6.12 cohort. GRUB's existing
6.12 default is preserved, with no pending one-shot selection.

## Filesystem coverage and prerequisites

Each cell performs actual native mounting, downloaded Bitcoin/Ethereum/Transformer
PDF and web-text writes, normal close, Auto reopen, byte/hash verification,
append plus a second file, normal close, another reopen and all five final hashes.
Mount identities and writable-superblock checks prevent underlay/wrong-filesystem
passes. Permission, ACL, reflink and journal observations are retained per case;
these describe the tested profiles, not universal filesystem capabilities.

| Filesystem | Container cells | Scratch cells | Scope |
| --- | ---: | ---: | --- |
| ext4 | 6/6 | 6/6 | Native |
| Btrfs | 6/6 | 6/6 | Native |
| XFS | 6/6 | 6/6 | Native |
| APFS | 6/6 | 6/6 | Separately installed native kernel driver |
| UFS1 | 6/6 | 6/6 | Supplemental existing write-enabled module |
| UFS2 | 6/6 | 6/6 | Final artifact and corrected host FUSE driver |
| HAMMER2 | 6/6 | 6/6 | Official 7.3 kernel plus current external module |
| NTFS | 6/6 | 6/6 | Native ntfs3 / positively identified modern writable ntfs |
| exFAT | 6/6 | 6/6 | Native |
| ZFS | 6/6 | 6/6 | Distribution OpenZFS 2.3.9 on 6.12 |

The six methods are LUKS1, LUKS2, native privileged VeraCrypt, gocryptfs, CryFS
and Cryptomator. The seven excluded methods are Apple images, native encrypted
APFS, GELI, PEFS, softraid CRYPTO, RAID1C and CGD. Encryption profiles and KDF
strength were not weakened. Representative AES/Serpent/Twofish, binary-keyfile,
wrong-credential, no-overwrite, alias, duplicate-session, busy-close and fsync
cases are explicit; a full option Cartesian product is not claimed.

Fresh APFS source is v0.3.22 commit
`80f5db04ecd3b240fee4c67829a5b5465f11f63f`; HAMMER2 source is
`09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0`. UFS2 source is
`ee5c4a447da98220a11edb69a225d51379afc4fb` plus the qualified no-replace,
barrier and pagination prerequisite patches. These drivers are separately
administrator-installed host facilities, never bundled in Lurker.

The existing UFS1 module SHA-256 is
`74ded149aabb284025263a784928ef69cb14474fc78a4ca87c12565848792fe6`.
Its recovered original build script, log and compiler commands match the cached
artifact; all 15 material source/build files and original Git blob identities
match Linux stable 6.12.118 commit `b869a5313cbfe7102873d3de318ddb68675146d0`.
It explicitly enables non-stock `CONFIG_UFS_FS_WRITE`; the stock cloud config
has UFS disabled. No new module rebuild or configuration change was made.

## Final-artifact gates

- Normal-user CLI password and binary-keyfile full payload lifecycles:
  **6/6**, all three disk methods, normal close and terminal survival.
- Installed normal-user helper full payloads: three file and three block-source
  methods, **45 strict full-stdout JSON responses** (21 file, 24 device), including
  the physical fixture's ordinary reboot reopen and close.
- Default Slint create/mount/write/reopen/unmount callbacks: **6/6 backend
  families**, exact workspace test artifact and actual installed helper as UID
  1300. The test artifact is identified separately from the installed desktop.
- Actual authenticated X11/winit software-rendered native window and installed
  desktop-launcher window: **2/2**, UID 1300, captured native frames, owned display
  resources closed. The installed CLI launcher passes its supported `--help`.
- Ordinary reboot: six Btrfs file-backed fixtures, including all three folder
  engines on native encrypted backings, plus the physical LUKS2/Btrfs fixture
  preserve all five hashes and close normally. Boot changed from
  `e50f1be3-d2b1-43b3-a3f2-e352e434cc66` to
  `2d04b4f6-9e21-46e2-935d-70fb97b5457b`, retaining official 6.12 and exact binaries.
- Final manifest audit: **1,083 files/symlinks**, exact hashes, administrator
  ownership and expected modes; empty `dpkg -V`. No bundled FUSE runtime or
  filesystem driver, no owned backend processes, mappings, loops or test mounts.

The six-fixture reopener does not record a controller UID or EUID field. Its
identity is supported by the original unprivileged `adjutant` SSH launch chain,
the prepare wrapper's real-UID guard, the physical-helper/final-audit real-UID
guards around the verify shell's ordinary Python child calls, and retained
systemd/sudo user-session records. The actual physical-helper runner also guards
`getuid() == 1300`, not `geteuid()`. Bounded I/O workers use ordinary `fork`
credential inheritance without changing UID; successful worker results have no
UID/EUID fields. Thus effective identity is inferred from that launch chain,
not independently sampled in each controller/worker receipt. The read-only UID
audit addendum preserves this limit; crypto was not rerun for it.

Named current-round `576505d` evidence also retains nine native cipher suites
(**108 checks**), three folder suites (**36 checks**), three binary-keyfile
negative suites (**36 checks**), three cold HAMMER2 Auto reopens after actual
module unloading, and ordinary cross-kernel persistence. These are unchanged
functional coverage, not a compliant final-package installation claim.

## Findings, fixes and retained failures

The new round found and repaired canonical kmod applet-name loss, accidental
ext2 formatting through canonical `mke2fs`, native NTFS selection, cold HAMMER2
Auto loading, UFS1 logical-sector geometry and the writable-VFS/read-only-superblock
classification. Initial wrong-filesystem receipts do not count as ext4 passes.
Invocation, private-prefix mode and concurrent test-lock failures remain retained
and separately classified; later repaired candidates use independent fixtures.

Debian built the qualified portable Linux CryFS 1.0.3 donor from the pinned
source and complete Linux patch set registered by `195b56be87b38361aaa45a085954e794338ba09a`.
Binary SHA-256 is `2ec7b6f5bd7b999b81750fec75ffb3ca13174f4643d2326ba250143e9f444a14`.
Source, patches, compiler/cache, libraries and license provenance were verified;
native exFAT and patched-UFS2 payload controls passed before handoff. Exclusive
publication uses native no-replace rename and barriers. Unchanged gocryptfs and
Linux-patched Cryptomator inputs pass current-source staging checks.

The physical CryFS/UFS2 reopen exposed a separate host-driver bug. The full failed
1 GiB device image remains immutable with SHA-256
`eb8f0d15505ae08adbfe40741cb83dc7c48a32b03c1aa70c5ddd5872cda49195`.
A writable clone reproduced successful create/fsync/stat but omitted the new
name during native enumeration, without CryFS. FUSE-UFS prematurely returned EOF
for every nonzero readdir continuation offset. The minimal `9e27dcc` pagination
patch after the no-replace/barrier prerequisites fixes this. New driver SHA-256:
`afcb489fdd1e441d418883c71d5abfe45d89d1f9548a1b2fe94479375641c30d`.
All 28 rufs tests pass. Old/new native 512-name controls show truncation to 25
versus complete 512 then 513 names; the failure clone now enumerates 186 then
187 including its newly created entry. Repeated scans agree. Seven namespace
controls and three forwarded fsync/EIO/retry controls pass. All 12 affected UFS2
runtime cells were rerun successfully with the final package; original receipts,
image, backend diagnostics and driver bytes remain retained. Upstream's
experimental-write/no-ACL limitation remains explicit.

Final packaging also corrected the older Linux creator's bundled FUSE closure.
That older package was functionally exercised but fails this round's host-only
FUSE policy. The verified creator bytes remain exactly
`7975cc4724d558fd10bafcc670627c56959a1c76d721f7f83dce855eb0052769`;
the new licensed private closure contains zlib only and records the native
administrator-controlled FUSE ABI. Actual GNU loader resolution and creator
execution pass. FUSE SONAME 4 resolves to Debian's `libfuse3-4 3.17.2-3` provider.
No fake provider alias or old-envelope identity was used.

A final auxiliary `--version` launcher probe was unsupported and stopped only
reboot preparation. Its failed receipt is retained; the supported `--help`
probe passed and preparation resumed without a product change. Native windows
were already successful and were not repeated for that invocation correction.

## Shared snapshot regression after qualification

Exact committed source `c3dd43bf19657baecc89c6e69bb5910e0cfd2dcd` subsequently
passed the bounded Debian workspace check/test regression, run
`20261127T012205Z-7bfadbd0`: **171 passed, zero failed, 15 ignored**. The new
held-snapshot replacement/transient-link/persistent-link test passed. Source
came from its exact Git archive, SHA-256
`12124a343d9742e66832d209b88b87aa387024332a019f373905d5a38e87e8d8`, with no
worktree overlays. Existing managed Rust 1.100.0/Python 3.15.2 caches were used
with one Cargo worker after checking for compiler conflicts.

This follow-up runs `cargo check --workspace` and
`cargo test --workspace --no-fail-fast` only. The installed qualified package
remains `67894f6`, with all three component hashes unchanged. Its original GUI
test executable was preserved with its recorded hash before compiler-cache
reuse. Source, check/test logs, counts and retention receipts are under
`validation/session-snapshot-c3dd43b.json` and
`build/session-snapshot-c3dd43b/` in the private evidence root; the complete job
also remains in the owned VM's retained-build directory.

## OpenBSD integration compile regression

The final exact archive of `123d1777505fef2c3aa0356af3db3896aa3a4a00` passed
Linux `cargo check --workspace` and `cargo test --workspace --no-fail-fast`:
**173 passed, zero failed, 15 ignored**, including the platform-neutral OpenBSD
directory-form test. Run `20261127T014912Z-9f46f6a4` used the idle managed cache
and one worker. Archive SHA-256 is
`a8ded2542fa46ad3d27d75c626547a4a3b2b97a8d4a1b9342ffb129eb8ee4931`;
worktree WIP was excluded. The installed qualified `67894f6` package and its
three component hashes remain unchanged. This check/test-only follow-up is
recorded in `validation/integration-123d177.json` and
`build/integration-123d177/`; the full source/job is retained on the VM.

## Evidence and retained storage

Private evidence root:
`target/goal-validation/native-fresh-20261127/hosts/debian/`.
`final-ledger.json` records all 130 method/filesystem rows, separate source kinds,
artifact identities, native receipts and exclusions. `gate-manifest.json` records
the additional final-artifact gates. Sources, build/check/test logs, installed
manifest, qualified prerequisites, feature probes, failed receipts and native
frames are retained with hashes. VM evidence persists under
`/var/tmp/lurker-native-fresh-20261127-debian-storage`; closed fixture paths and
failed snapshots remain available. No unrelated storage or work was removed.

Owned domain: `debian-core-01`, UUID `27ab88f9-d2e2-49fe-b03f-8364a418d891`.
The dedicated 128 GiB evidence disk has observed virtio serial
`lurker-fresh-debian-` and UUID `1f49c989-f748-4493-af93-0f0c58037595`; its online
grows preserved identity and data. The separate new 1 GiB qcow2 scratch has no
backing file and observed serial `lurker-fresh-debian`, reached through its exact
by-id alias. Every destructive create checks serial, size, alias resolution,
children and holders before passing the guarded canonical node. One optional
VeraCrypt Quick Format confirmation is fenced to forced, verified scratch create;
the shared harness default remains unchanged. The evidence disk stays mounted;
the scratch and encrypted fixtures are closed and retained.

Primary upstreams checked during the round:

https://github.com/linux-apfs/linux-apfs-rw

https://github.com/jdmanring/linux_hammer2

https://github.com/realchonk/fuse-ufs

https://github.com/openzfs/zfs/releases

https://github.com/cryfs/cryfs/releases

https://github.com/rfjakob/gocryptfs/releases
