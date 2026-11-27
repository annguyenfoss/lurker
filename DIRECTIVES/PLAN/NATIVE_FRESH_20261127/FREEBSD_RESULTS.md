# FreeBSD fresh qualification checkpoint

This is an ordinary-route checkpoint, not a final complete host pass. Final
installed container/device matrices have finished. An ordinary shutdown
with 33 qualified mounted fixtures stalled. A later explicitly enabled small-cohort
service reboot passes; broader ordinary-route integration and final package
qualification remain open. Product integration, desktop callbacks and
canonical package installation have completed the gates recorded below. Old reports are not counted as fresh passes.
Private receipts are under `target/goal-validation/native-fresh-20261127/hosts/freebsd/`;
remote logs and preserved fixtures are under `/srv/lurker-fresh-20261127/`.

## Host and artifact provenance

FreeBSD's running kernel, installed kernel and userland are 15.1-RELEASE-p7,
`kern.osreldate=1501000`. Fresh `pkg update` and `pkg upgrade -y` found no
pending ports, kernel-module or pkgbase upgrades. PEFS's ABI annotation is
1501000; PEFS, HAMMER2 and FUSE modules loaded. Ordinary CMake, Ninja, Boost,
range-v3 and spdlog build prerequisites were installed for the native CryFS
build. Existing NTFS 2022.10.3, exfat-fuse 1.4.0 and LKL 4.16 packages were
checked against the configured repository; no newer ordinary packages appeared.

The original root pool was nearly full. A new 64 GiB qcow2 was attached to
`freebsd-core-01`, verified as `/dev/vtbd1`, and made into the separate
`lurkerfresh20261127` build/fixture pool. Old Cargo caches were preserved there
with symlinks at their original paths. Old fixtures were not removed. The
build disk remains necessary for those caches; it is not destructive test storage.

The exact baseline `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`, native run
`20261126T204256Z-a32c3772`, passed CLI/helper build, `cargo check --workspace`
and workspace tests: **122 passed, 0 failed, 11 ignored**. One Cargo worker was
used. Earlier failed/blocked attempts and an intentionally cancelled build
order are retained separately. Baseline staged CLI SHA-256:
`bccebf721ad35162c1b89c5e017190a59ea1bb767078f758a4e649da4c4ff28a`;
helper: `b809af5f824ae2eced807e9fd1effe35d1e894967a20153c71086439dae75553`.
Verified cached native private VeraCrypt 1.26.31 and gocryptfs 2.7.0 retain
source, patch, library and license identities. Their fresh runtime receipts
qualify administrator-owned staged binaries, not an installed package.

CryFS was added after a native upstream proof. Exact candidate
`1c62f1feb534cf484eb2343a83300d9100c3a79a`, native run
`20261126T212936Z-6872f321`, passed fresh CLI/helper build, workspace check
and **123 tests, 0 failed, 11 ignored**. CLI SHA-256:
`a47b8bdb3f6171b92918135af0f438de1d3f8966226cc6711040d34add4e90ac`;
helper: `5e65f245b20dbe00a5e0ddd1d2788976104e385d8dc4d82b069cfc2f88bc01e9`.

Private CryFS uses stable 1.0.3, pinned upstream commit
`5d0cc36785d93e12250696a3ffc928042f2e8d61`, with the existing common durability
patch. The native common factory built the administrator-staged engine with
SHA-256 `971e3202738195258f75a44a889036d424056f0a7545a7e0e0d3539dfaeb254d`.
Its version reports `1.0.3+0.g5d0cc36.dirty` because of the reviewed patch; this
is the stable pinned base, not an experimental 2.x build. Manifest retains
compiler/CMake/package inputs, four runtime package identities and 18 license
files. Native builder and stager focused checks passed (12 and 45 tests).

The first seven CryFS backing receipts had a manually supplied source-label
typo. The actual immutable runner/source manifests and binary hashes identify
the correct `1c62f1feb...` revision above. External receipts were corrected
with explicit `source_identity_correction`, preserving original JSON under
`provenance-originals/`; encrypted inner receipts were not rewritten. Future
launchers read the protected artifact identity and verify its CLI hash.

## Fresh container coverage and ordinary-route limitations

All initial container cases preceded device work. Shared payload harness base
is `c5feaec`, with identified private FreeBSD wrappers. Payloads are the actual
downloaded Bitcoin, Ethereum and Transformer PDFs plus text. Each passing case
created/formatted, opened a real native read/write mount, copied bytes, closed
normally, reopened, verified, appended text/created another file, closed,
reopened and checked final hashes before closing again.

There are **41 initial product attempts: 30 passes and 11 non-passes**. The
initial seven direct-format guards include the subsequently corrected GELI/NTFS
gap. Btrfs/XFS have newer positive native controls pending an eligibility
decision; APFS still lacks a qualified write route. Four directory failures are
identified below. This is not 41 eligible passing combinations.

| Filesystem | GELI inner | VeraCrypt inner | PEFS backing | gocryptfs backing | CryFS backing |
| --- | --- | --- | --- | --- | --- |
| ext4 | Passed | Passed | Passed | Passed | Passed |
| UFS1 | Passed | Passed | Passed | Passed | Passed |
| UFS2 | Passed | Passed | Passed | Passed | Passed |
| HAMMER2 | Passed | Passed | Passed | Passed | Passed |
| NTFS | Passed scoped retest | Passed | Native backing failure | Passed | Product publication gap |
| exFAT | Passed | Passed | Native backing failure | Passed | Product publication gap |
| ZFS | Passed | Passed | Passed | Passed | Passed |
| Btrfs | Product guard; native control now passes | Product guard; native control now passes | Policy decision pending | Policy decision pending | Policy decision pending |
| XFS | Product guard; native control now passes | Product guard; native control now passes | Policy decision pending | Policy decision pending | Policy decision pending |
| APFS | Guard/no native write route | Guard/no native write route | Ineligible backing | Ineligible backing | Ineligible backing |

Fresh native package controls created XFS and Btrfs, mounted through packaged
LKL, wrote payload, closed and reopened. Both then failed append with `EBADF`;
these initial controls are not Create + Read/Write passes. Later `direct_io`
controls described below supersede that driver assessment. Initial controls are in
`native-filesystems-1795725868`; preliminary setup failures are retained.
Initial GELI/NTFS formatting timed out with the native port's default cache;
that receipt and the original product guard are retained. A later 512-byte
sector control also timed out after 120 seconds. Scoped `truss` exposed an
infinite UBLIO retry: a 262144-byte read at the provider's last cache block
repeatedly returns 261632 bytes. The installed port's `README.FreeBSD`
documents `NTFS_USE_UBLIO=0`; using it passes the complete native physical
NTFS payload lifecycle with **unchanged 4096-byte AES-XTS-256 GELI**.
Shared commit `68b768c` sets this only for the exact trusted native `mkntfs`
and `ntfs-3g` paths after clearing the environment. With the owned guard
removed, product container (seven operations) and physical partition (14
operations) lifecycles both pass, including Auto and conformance guards.
This was an ordinary driver cache configuration gap, not an OS or sector-size
impossibility. The final installed container and physical replay also passes.
Native fixture is `native-geli4096-ntfs-noublio-1795731158`, product
container `lurker-fs-cli-miq3lut4`, physical `product-geli-preview-1795731313`
on `/dev/vtbd2p7`. Native PEFS over fresh NTFS failed a nested Bitcoin write
with `EBADF`; over exFAT it failed opening the nested payload with `ENOENT`.
These controls did not use Lurker's metadata symlink, so removing that symlink
would not establish a valid lifecycle.

Stable unmodified upstream CryFS, with the product's AES-256-GCM profile,
**passes the full fresh payload lifecycle on both NTFS and exFAT**. The bundled
engine fails initialization because the common durable publication patch uses
hard links: NTFS returns `EIO`, exFAT `ENOSYS`. This is a **product gap**, not an
upstream CryFS limitation. Matched control root is
`fixtures/publication-controls-1795730212`. Native exclusive rename preserved
an existing sentinel with `EEXIST`, but publication to an absent name and all
eight concurrent contenders returned `EOPNOTSUPP`; directory fsync succeeded.

The FreeBSD 15.1 `fuse_vnop_rename` implementation explicitly rejects every
nonzero `ap->a_flags` before dispatching to userspace. Both installed drivers
link FUSE 2; FUSE 3.18.3 is also installed, but a library-only rebuild cannot
supply the missing kernel operation. exfat-fuse 1.4.0's FUSE 3 rename callback
also ignores its flags. A correct fix would need native kernel FUSE RENAME2
plumbing and driver no-clobber support, not a check followed by ordinary
rename, a partially visible target, or weakened durability. Custom kernel
work is deferred by the round instructions until the complete ordinary-route
assessment is reported. No such custom build has started.

The private `cases/applicability.json` retains the initial **130 method/filesystem
cells**, receipts and scoped retests. The separate
`cases/applicability-installed-d601bc5-with-recovery.json` reconciles all 130 current
cells, installed file/device/directory-backing routes, writer credentials,
failed stock shutdown, clone recovery and the later PEFS guard without rewriting
that history. GELI/NTFS raises installed lifecycle passes to 31; the original 41
attempts remain 30 passes and 11 non-passes. The installed replay is detailed below.
OS-specific hdiutil,
diskutil, CGD and softraid methods are not FreeBSD providers. No eligible native
LUKS1/LUKS2 read/write implementation has been established: libluksde is an
experimental read-only LUKS1 reader and explicitly excludes LUKS2. libfsapfs is
an experimental read-only APFS reader. Cryptomator's selected Java FFM provider
has Linux/macOS/Windows ABI layouts
without a FreeBSD provider; actual native assessment below leaves this as
feasible integration work, not an unavailable platform. These
are explicit current limitations, not compatibility-layer passes.

Representative baseline conformance completed **11 suites, 132 passed,
0 failed**: GELI password/keyfile with Auto, PEFS password/keyfile with Auto,
VeraCrypt AES/Serpent/Twofish each with password/keyfile and Auto, and login-user
gocryptfs. Cases include overwrite refusal, duplicate/alias acquisition, wrong
credentials, busy-close preservation and normal persistence. The existing
SQLite WAL workload ran only after reading the database instructions.
Ordinary-user staged helper GELI/UFS2 binary-key create/three-open lifecycle
also passed seven operations with native helper listings; an earlier private
JSON field typo is retained separately.

## Physical controls and product integration

A separate fresh 4 GiB qcow2 at
`/srv/build/lurker-native-fresh-20261127/freebsd/device.qcow2` is attached to
`freebsd-core-01` as vdc, guest `/dev/vtbd2`, serial `lurker-fresh-fb-data`.
Size 4294967296, serial, topology and idle state were verified before any
partition or format command. GPT partitions and failed fixtures are preserved.

Native controls passed full create/three-open payload/append/final-hash/normal
close cycles for GELI/UFS2 on `/dev/vtbd2p1` and private VeraCrypt/UFS2 on
`/dev/vtbd2p2`. They are native controls, not product passes.

Owned adapter commits `ff9f312` and `3130c21` add immutable GEOM inspection,
separate idle/system/boot/swap protection, physical GELI operations and
recovery while retaining vnode-backed md identity. Active identity uses stat,
rdev, GEOM geometry, partition UUID, disk serial, offset and sector checks
without opening an exclusively attached raw provider. Native scoped checks
passed 13 tests with two explicit privileged ignores; the root-owned ignored
test then passed real active-GELI identity, backing and mapping verification,
busy rejection, normal detach and system-disk guards.

Shared product integration preview (base `3130c21` plus hashed root-owned
source overlays) built CLI/helper, passed core 65/0/11 and CLI 19/0 tests.
Fresh `/dev/vtbd2p4` UFS2 product lifecycle passed **14 CLI operations**, including
payload persistence, explicit/Auto opens, wrong binary key, duplicate open,
busy-close preservation, no-force refusal, system/build-pool disk refusal and
partitioned-whole-disk refusal. Preview fixture is
`product-geli-preview-1795730659`. Earlier p3 creation succeeded but the private
wrapper called a function absent from its older harness; that failed receipt
and its closed partition are retained. The final installed physical matrix and
the later desktop physical-source
controls below supersede this preview; its source identity remains separate.

After that preview, owned crypto mappings and plaintext mounts had closed. The
scoped conformance nullfs mount and dedicated build pool remain intentional.
Each completed user-FUSE batch restored `vfs.usermount` to its original zero.
The newer final matrix below uses its own fixtures. Final cleanup and orderly
reboot persistence have not passed; the failed ordinary shutdown is recorded below.

## Final ordinary candidate and remaining native gaps

Exact source `d601bc5c4d2af95ecf24145829d7c282a8284791`, native run
`20261126T232638Z-7ffd0e78`, built all three products and passed workspace check
and **139 tests, zero failures, 13 explicit native-fixture ignores**. The consistent
profile is dev with debug information and incremental compilation disabled.
Canonical staging under umask 077, native package generation, two install/check/
remove cycles, the final reinstall, file hashes and root-owned public modes all
passed. Archive SHA-256 is
`81d7816a0117720b37542d7655eef4de0105148f634158a29dcf8f87256c7f05`;
native package is
`1c953abd084185482e21793770476b9fd414ec653fe89789ff584308516d4a45`.
Installed products are:

- CLI: `7844aaf653305ad696aa0ba2272d9c705f6abd484de8787c268da0835267e751`
- helper: `38c4682c6a3c4fcf75ae59b2b50d3a8a403bfef20a75dfe49d8d19d9c90e7588`
- desktop: `400bb6e30cc218221883b34501ea8b75b227a38a6eded9455f21785f33763f16`

FUSE, including its userspace library, remains a host prerequisite. The final
VeraCrypt SDK contains only private libiconv. Its engine bytes remain
`92edaf1363133aab8844ddd2c01656174967e260cd046569bd2dcbac64eec8b4`.
The host ABI is `libfuse.so.2`; `/usr/local/lib/libfuse.so.2` resolves to the
administrator-owned versioned provider. Producer hashes record build inputs;
compatible administrator updates are accepted after current ownership,
symlink-chain, SONAME, engine-NEEDED and actual cleared-environment loader
resolution checks. The first native FreeBSD runtime test caught an alias-versus-
canonical-path comparison bug in `c7f7092`; shared fix `d601bc5` passes all three
root runtime tests. The independent active-GELI/read-only system-device guard
test also passes. Failed intermediate package/runtime receipts remain retained.

The installed desktop opened a real winit/software-rendered native X11 window
as UID 1300 with no loader overrides. Its fresh frame was visually inspected.
Earlier native helper-backed GUI callbacks passed GELI and physical VeraCrypt;
the physical create/open fields and actual 120% scrolling to both action
buttons passed. Root commits `ca14341` and `2e93f78`, and owned `507f9ea`, retain
those UI changes. The final package window check supplements those callbacks.

The fresh final device is the independent 8 GiB qcow2
`/srv/build/lurker-native-fresh-20261127/freebsd/device-final.qcow2`, domain
`freebsd-core-01`, target vdd, guest `/dev/vtbd3`, serial `lurker-fb-final-data`.
Native size/serial/idle state were verified before partitioning. Eighteen GPT
partitions are now retained, including a failed private helper attempt on p17
and its successful fresh p18 retest. Final container and device batches use the
same installed binaries;
all **14/14 physical rows** (seven filesystems times GELI/VeraCrypt) have passed
108 CLI operations. All original hashes match, all append hashes match the final
five hashes, all targets are UID/GID 1300 mode 0700 and all 14 closed normally.
Both physical Serpent/Twofish binary-key variants also pass.

The final installed file/container and directory-on-file-backed matrix has
**41 attempts: 31 passes, six guards and four retained directory gaps**. All 31
passing receipts match the original pinned hashes and append/final five-file
hashes, and all 41 close. The six guards are Btrfs/XFS for both disk methods
(policy/integration pending) and APFS for both (no qualified write route).
The four gaps remain PEFS and bundled CryFS on NTFS/exFAT.

The separate directory-on-physical-device batch has **21 attempts: 17 passes
and the same four gaps**. This covers PEFS/gocryptfs/CryFS on all seven actual
native filesystems backed by the fresh GELI partitions. Passing PEFS/HAMMER2 is
qualified to its original same-HAMMER2 plaintext target; the external ZFS target
limitation below must not be inferred to pass. All completed inner and outer
mounts closed normally before the later reboot-access batch reopened them.

Installed conformance adds **36 passes, zero failures**: CryFS and VeraCrypt AES
with password and binary-key credentials, including explicit/Auto and negative
controls. The earlier 132 passes retain their own baseline source and artifact
identities; unchanged PEFS/gocryptfs and additional VeraCrypt option coverage is
carried from those fresh controls, not relabeled as installed-package results.
Installed helper file and fresh physical p18 lifecycles each pass seven
operations, actual UID 1300 payload I/O, three payload opens, pinned hashes and
normal close. A private plan permission failure and a parallel native-listing
deadline on p17 are retained; p18 used a readable sanitized identity and a
240-second native-query deadline on an otherwise quiescent host.

### Actual caller access and failed ordinary shutdown

Historical full PDF writer/reader credentials were audited separately from
ownership metadata. The final 14 direct rows, GELI/VeraCrypt file rows and PEFS
payload rows used UID 0; `SUDO_UID=1300` selected target ownership but did not
change the writer's effective UID. gocryptfs/CryFS and both final helper payloads
used actual UID 1300. Root-created payload file ownership is not proof of caller
access and is not silently reassigned.

A subsequent scoped access batch passes **33 actual UID 1300 nonce-file
write/fsync/read controls** and **33 unrelated UID 65534 list/read/write denial
controls**, with every plaintext ancestor searchable: 14 physical rows, two
helper-created fixtures and 17 qualified directory backings. These are access
proofs, distinct from the historical full PDF root-writer phases. The nonce
files were retained for post-reboot checks.

That access batch exposed a genuine PEFS/HAMMER2 mount-target limitation. With
the plaintext target on ZFS, both root and UID 1300 create/fsync succeed but
lookup/read returns ENOENT. Filename lengths 8/33/64/80/128 all reproduce it;
plain HAMMER2 controls pass, and making the ciphertext fixture ancestor
searchable does not fix it. A fresh native PEFS-only control with the same
AES256-XTS/600000 profile reproduces all five failures on an external ZFS
target and passes all five on a same-HAMMER2 target. The installed product at
its original same-HAMMER2 target passes caller write/read and unrelated-user
denial. Failed external-target mounts closed normally, and receipts/files were
preserved. This is not attributed to caller permissions or filename length.

A subsequent matched native HAMMER2-source/UFS2-target control passes all five
filename lengths as actual UID 1300, alongside the same-HAMMER2 positive. The
restriction is therefore scoped to the proven HAMMER2-to-ZFS placement, not all
cross-filesystem targets. The product now checks native source/target filesystem
names after verifying the empty unmounted target and before session publication
or backend writes. Only that pair is rejected, with HAMMER2/UFS2 alternatives.

The owned guard overlay on exact base `0c548ded6c2e5700a1e37e9d699c6cfe257c0b2f`
passes native core **75 tests, zero failures, 12 explicit ignores**, plus CLI
build. The pre-mutation native negative proves identical ciphertext tree hashes/
metadata, registry records, mount table and empty target before/after rejection.
The same source then passes product opens at both HAMMER2 and UFS2 targets,
canonical hashes, actual caller nonce writes/fsync/reads, unrelated-user denials
and normal closes. Overlay CLI SHA-256 is
`68afb9eef0464352cde6c2254c6bc949ce6f9d7036a947e3088b65ffa85a2088`;
`validation/pefs-placement-product-proof-result.json` records exact formatted
source hashes and full receipts. This is a scoped guard validation; the installed
`d601bc5` package has not been relabeled or replaced by it.

After all 33 qualified mounts and access gates were observed, the authorized
ordinary `shutdown -r now` was issued. It **did not complete**. The console
reached buffers-synced output, detached helper GELI mappings, then reported an
unrecoverable I/O failure and suspension of the final physical VeraCrypt/ZFS
pool `lurker_96445_1795736711956201591_5`. At that point SSH refused connections
and the same live VM remained running with an unchanged console; preserved-state
recovery below is separate. No force reset, detach, hidden
reconciliation or shutdown hook has been introduced. Local
`validation/shutdown-observation-20261127T010848Z/` retains console SHA-256
`8f6105d19cc6c318c99d6b8e8e9e87c85ae858e476ee58f75bb8e71b03ca0f46`,
domain XML, QMP state, block backing paths and resource observations. The full
cohort/access receipts were subsequently collected from the verified
recovery copy. An offline audit confirms all 33 access pairs, searchable
ancestors and prepared hash sets; the external-target failure remains retained.

Primary FreeBSD 15.1 shutdown sources and VeraCrypt's signal/mount-discovery
sources support a transport-ordering hypothesis: user processes terminate
before kernel filesystem shutdown, while ZFS's pool-name mount does not match
VeraCrypt's literal md-device discovery. The exact transport death sequence is
not proven without guest diagnostics. Console swap-pager exhaustion warnings
are retained. Persistent syslog places
them at 01:06:26UTC, 39 seconds before the 01:07:05 shutdown request. No matching
process-killed record was found. Pressure preceded shutdown, but its contribution
and an engine OOM death remain unproven; no immediate pre-shutdown guest VM/RSS
snapshot was captured. No reboot or final clean-resource
pass is credited. All qcow backing files are preserved.

### Preserved-state recovery and ordinary host prerequisites

The coordinator approved a reversible recovery of this dedicated VM. Native
libvirt saved the failed RAM/device state with `save --paused`; the domain was
verified shut off. The root-owned 0600 memory image is 2415777921 bytes.
All four standalone qcows were copied offline, and each original/copy SHA-256
pair matched before boot. Original RAM, XML and disks remain unchanged under
their retained paths. Recovery root is
`/srv/build/lurker-native-fresh-20261127/freebsd/shutdown-recovery-20261127T011325Z/`;
local receipts under the matching `validation/` directory retain all four
hashes, XML, commands and actual block-source checks.

The same domain UUID, MAC, disk serials and normal libvirt labeling were
preserved; only the four source paths changed to verified copies. The guest
cold-started on those copies at 01:19:32UTC, with healthy root/build pools.
No saved RAM was restored concurrently, filesystem repaired, pool force-imported
or shutdown hook added. This is recovery after a failed shutdown, not an
ordinary reboot pass.

Small-group recovery with one directory engine at a time verifies **29 of 33
unique fixtures**: 13 direct rows, two helper fixtures and 14 directory backings.
Canonical hashes, persisted caller nonce reads, unrelated-user denials and
normal closes pass, including physical VeraCrypt/ZFS. GELI/ext4 refuses a normal
read/write mount; read-only dumpe2fs reports an unclean filesystem and
`e2fsck -n` returns 4 with inode-bitmap/free-inode-count inconsistencies. Every
repair prompt was declined. A native read-only GELI/ext4 diagnostic recovers all
five canonical hashes and the caller nonce, then unmounts/detaches normally.
It is not a product read-only feature or a Create+Read/Write pass. Its three
directory backings remain blocked by the failed write mount.

The first cold-recovery user-FUSE attempts also exposed an ordinary host setup
gap: fusefs had been manually loaded before shutdown, but no boot configuration
loaded it. With no `/dev/fuse`, those attempts failed and their JSON exit/argv
receipts and unique backend diagnostics remain retained. Affected routes pass
after an explicitly recorded native `kldload fusefs`. The approved persistent
ordinary setup now adds fusefs to `kld_list` and sets `vfs.usermount=1` in
`/etc/sysctl.conf`, with private before-config backups. Native `/dev/fuse` is
root:operator mode 0666 and actual UID 1300 has read/write access; no devfs rule
was changed. Its later automatic boot availability is verified by the explicit
service reboot below. Runtime user-mount authorization 1 is now intentional host setup,
separate from the earlier temporary batches that restored zero.

One private retest controller reused early terminal-log basenames; its v2 JSON
receipts and unique failed-session backend diagnostics are preserved, and the
log snapshot explicitly identifies that mixed-phase limitation. Future phases
use distinct prefixes. No earlier failed prerequisite receipt was relabeled.
After Java/font installation and recovery, the unchanged installed desktop
again passes a real UID 1300 X11/software window probe, with children reaped,
no loader overrides and a visually inspected fresh frame.

### Native termination protocol and explicit opt-in service assessment

A fresh native-only VeraCrypt/ZFS control isolates termination from Lurker.
It uses the same qualified 1.26.31 engine (`92edaf...`), pinned source and the
existing three reviewed patches; none changes shutdown. A unique 1 GiB file,
direct engine/native ZFS commands, four engine opens and five actual UID 1300
payload phases preserve all original/final hashes. The exact native handler
and FUSE-service PIDs were distinguished by source-scoped process records and
procstat file descriptors. TERM to only the handler makes it exit, while the
transport, ZFS pool and caller-readable data remain live. Native verbose
metadata reports an empty Mount Directory for the pool. An explicit native
export/dismount, another reopen/read and normal export/dismount all pass.
There was no SIGKILL, VM reboot, repair or forced cleanup in this control.

This demonstrates a native termination/export gap distinct from Lurker's
correct explicit normal close. It does not prove the exact signal sequence
inside the earlier failed whole-VM shutdown. The single native service/handler
RSS observations are 12496/10240 KiB; top/swap snapshots retain whole-guest
resource evidence. The failed first private observer used an invalid mdconfig
argument after attach; it closed normally before any pool creation and remains
separate. The successful receipt is
`validation/native-veracrypt-zfs-signal-control-v2-result.json`.

An optional native rc.d service and administrator instructions are now supplied
under `native/freebsd/`. Packaging includes only dormant text files under
`share/lurker/host-setup/freebsd`, never an rc.d installation or enablement.
Default enablement is NO. An administrator supplies an ordered account/tab/
mountpoint list, reviews `oneplan`, and explicitly installs/enables it. The
service validates the complete list and canonical root-controlled executable/
config ancestor chains before any close, then uses only normal identity-checked
product unmounts, dropping to each configured user for directory mounts.

Manual native prototype qualification passes on one fresh managed VeraCrypt/ZFS
fixture: three actual UID 1300 full-payload phases and normal service closes,
a Unicode/spaced mountpoint, default-disabled no-op, bad-later-line prevalidation,
and nonadministrator/writable/symlink-list rejection with unchanged registry and
mounts. A real busy holder produces refusal with the mount preserved; releasing
the holder permits normal close. Native rcorder includes the KEYWORD shutdown
prototype before init's later process termination. It remains outside rc.d and
disabled during these manual tests. A busy failure cannot veto OS shutdown;
that limitation is explicit in the operator instructions.

Native stager checks pass **57 tests, zero failures**, including umask 077 public
modes and proof that these assets are dormant, pinned source files. Manual
protocol evidence is `validation/freebsd-shutdown-service-control-result.json`.
The first manual prototype remains separately identified by script SHA
`9002fce976248f916993be88f8c697090e82561a22845f432bca0040b0dce4c0`.
Review then required an immutable bounded list and strict tab/duplicate parsing.
The revised service captures at most 64 KiB in a private root-owned 0400 file and
uses that exact snapshot for validation and execution. Its raw-line parser
rejects extra/trailing tabs and duplicate mountpoints, including existing aliases.
Temporary snapshot files are removed on normal/error exits.

Fresh revised proof uses a gocryptfs vault inside a new VeraCrypt/ZFS volume,
with actual UID 1300 payload write/append/final phases and three service closes.
Each close confirms the non-root directory branch completes before the root
outer branch. A busy inner holder preserves both layers; releasing it allows
normal close. Twelve native prevalidation/no-op checks preserve both mounts and
registry state. Concurrent atomic list editing makes 741 replacements across 40
plan calls: 23 complete valid snapshot plans are accepted and 17 invalid snapshots
are rejected, with no mixed or unvalidated output. A native syscall/name trace
and successful snapshot cleanup are retained. Current script SHA-256 is
`7acc546b7dcda537ac7b286e98725287dcf31ef9093a572bf84534b62c25ca8d`;
`validation/freebsd-shutdown-service-control-v2-result.json` identifies that
revised native proof. It remains dormant and outside rc.d. The explicit list is
for the active cohort; already-closed/unowned targets cause normal refusal, not
automatic registry enumeration or error-string-based skipping. That operator
contract is documented separately from a general idempotent shutdown manager.

The explicitly activated ordinary reboot subsequently **passes** for one fresh,
measured cohort: a 1 GiB VeraCrypt/ZFS file plus one inner gocryptfs vault. Four
crypto/supervisor processes total 109876 KiB RSS; 28 processes, no builds and the
full 1 GiB swap were free before shutdown. Installed script/config/list and
native rc order are recorded. Serial capture proves the user directory closes,
then the outer native container closes, before syslogd/global termination and
normal buffers-synced/Rebooting output. Boot identity changes from
1795742372 (01:19:32UTC) to 1795745832 (02:17:12UTC).

Before any postboot mount or manual prerequisite restoration, fusefs.ko is loaded,
`/dev/fuse` is native root:operator 0666, and `vfs.usermount=1`. Both layers reopen
normally for read/write; all five append/final hashes match. The persisted caller
nonce, a fresh postboot UID 1300 write/fsync/read, and unrelated UID 65534 denials
all pass. Registered-service normal close succeeds. Activated rc.d/rc.conf.d/list
files are archived and removed by exact hash; intentional persistent FUSE setup
remains. No forced power action, filesystem repair, force import or error-string skipping occurred.

The local serial log is `validation/service-reboot-console.log`, SHA-256
`1a4b92f864d3ba43c825b4475ef2ff8eb373277677bd1cf46c7ea7ac52c40490`.
`validation/freebsd-shutdown-service-reboot-result.json` retains full observations;
`validation/service-reboot-evidence-index.json` correlates recipe/helpers/list/
config bytes. The activation archive had a basename collision: the rc.conf.d
copy overwrote the rc.d script copy. The unchanged executed recipe/state remain;
the separately protected prototype is collected as an explicitly labeled
byte-equivalent script, matching hashes asserted before and after reboot. That
archival limit is not hidden. The coordinator independently verifies the complete
milestone in `validation/root-shutdown-service-reboot-audit.json`.

This qualifies the explicit small-cohort administrator setup. It does not replace
the failed stock shutdown, the 29/33 clone recovery, or all-method final-package
and ordinary reboot qualification.

Measured filesystem features below describe the created native profiles, not
all capabilities available under other mount/format options. Both backend rows
retain raw probes in `device/final-direct-d601bc5.json`.

| Filesystem | Mode/owner mutation | Additional ACL entry | Native observations |
| --- | --- | --- | --- |
| ext4 | Both observed | Not supported by this mounted driver | `dumpe2fs` reports `has_journal`, `metadata_csum` and CRC32C; journal replay was not exercised |
| UFS1 | Both observed | Disabled in this profile | Soft updates enabled; soft-update journaling and gjournal disabled |
| UFS2 | Both observed | Disabled in this profile | Soft updates enabled; soft-update journaling and gjournal disabled |
| HAMMER2 | Both observed | Not supported by this mounted driver | Advanced feature controls not exercised |
| NTFS | Fixed mount UID/GID 1300 and mode 0700; chmod 0640 was not reflected | Not supported in this profile | Native OEM identity verified; publication hard links return EIO |
| exFAT | Fixed mount UID/GID 1300 and mode 0700; chmod 0640 was not reflected | Not supported in this profile | Native OEM identity verified; hard links unavailable |
| ZFS | Both observed | NFSv4 ACL mutation passed | Checksum/compression on, sync standard, copies 1; snapshot create/destroy passed |

Btrfs/XFS have separate positive native payload controls below, with product
eligibility/integration still pending. APFS has no qualified native write route.
No snapshot, compression, journal-replay or checksum-corruption result is
inferred where it was not exercised.

Ordinary NTFS source assessment also built Tuxera 2026.9.28 natively in a
separate root-owned host-tool prefix with the official FreeBSD aligned-I/O
patches. The configured package remains 2022.10.3; the ordinary source control
preserves its own logs and binary hashes. Unmodified upstream CryFS passes the
full payload on the newer driver, while the bundled durability patch still
fails hard-link publication with EIO and kernel rename-no-replace still returns
EOPNOTSUPP. This newer ordinary driver does not solve the product gap. Additional ordinary `direct_io`, `use_ino` and combined NTFS mount-option
controls all retain hard-link EIO, rename-no-replace EOPNOTSUPP and successful
directory fsync (`native-ntfs-mount-options-1795736666`). No custom kernel module
or weakened publication/durability workaround has been used.

Additional native XFS and Btrfs controls now pass all three payload opens with
host `lklfuse -o direct_io`. Both also pass through GELI with AES-XTS-256,
PBKDF2 iterations 600000 and 512-byte sectors; all closes and native detaches are
normal. Controls are `native-lkl-directio-v2-1795733146` and
`native-geli512-lkl-directio-1795735582`. The installed official port is
`fusefs-lkl-4.16.g20180628_5`, GPLv2 source
`86dd3afb590eccc1903611bdaa8bac87757eb80d`. This is a native FreeBSD ELF process
statically embedding Linux Kernel Library code with host threads, file I/O and
host FUSE; it has no VM or network layer. Upstream is active, with inspected
commit `d0f76a77e8c2ca68bd7d96190b8da09040eab072` dated 2026-09-28. Product
mount/geometry changes were initially held for eligibility clarification. The
coordinator now accepts this official separately installed native host route:
the exclusion concerns VM/microVM/WSL/network workarounds, none of which this
driver uses. Btrfs/XFS product integration is required ordinary work, using
`direct_io`, native formatters and selected 512-byte GELI geometry while preserving
AES-XTS-256/PBKDF2-600000. Both inner sources and directory backings still need
actual product and caller-access qualification. Native controls remain separate
from product passes; no name-based impossibility is claimed.

### Cryptomator ordinary native source assessment

Native Maven 3.9.16 and OpenJDK 25.0.5.1+2-freebsd-1 were installed after the
immutable crypto matrix. All three private crypto engines and the host FUSE 2
provider retained their prior hashes. Native Java FFM works on actual FreeBSD
amd64: the SysV x64 linker calls getuid and returns UID 1300. Native FUSE3 headers
report `struct stat` size 224 with st_size at offset 112; the selected Linux
jFUSE generated binding uses offset 48. Selecting that Linux binding would be an
ABI error, not a valid port.

The unchanged pinned helper and locked runtime dependencies build natively.
Cryptomator initialization succeeds as UID 1300; mounting fails because neither
the helper selection nor upstream service providers supply a FreeBSD ABI.
An ordinary native FUSE 3.18.3 passthrough control demonstrates the host route:
the upstream example needs documented config direct_io for FreeBSD read/write;
its fsync stub was replaced by real native fsync/fdatasync before counting
persistence. That control passes the canonical full payload across three
mounts, append/final hashes and normal closes as UID 1300. The final fixture is
`native-fuse3-directio-fsync-control-1795740528`; failed cached-write controls and
the earlier fsync-stub experiment remain separate.

This establishes native Java/FUSE feasibility, not a Cryptomator product pass.
Genuine FreeBSD structure/errno bindings, provider registration and helper
selection remain integration work. No OS masquerade, copied crypto logic,
shared core enablement or custom kernel change was used. New Java packages also
installed fonts; the prior installed native desktop-window result remains
identified, and the post-install/recovery window repeat above also passes.

## Primary ordinary-route sources inspected this round

- https://docs.freebsd.org/en/books/handbook/filesystems/
- https://cgit.freebsd.org/ports/plain/security/veracrypt/Makefile
- https://github.com/kusumi/freebsd_hammer2
- https://raw.githubusercontent.com/libyal/libfsapfs/main/README
- https://raw.githubusercontent.com/libyal/libluksde/main/README
- https://raw.githubusercontent.com/cryptomator/jfuse/develop/README.md
- https://raw.githubusercontent.com/cryptomator/fuse-nio-adapter/5.1.0/src/main/java/org/cryptomator/frontend/fuse/mount/LinuxFuseMountProvider.java
- https://raw.githubusercontent.com/cryfs/cryfs/1.0.3/CMakeLists.txt
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/sys/fs/fuse/fuse_vnops.c
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/sys/kern/vfs_syscalls.c
- https://raw.githubusercontent.com/tuxera/ntfs-3g/2022.10.3/src/ntfs-3g.c
- https://raw.githubusercontent.com/relan/exfat/v1.4.0/fuse/main.c
- https://raw.githubusercontent.com/freebsd/freebsd-ports/main/filesystems/ntfs/files/README.FreeBSD

Additional primary source inputs:

- https://download.tuxera.com/opensource/ntfs-3g_ntfsprogs-2026.9.28.tgz
- https://github.com/lkl/linux
- https://api.github.com/repos/lkl/linux/commits/master

Shutdown source inputs and native ABI/provider assessments:

- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/sbin/init/init.c
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/libexec/rc/rc.shutdown
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/libexec/rc/rc.d/zfs
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/sys/kern/kern_shutdown.c
- https://raw.githubusercontent.com/veracrypt/VeraCrypt/VeraCrypt_1.26.31/src/Driver/Fuse/FuseService.cpp
- https://raw.githubusercontent.com/veracrypt/VeraCrypt/VeraCrypt_1.26.31/src/Core/Unix/FreeBSD/CoreFreeBSD.cpp
- https://github.com/cryptomator/jfuse
- https://github.com/libfuse/libfuse/blob/fuse-3.18.3/example/passthrough.c

## External LKL driver qualification checkpoint

Commit `c045e7b` centralizes the fixed GELI create profile and passes the
selected filesystem through the file and physical adapters. Its frozen native
four-file snapshot passed workspace check and **150 tests, zero failures,
13 ignored**, including 77 core tests and 43 desktop tests. This is preparation
only: existing Btrfs/XFS create/write guards remain in place. The exact receipt
is `validation/geli-profile-refactor/result.json` under this host's round
evidence. The future feature documentation remains a separate uncommitted
coordinator change.

The external host-tool recipe and patch are being qualified against the
official `filesystems/lkl` source revision
`86dd3afb590eccc1903611bdaa8bac87757eb80d`, source archive SHA-256
`d63cccf64ab8938389b0054e3e68ee467100bfc98475b1864ab7dac542d51317`,
and all seven FreeBSD port patches from
`46cf3d9696491c61d91ecd22db11c7f7c1e62c24`. This remains an external native
FreeBSD filesystem prerequisite, with host-installed FUSE; it is not a private
Lurker engine or a VM/network workaround. No new product support is credited
from the following diagnostic controls.

Diagnostic v6-NX, SHA-256
`d95f9572f3df2eef16213667f0f3bf4082fff9f2dcd7e8eda0c461cb1f9c1094`,
passes complete native XFS and Btrfs write/append/final-reopen payload cycles
both on regular images and through GELI AES-XTS-256, PBKDF2/600000 and 512-byte
decrypted providers. Every payload actor is actual UID/EUID 1300. New inode
UID/GID, mode changes, native primary and supplementary group access, and
unrelated-user list/read/write/create denials through searchable ancestors
pass. Native errno checks include Linux/native ENOTEMPTY 39/66, ENOSYS 38/78,
EOPNOTSUPP 95/45, EAGAIN 11/35 and missing-xattr ENODATA 61/ENOATTR 87.
Kernel-held release/dir-release are cleanup only; genuine I/O and metadata
requests with a dead or PID-zero caller remain unsupported and fail closed.

The twelve native syscall traces prove source-fd regular-file `fsync` or GEOM
`DIOCGFLUSH`, source close before the final status record, and **no unmount
syscall or force flag** in the corrected driver. Each route has strict
identity-bound ready/drained records, busy preservation, and active/final
status reuse rejection. Native ELF checks show separate RX/RW loads and a
non-executable GNU stack. The source remains under review to strengthen the
INIT-reply/monitor ordering and bounded cancellation before a final linked
host-tool freeze; diagnostic and factory identities are kept distinct.

The actual UID/EUID-1300 injected native source-fsync test also passes: unarmed
write/fsync succeeds; armed application `fsync` receives EIO; normal host
unmount succeeds; the driver exits nonzero and publishes the matching final
**error**, never drained. The loader override is confined to this unprivileged
diagnostic; it is not a production/root execution option. A preceding
unprivileged `allow_other` attempt was rejected by native mount policy, then
normally signal-aborted without publishing a host mount or writing payload.
Both results are retained separately. Receipts are in
`validation/lkl-driver-v6-nx/`, including its `geli512/` trace audit.

An earlier v5 diagnostic remains a real failed startup. FreeBSD libfuse starts
`mount_fusefs` asynchronously; eagerly checking mount identity before serving
INIT rejected a not-yet-published mount and closed its channel. The helper
subsequently published an uninitialized native FUSE mount. No LKL filesystem
mount or payload write occurred on this 512-MiB image. Its SHA-256 before and
after the recovery attempt is unchanged:
`9701aef9565098770f499932fcc0f4066c49a6cacaccae40847260f9b630ec6b`.
The image remains at
`/srv/lurker-fresh-20261127/fixtures/lkl-native-request-lklfuse-host-io-diagnostic-v5-1795749524/xfs/image.raw`.

Normal unmount blocked. After preserving native mount/process/descriptor
snapshots and proving no remaining `/dev/fuse` descriptor, the coordinator
authorized an explicit recovery-only forced unmount of this exact reverified
FSID `[-318701815,237]`, source and target. That recovery syscall is **still
blocked**, not completed or passed. No reboot, reset, repair, other mount
mutation or encrypted-mapping detach was performed. The exact retained
process/birth identities are recipe `34031/1795749524.464375`, read-only find
`36854/1795749607.176385`, normal unmount `39624/1795749746.348368`, and scoped
forced-recovery helper `42884/1795749945.800178`. Exact birth-checked SIGTERM
attempts for the find and normal-unmount processes remain pending in their
kernel waits. The mount's absence from a later cached mount-table snapshot
does not establish completed recovery while these operations remain blocked.
The recovery evidence is `/srv/lurker-fresh-20261127/lkl-v5-failure-recovery/`.
No additional recovery action is being taken; later bounded controls use
fresh, distinct paths and do not traverse the failed target.


### Final external LKL prerequisite candidate, before product admission

The final11 native client and rebuilt host library use the audited immutable
kernel-object/header cache while preserving every relevant kernel/compiler
input. The exact native executable is
`a6450b6f08bf9ebff09d75f5ebb2c6695df79bf94f1b995f2af1bc9c859dbd9f`,
with host patch
`54598d98f4eb5170fa3478448e87130be0737cac1db7deb54a19cc21f7081781`.
It is PIE with a non-executable stack. FUSE remains the administrator-installed
`libfuse.so.2`; current provider provenance is recorded without freezing
compatible future administrator updates.

The client now constructs an unmounted native channel and completes its FUSE
constructor and LKL startup before the native descriptor-handoff helper can
publish a mount. Readiness follows a successful actual INIT reply and the
exact native identity. Fresh unknown-option and invalid-filesystem controls
return errors before helper/mount publication, with unchanged source hashes.
A pending SIGTERM before execution/startup also returns an error after LKL
startup without publishing a native mount. All twelve native producer/unit
checks pass, including real libfuse channel delegation and single close,
failed-send readiness, global moved-FSID/source/replacement absence refusal,
and emulated restoration faults. The latter is a compiled-client syscall mock,
not an injected real kernel credential failure. Restoration attempts every
field, latches poison, keeps the process alive and blocks later emulated I/O.

Both regular-file filesystems and both GELI-512 provider filesystems pass all
three actual UID/EUID-1300 PDF/text phases, owner/mode updates, primary and
supplementary-group controls, unrelated-user denials, native errno checks,
active/final status reuse rejection and busy-close preservation. A SIGTERM
while each published mount is owned retains the process, mount and ready
record until ordinary unmount. All twelve raw/decoded native traces show
regular-file `fsync` or GEOM `DIOCGFLUSH`, source closure before final status,
and no driver unmount syscall or force flag. GELI retains AES-XTS256 and
PBKDF2/600000 with selected 512-byte geometry.

The one-shot barrier control fails exactly one native source fsync, disarms
before close, and observes two later successful native fsync calls. Application
fsync returns EIO, normal unmount succeeds, and the driver still exits 1 with
the matching final error, never drained. The loader override runs only as
UID/EUID 1300 in this diagnostic. Dead-origin/PID-zero genuine held-handle I/O
remains unsupported and fails closed; cleanup-only release is separate.

`validation/lkl-driver-final11/result.json` binds receipts and the two retained
archives: full native evidence
`162174db39958f763c613dfbe9dc379c6b01b1334c1a24cf59bf60a63b4e2586`
and the qualified tool/recipe/metadata archive
`b29e7a409b6c4b37d67aa4803f1d3d0bea65af2304765d495c59ba34741dcd91`.
Restaging rechecks every original source input, compiler, binary and license;
the exact tested engine bytes remain unchanged. These are external-prerequisite
results. Lurker write guards are unchanged, and product source/backing/device
and final package qualification remain outstanding. The earlier v5 failed
startup and blocked recovery above remain failures, with no further recovery
or reboot action.


### Checked-halt final12 candidate

Source review found that final11 ignored `lkl_sys_halt()`'s return value. In
pinned `arch/lkl/kernel/setup.c:119`, a negative return precedes the CPU shutdown
wait, syscall/thread cleanup, memory release and kernel-thread join; it does
not prove quiescence. Every client halt site now checks the result. A negative
return preserves the source descriptor, owned channel and status lock, reports
an error, and retries at 250-ms intervals. Only confirmed halt success permits
closure. The halt failure remains sticky after a later successful retry.
The native compiled-client fault test verifies negative-to-success behavior,
real descriptor and exclusive-lock retention, unchanged error status and the
sticky failure. This is a mocked halt failure, not an injected kernel failure.

The new actual native final12 executable is
`0535ad961bbbb06997586465288fdd47fc782b7a34f326b8e31116caecce93df`,
with host patch
`4fd6e10bbd70b132c0c0742acd574eec458d2f16725406c65a4f738e32ec10f6`.
Its twelve producer/native unit checks pass. Fresh constructor, invalid-FS and
pending-signal controls pass. Both regular-file and both GELI-512 filesystems
again pass all twelve actual UID/EUID-1300 payload phases plus ownership,
groups, foreign denial, native errno, busy and lease-reuse controls. Twelve
fresh traces again show native fsync/DIOCGFLUSH, source close before final
status, and no driver unmount/force call. The one-shot/disarmed barrier-error
control again yields application EIO, later successful native fsync, normal
OS unmount, driver exit 1 and matching final error.

Published SIGTERM has a precise separate result: it retains ready status and
service until the later normal administrator unmount, after which the driver
exits zero and reports drained. The signal itself is not a sticky I/O failure.
Final11's `a645...` receipts retain their original identity. Final12 raw/decoded
traces, source/build/binary/recipe/helper identities and receipts are retained
in `validation/lkl-driver-final12/`, bound by archive
`db3d520a847fa137c5e76555645dbe8c285ebe8127979fbb32dc448d233050ca`
(191 files). Product write guards and the previous v5 failed startup/recovery
state remain unchanged. No Lurker product or final-package pass is claimed.


The exact final12 tool is now explicitly installed as the external prerequisite
`/usr/local/sbin/lklfuse-host-io`, with root-owned metadata/license under
`/usr/local/share/lkl-host-io/`. Binary mode is 0555, metadata/license 0644,
directories 0755; all ancestors are administrator controlled. The canonical
protocol query passes. `validation/lkl-driver-final12/canonical-install.json`
records this setup; no service, private backend or product admission is enabled.

For upcoming product device cases, a new standalone 24-GiB qcow2 is attached
live and persistently to the same domain
`04179a0f-f945-4351-bb07-c4235a9e60c7` at target `vde`:
`/srv/build/lurker-native-fresh-20261127/freebsd/lkl-data.qcow2`.
Guest `/dev/vtbd4` independently reports serial `lurker-fb-lkl-data`,
25769803776 bytes, 512-byte sectors and idle access state. No GPT exists and
the first/last MiB are zero. Only read checks have run; device formatting awaits
the applicable new product-container cases. `device-lkl/` retains before/after
live/persistent XML, qcow metadata and guest identity receipts. All four
previous disks are preserved. Any later VM state-preservation/recovery inventory
must include this fifth disk; no reboot or recovery action was performed.


### Formatter preparation and live completion requirement

Native formatter preparation now checks the exact decrypted source, positive
media length, 512-byte sectors and consistent sector count before invoking
XFS formatting or fresh-empty-NTFS-to-Btrfs conversion. Existing XFS/Btrfs
write guards remain unchanged. Its native focused tests pass 2/0/0 and
workspace check/test pass with 159 passed, 0 failed and 13 ignored. The executed
formatter source is
`c1086997d735f76f9b961d30ca6eceb1a1c88838539c97385a7afbe96932a6ce`,
based on `b721dab`; evidence is `validation/lkl-formatter-prep/attempt-2/`.
The preceding E0308 compilation failure is retained separately as attempt1.

The strict status-file parser/reader is an auxiliary primitive, not sufficient
teardown authority. A visible clean record plus released flock can survive a
failed final status fsync/error rewrite while the producer exits nonzero.
A live guardian must retain the actual owned worker wait status, source custody
and identity-bound RPC completion, including parent flush/close errors, before
product admission. Its common v2 codec is being coordinated with macOS;
FreeBSD native peer credentials and GEOM/FUSE callbacks remain separate.
No encrypted provider is released solely from the file record, and no new
write admission is enabled by this preparation.


### Updated kernel-patch scope: source audit and admission hold

Further LKL builds, admission and storage tests remain held pending explicit
runtime/lifecycle coordination. Final directive `10fbc99`
allows existing supported configuration, reproducible customized builds and
established pre-made patches or documented upstream/mainstream guidance.
It excludes kernel behavior, code, ABI, hooks or ports invented by Lurker,
including embedded LKL. Excluded running, partial and planned work must be cut;
established patches are not excluded merely because they contain kernel code.
Existing artifacts and failures remain preserved. No LKL build was active when
this stop-and-cut rule arrived, and none has been restarted. The never-started
custom FreeBSD FUSE RENAME2 kernel plan is removed; no such module was built.
The retained v5 recovery processes are still nonterminal, not cancelled
or successfully recovered. Independent userspace work continues.

A read-only audit compared all 63,047 pinned pristine source entries to the
frozen final12 input manifest. It found exactly eleven content-changed files:
seven official FreeBSD port targets plus five local targets, overlapping at
`tools/lkl/lib/virtio_blk.c`. An equivalent PowerPC selftest symlink was
normalized, along with extraction permissions; there were no other content
changes or missing inputs. `validation/kernel-domain-audit/classification.json`
and `lkl-kernel-domain-source-audit-summary.json` retain exact file hashes,
URLs, changes and publication limits. `native/freebsd/lkl/README.md` lists every
official/local target and its effect.

The local patch does include `arch/lkl/scripts/headers_install.py` (forced
multiprocessing fork for Python 3.14). It also contains custom virtual block
FLUSH advertisement/request handling/error latching, native provider/barrier
host glue, host API declarations and the userspace FUSE client. Current final12
raw-provider/durability/error controls depend on those local changes; they are
not being credited as already published/tested mainstream patches. Coordinator
review classifies the seven Ports patches as established, Python fork-context
as documented toolchain adaptation, and the local virtio FLUSH/latch, native
barrier and client code as integration using existing kernel APIs without
injecting new kernel behavior. That eligibility decision does not complete
runtime admission or restart a held job. No local Btrfs/XFS filesystem, kernel/
or mm/ C changes were made, and `.config` came from pinned upstream defconfig.
Product write guards remain unchanged. Standalone userspace guardian/codec and
Cryptomator native header/FFM controls are independent of LKL execution.


Independent userspace guardian preparation continues without linking or running
LKL. Shared codec `9269641` passes 2340 native checks with exact regular/CHAR
golden frames. The native identity/held-source platform header passes 29 strict
C11 controls for PID/birth/boot, reaped/missing processes, regular source
identity/resize rejection and checked flush/close. Its CHAR ioctl branch is
compiled but not runtime-qualified here, and no completion authority is enabled.
Evidence is `validation/guardian-codec-v2/` and
`validation/guardian-platform-identity/`. Native stream credential probes also
show SCM_CREDS2 on each write, not each partial read; a wrapper must retain and
recheck the authenticated writer over a bounded frame. No embedded kernel
change or mount test is part of these standalone userspace controls.


### Genuine FreeBSD Cryptomator ABI and source-kit preparation

Debian-owned source commits `b536fa2` and `2424fb3` add a genuine FreeBSD/amd64
jFUSE provider derived from matching native FUSE3 headers. No Linux ABI or OS
identity is substituted. The frozen candidate2 archive is
`55176e8d03ae1cec85d41ce7fd5b0eb0d89a020e798e646fac7a0795d5d12680`.
Native LLVM19/JDK25 checks pass for nine structure layouts, 92 memory field
controls, all 30 registered callback upcalls and three real C-string pointer
controls. All eleven build stages and eleven focused producer tests pass.
The provider loads host FUSE3 3.18.3, constructs/closes its callback arena and
registers FSYNC, with FSYNCDIR explicitly absent. The provider test deliberately
uses a memory-only FileStore fixture: upstream real factory, fuse_new, mount,
payload and normal-close behavior remain unproven. The helper's FreeBSD mount
refusal and product capability guards remain in place.

`build/cryptomator-port-source/native-candidate-2/` retains the exact manifest,
raw logs and complete output archive
`5dbef685e0642bd434c19a5bf16d920dc8ed8969560a7771471c4c218d83de93`.
A separate metadata-only derivation adds the exact original source/recipe/POM/
lock inputs to a new image, preserves every compiled/runtime byte and passes
13 focused tests without rebuilding or invoking Java. Its final manifest is
`561337eb729ff984479356c078d955a3916ee400e6f62a2af4aa18a34d751ff0`;
new image archive is
`519840233a4a4275226a0053fcd18bc9ffcc1473461674562c2d36039cba9094`.
Evidence is `build/cryptomator-port-source/native-sourcekit-1/`.

Candidate1 remains separately blocked, not a terminal failed build: its real
FileStore factory reaches `getfsstat` and waits in native `vfs_busy`. Controller
55751 and Java55792 remain nonterminal; the original stage timeout did not reap
the kernel-blocked child. Exact starts, current process states and earlier raw
Java/kernel diagnostics are in
`build/cryptomator-port-source/native-candidate-1/`. No new signal, mount or
recovery action was used for this collection. Candidate2 does not establish
success for that skipped real factory dependency.

Before any mounted gate, source review found a concrete cleanup blocker in
pinned host libfuse3 3.18.3: `fuse_unmount` delegates to `fuse_kern_unmount`,
which closes its channel then calls native `unmount(..., MNT_FORCE)`
unconditionally. The candidate's generated unmount method still calls it.
The mounted gate is held; no force-based normal-close pass is claimed.
A source-backed alternative is ordinary native unmount followed by independent
captured mount absence and terminal-loop checks, then `fuse_destroy`, whose
session destructor closes the owned descriptor without a native unmount call.
That alternative still requires implementation and native lifecycle controls,
including busy, moved/replaced mount and startup failure cases. Exact upstream
release sources and hashes are retained in
`inventory/cryptomator-fuse3-lifecycle/manifest.json`.
