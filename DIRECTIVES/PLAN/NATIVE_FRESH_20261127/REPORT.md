# Fresh native qualification status

Round: native-fresh-20261127. Status: in progress.

No previous result is accepted as a fresh pass. The acceptance contract and host
ownership are BRIEF.md and HOSTS.md. Private case receipts and checkpoints live
under target/goal-validation/native-fresh-20261127/.

## Latest reconciliation

The chronological milestones below retain their original candidate and scope.
Recorded candidate qualification reports cover all six Linux distributions,
macOS, DragonFly and NetBSD within their stated driver/source scopes. The native
LKL eligibility clarification has reopened ordinary driver assessment on several
hosts. OpenBSD installed-package qualification and FreeBSD adapter work remain
active. Whole-round completion is unproven.

Nine host owners now provide private `completion-evidence-index.json` files
mapping the acceptance requirements to exact receipts and limitations. Root's
`audits/evidence-index-integrity.json` independently checks 4,515 indexed file
and archive-member references against frozen index snapshots, with no hash/size mismatch
in that audit. This establishes evidence identity, not requirement coverage or
the truth of every availability claim. The requirement-by-requirement review
continues. This refresh includes Fedora's metadata-only PEFS backing-role
correction and the expanded frozen indices. FreeBSD and OpenBSD indexes await
their active input batches.

The index review exposes retention and scope limits that remain reportable:
Arch lost 24 physical folder console logs while retaining structured receipts;
Gentoo deleted three initial ext4 selections later identified as ext2 before
their reclassification as failures; several Linux historical payloads lack
direct numeric EUID samples. Gentoo's old pool check used an invalid field;
new valid read-only pool/dataset commands now return empty inventories. These
later checks do not reconstruct missing historical data or failed images.

macOS report correction `3eaf842` retracts the earlier public-source blocker for
xlinuxfs/LKL. Pinned public Darwin/arm64 source and its local I/O bridge provide
a credible native build route for Btrfs/XFS. The native library now builds for
both Mach-O architectures. Host-transport correction `59b0a12` explicitly opts
verified callbacks into FLUSH and handles its two-buffer request. Native bridge
controls on ext4 and imported Btrfs/XFS passed four payload opens and actual
host-fsync/error-forwarding checks. Baseline callbacks issued no host fsync and
hid injected EIO; those failures remain preserved. Imported seeds establish
driver I/O only. Native Mac formatting and a mounted macFUSE adapter still
require separate proof; restricted FSKit entitlements remain a blocker for that
transport. Existing XFS guards remain in force. No new route is yet a product
pass, and no additional USB operation has been performed.

The Linux package audit found a separate existing-feature gap: installed
`67894f6` supplies the privileged VeraCrypt creator but lacks the unprivileged
crypto helper and a separately installed host filesystem helper. Earlier
privileged successes do not establish unprivileged operation. Resolver/readiness
correction `18692a5` separates those paths and passed 97 native Arch core tests.
Crypto-only packaging `e4c2468` and separate host-driver provisioning `6fbd474`
are committed. Host-driver correction `3f60421` makes newly formatted roots
private without changing permissions when reopening an existing filesystem.
Helper/desktop correction `b2f58d4` separates compile-time protocol support from
login-user prerequisite readiness and refuses explicit missing prerequisites
before sending create/mount secrets. Closing an owned session does not require
those backend executables to remain installed. The exact frozen native Debian
workspace check/test and helper build passed 182 tests, with 15 ignored. Root
verified the three source overlays, build/log/archive hashes, eight strict JSON
capability responses and two argument refusals. All six root/`--as-root` queries
leave readiness unchecked; they do not execute configured probes. The receipt
is `hosts/debian/build/userspace-readiness-preview/root-readiness-audit.json`.
This is source/unit/capability evidence, not installed storage qualification.
Actual unprivileged package and desktop lifecycles remain the next gate.

Ubuntu's final report is `e68bb47`: both 130-cell source-kind ledgers contain
54 passed, six stock-UFS1 unavailable and 70 platform-inapplicable cells. The
owner reports ten ordinary reboots covering 54 file/backing fixtures and three
physical partitions, all six installed `/usr/bin/lurker` user flows, helper and
GUI gates, and a clean final package/resource audit. These remain owner results;
the coordinator's earlier payload audit has a narrower scope.
Root subsequently checked the six new installed-prefix CLI receipts, all 42
operations, original/appended hashes, private roots, writable Btrfs mounts and
final package/cleanup outputs in `hosts/ubuntu/validation/root-installed-six-cli-audit.json`.
The harness resolves `/usr/bin/lurker` to the verified installed binary before
execution. This audit does not independently cover Ubuntu's full reboot matrix.

DragonFly's final report is `696c899`. Root inspected the six ordinary-reboot
receipts: five container fixtures cover UFS1, ext4, exFAT, HAMMER2 and NTFS,
and the physical sample is VeraCrypt/Twofish/keyfile/UFS1. All changed-boot
payload hashes, actual UID/EUID 1300 reads and writes, unrelated-user denials
through searchable ancestors, normal reopens and final closes match. The
retained cleanup output has no crypto maps and all vnode devices idle. Root
also reviewed the native signature checks and corroborated the retained
root-owned recipes, package/installed manifest equality and all three product
hashes in `hosts/dragonflybsd/validation/root-six-reboot-audit.json`. The verifier
has no separate contemporaneous self-hash receipt; the later collection retains
that limit and the originally observed launch PID 931.

Root independently checked NetBSD's four CGD and one CryFS reboot fixtures,
the exact executed recipes and imported payload helper, retained arm/verify
log hashes, changed boot identity, relocated physical partition UUID, payload
hashes, installed binary identities and normal closes. The CGD payload worker
drops groups/GID/UID before filesystem calls; the CryFS recipe asserts actual
UID/EUID 1300. Package verification covers 861 payload files and final cleanup
has no owned crypto processes or active CGD/vnd devices. The private audit is
`hosts/netbsd/validation/root-five-reboot-audit8e.json`. User-mount authorization
and `/dev/putter` permissions were reapplied manually after boot and restored;
this does not prove automatic prerequisite availability or resolve ZFS import.

FreeBSD's revised opt-in service is committed at `0bdbfb2`. Its native manual
proof covers bounded immutable target-list snapshots, strict separators and
duplicate aliases, twelve policy refusals, an atomic-list replacement race,
the real user-folder branch, busy preservation and ordered normal closes.
Root reviewed these inputs and authorized one fresh small-cohort VM reboot
with explicit administrator activation. That new gocryptfs-on-VeraCrypt/ZFS
cohort passed: changed boot, all five persisted payload hashes, actual caller
postboot writes and unrelated-user denials, automatic FUSE prerequisites and
normal service close. Temporary activation was reverted. Root independently
checked the serial ordering through normal reboot, exact recipe/helper hashes,
activation list/config and retained source in
`hosts/freebsd/validation/root-shutdown-service-reboot-audit.json`. An archive
basename collision overwrote the activated script copy with its rc fragment;
the audit records the before/after execution hashes and retained byte-equivalent
prototype instead of claiming that original archive instance survived. This
does not turn the failed stock shutdown or clone recovery into ordinary passes.

Preparation `c045e7b` centralizes the fixed GELI profile and filesystem geometry
selection for file/device creation. Exact native FreeBSD workspace check/test
passed 150 tests, with 13 ignored; source and log hashes were independently
corroborated. Existing Btrfs/XFS write guards remain unchanged. The external
driver's flush, credential, errno and checked-drain contract must pass before
filesystem admission and the corresponding user documentation are committed.

OpenBSD's exact `123d177` workspace passed 154 native tests, with 22 ignored,
and all three products built. Native staging and package construction passed;
the first wrapper attempt exhausted space and is retained separately from the
successful unchanged-byte retry. Package SHA-256 is
`ac36aa9fe987805bad70fe5cd5880ae962df671f75808cecdf6efd5f21f01b7a`.
Installed integrity matched all 866 entries, and all twelve block-container
cases passed. The first installed CryFS batch failed on all four backings
before initialization: runtime lookup expects an extra `cryfs/` directory,
while the stager installs the provenance file at
`share/lurker/provenance/cryfs.json`. The adjacent SDK layout had masked this
packaged-layout bug. Correction `8c6f2b6` removes that extra directory and passed
four native checks, including actual installed engine/provenance/host-loader
validation. Corrected-package CryFS/device/reboot validation is still required.

CryFS exFAT failures on OpenBSD and NetBSD are now matched to native hardlink
`ENOSYS`, while ordinary file create/write/fsync/read/rename/unlink work. This
is a bundled publication adaptation gap. The existing common patch is unchanged:
a bare exclusive-create fallback can expose partial valid block names to
unlocked readers and scans. The bounded NetBSD native lock investigation does
not justify a fallback: advisory locks release on creator death, exposing empty
or partial files, and the kernel unlocks the vnode before acquiring the advisory
lock. Its exFAT fixture additionally lost its native daemon during the partial
write control; that failed image is preserved and normal detach completed.
Source review also rejects header-last publication as an existing-format
drop-in: current readers snapshot length before reading the header, zeroed old
corruption is indistinguishable from a proposed pending state, and legacy scans
have no compatibility gate. Exact reviews are retained under
`hosts/netbsd/build/native-open-lock-review.txt` and
`hosts/netbsd/build/cryfs-header-last-review.txt`. No engine patch, weakened
integrity policy or format change was made. FreeBSD NTFS/exFAT publication
and native Cryptomator ABI integration also remain open. The earlier optional
LKL eligibility hold is resolved under the native-process boundary in BRIEF.md:
the separately installed FreeBSD port uses local host I/O/FUSE without a VM or
network transport. Its positive `direct_io` controls require product adapter
work and qualification; they are not product passes. No new custom
kernel/module experiment has begun.

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
  seven-filesystem container/device coverage passed on the subsequent recorded
  candidate. `5b9939e` adds the native desktop partition fields; its ordinary-user
  helper callback and 100%/120% layout probes passed. The final source-specific
  rerun and receipt audit remain with the NetBSD owner.
- CryFS initialization: `3178144` uses the private runtime for the temporary
  plaintext mount on every platform. The macOS APFS backing regression passed.
  Other backing failures remain independently tracked.
- FreeBSD CryFS: `eface5b` adds the pinned native private build/stager and its
  verified runtime/license closure. `1c62f1f` exposes the route in the core and
  desktop capability model; scoped native core/desktop tests passed. The full
  backing-filesystem lifecycle round is still required.

## Subsequent fixes and native controls

- Linux LUKS2/UFS1 geometry: `93e0630` selects 512-byte logical sectors only for
  UFS1, preserving cipher, key size and KDF costs, including replacement of the
  usual Twofish sector argument. Arch core tests passed (83 passed, 2 ignored)
  and CLI/helper builds passed. Fedora's prior native AES/Argon2id 512-sector
  control passed the payload lifecycle. The exact changed product now passed
  all four AES/Twofish container/device payload regressions on Fedora. Header
  inspection confirmed 512-byte sectors, 64-byte XTS keys and unchanged
  Argon2id memory costs. Stock UFS write limitations remain explicit prerequisites.
- Linux CryFS publication: `166266f` uses native exclusive rename with the
  existing durability barriers; `195b56b` registers the Linux-only patch. The
  Debian-built portable engine's archive SHA-256 is
  `fa5e8f14e008b0df5bb5faff6247bbac0810af4e04c32dc1d074a5f9e5744db2`.
  Fresh affected UFS2/exFAT product payload regressions passed on Debian,
  Ubuntu and Gentoo under their separately recorded Rust component identities.
  Namespace correctness and backing-device fsync forwarding are separate host
  driver qualifications; a payload comparison alone proves neither barrier.
- FreeBSD physical native controls now pass for GELI and VeraCrypt on distinct
  checked GPT scratch partitions. `ff9f312` and `3130c21` add immutable GEOM
  inspection, separate idle guards and active-provider identity verification.
  `e116dcc` plus the required cross-platform fallback correction `b76d730` add
  product GELI device creation, filesystem selection and BSD CLI device-node
  classification. Native core tests passed (65 passed, 11 ignored), CLI tests
  passed (19 passed), and the actual device preview passed 14 operations. The
  coordinator matched all original payload hashes and final appended hashes,
  verified the invoking user's private filesystem root and clean final mount
  inventory. `2e93f78` adds desktop GELI partition controls. The actual UI exposed
  a pre-existing scroll-area sizing defect at 120% zoom, fixed in `ca14341`.
  Native desktop tests passed (35 product tests and one private software probe);
  ordinary-user helper/GUI execution passed. The coordinator inspected both
  120% scrolled screenshots and matched all five production source hashes to
  the executed inputs. Full device-matrix and installed-package gates remain.
- FreeBSD CryFS backing qualification: ext4, UFS1, UFS2, HAMMER2 and ZFS passed.
  Both NTFS and exFAT passed equivalent unmodified upstream CryFS payload
  controls, while the bundled durability patch failed during hardlink
  publication. Native no-replace rename controls show the stock FreeBSD FUSE
  layer rejects rename flags before calling either driver. This is an unresolved
  bundled-engine/host-primitive gap, not an upstream encryption failure or an
  inapplicable filesystem. Any custom kernel-module experiment is deferred
  until the ordinary routes have been fully assessed and reported.
- OpenBSD native CRYPTO/RAID1C device controls passed all eight applicable
  filesystem combinations. The private VeraCrypt device discovery and native
  whole-disk-alias guard are implemented in `6ab0f8b`; block, raw and symlink
  whole-disk rejection preserved the checked headers, and a real partition
  passed the full payload lifecycle. Shared product physical wiring is underway.
- macOS `175ea1c` adds the native exclusive CryFS publication primitive, and
  `065eaa2` registers its independent patch input. The patched engine passed
  direct UFS2 and APFS payload regressions. `383c47b` adds the core's anchored
  directory-publication fallback for native UFS2, preserving directory xattrs,
  exclusive file moves, durability barriers and configuration-last recognition.
  Four native unit tests and UFS2 CryFS/Cryptomator, APFS both engines and exFAT
  Cryptomator payload regressions passed. Existing normal rename behavior on
  other filesystems is retained. exFAT CryFS's engine-primitive gap remains open.
- openSUSE's actual login-user CLI/helper regression passed with the narrow sudo
  authorization probe change; the machine's `sudo -n -v` failure and
  `sudo -n true` success were preserved. `7d8827e` independently fixes the test
  harness's terminal echo-readiness race. Production authorization and harness
  synchronization have distinct receipts.
- NetBSD's final workspace and seven-container/seven-device batch passed on
  `dac5010`. The subsequent mounted-reboot batch exposed native ZFS import
  refusal. A persistent nonzero `kern.hostid` did not fix it: source inspection
  identified a native macro that shadows the global host identity with a local
  zero value. The stock service does not export pools during shutdown. Failed
  pools are preserved; no forced import, ineffective preflight guard or custom
  shutdown hook is used. A module-source correction remains deferred until the
  ordinary assessment is complete. Native package installation, packaged helper
  and actual desktop-window checks passed; an installed-artifact matrix rerun
  is still required because its binaries differ from the earlier matrix build.
- All eleven applicability ledgers now contain 130 unique normalized cells.
  The coordinator's private structural audit verifies coverage of the requested
  method/filesystem names only. It does not establish runtime completion or
  the correctness of any availability claim; source-build gaps and remaining
  native qualifications continue to be audited individually.
- Linux core/CLI/helper candidate `ca7d213` is frozen for all six distributions.
  Full workspace checks and the first installed native package gates have
  passed on several hosts; complete final container/device/reboot sweeps are
  running. The subsequent UFS1 negative cases exposed a mountinfo parsing gap:
  per-mount `rw` does not override superblock `ro`. The parser and payload-gate
  correction are being tested before any completed Linux qualification claim.

## Current candidate qualifications

The six Linux follow-up sweeps use `576505d`, including the mountinfo correction:
a filesystem is read-only if either its VFS flags or its superblock flags say so.
An actual encrypted UFS1 mount exposed `rw` VFS flags with a `ro` superblock;
create and Auto reopen now reject that state without publishing new storage.
The fresh full sweeps supersede the earlier `ca7d213` candidate only for cases
actually rerun. Debian and openSUSE have completed all 60 configured-host
container cells;
their UFS1 uses pre-existing non-stock write-enabled modules. Ubuntu and Gentoo
have completed 54 writable container cells each, with stock UFS1 limitations
recorded separately. Physical-device, conformance and mounted-reboot batches
remain in progress across the six distributions. Resource-pressure and harness
failures retain their original receipts and require explicit rerun results.
The coordinator independently inspected 222 successful embedded receipts from
Arch, openSUSE, Ubuntu and Gentoo: pinned inputs equaled initial writes, appended
data equaled final reads, the new file existed, all seven operations succeeded
and the plaintext mount was closed. The private
`linux-576-payload-receipt-audit.json` records source hashes and individual checks.
That audit does not establish driver provenance, application conformance,
physical-device coverage, reboot persistence or package completion. Fedora has
also reconciled its eight capacity-related retries to 60 successful container
cells; its original ENOSPC failures remain preserved.

macOS candidate `3bfed46` includes native Apple encrypted-image ext4/NTFS/ZFS
integration and guarded VeraCrypt external GPT data partitions. Its workspace
checks passed (144 tests, no failures, 12 ignored) and its DMG was built and
verified. Extracted-package qualification remains in progress. The native
desktop callback and 100%/120% form snapshots passed before the freeze. A third
small Toshiba sample passed the VeraCrypt payload lifecycle and restored native
device ownership after normal close. The frozen DMG subsequently passed all
280 profile-conformance checks and twelve inner-filesystem lifecycles. Its
directory matrix reproduced a CryFS/UFS2 publication failure: a native
AppleDouble companion disappeared after its associated directory moved. That
failed receipt is preserved; a narrow anchored-publisher correction and a new
package freeze are still required. Buran has not been rebooted.

NetBSD's installed CGD artifact has now passed all seven container and seven
device filesystem cases, plus four representative mounted-reboot cases. This
does not remove the separate upstream ZFS reboot limitation. `7d00998` adds a
native process identity reader with live/zombie/reaped controls. The CryFS core
route is committed at `bd65b08`, with factory/stager registration at `d806711`
and desktop directory forms at `cca3c17`. It has passed a login-user PDF
write/close/reopen/append/reopen lifecycle
using host ReFUSE. SQLite WAL fails because native shared writable mapping
returns `EOPNOTSUPP`; the stock ReFUSE implementation disables the puffs page
cache. The failed fixture and native mapping control are preserved. No cache or
durability requirement has been weakened. Actual prerequisite-denial tests
left the new target absent and its parent unchanged. Native desktop tests and
the ordinary-user real-helper callback passed, with the host policy restored.
The callback needed a temporary exact-helper rule for native inventory listing;
the engine itself stayed unprivileged. The immutable `cca3c17` workspace passed
124 tests with no failures and five ignored tests. Final installed-package and
full backing-filesystem qualification remain in progress.

OpenBSD physical CRYPTO, RAID1C and VeraCrypt have each passed their four
applicable filesystem lifecycles. An additional RAID1C fixture passed explicit
one-member recovery. Desktop forms are committed at `9d2b057`: forty native
tests and a 24-frame render probe passed, followed by all three real
ordinary-user physical callbacks. The coordinator inspected the 120% mirror
forms and verified all six source overlays against executed inputs, accounting
separately for the already qualified NetBSD changes.
DragonFly has booted its updated stock world/kernel. Product physical creation
is committed at `6a4b88c`, with explicit filesystem API admission at `34872dd`.
Both methods passed all five applicable physical inner filesystems. Its desktop
physical forms are being tested in a separate source overlay.

BSD SDK/stager changes `782a088`, `88d9315` and `c7f7092` keep FUSE on the host
and validate its ABI instead of packaging it privately. FreeBSD's fresh package
passed workspace checks and install/delete/reinstall checks, but a runtime
symlink-alias comparison defect was found during the native regression. Fix
`d601bc5` validates the resolved provider through the trusted host ABI alias and
has a native versioned-provider/alias-update regression. New artifact
qualification is required before that package is accepted.
The replacement `d601bc5` package has now passed the install cycles, 139
workspace tests, four native root audits and an actual ordinary-user desktop
window. Its fourteen physical GELI/VeraCrypt filesystem cases passed. The
coordinator independently matched all original payload hashes, final appended
hashes, invoking-user ownership and closed-mount results in those fourteen
receipts. Remaining container/backing, helper, variation and reboot gates
continue on that installed artifact.

FreeBSD Btrfs/XFS controls using a separately installed in-process LKL FUSE
driver passed with direct I/O. They remain a separate eligibility assessment:
LKL embeds Linux kernel code in a native process. They do not establish native
product support, and new product integration is held pending the user's
clarification about that architecture.

## Work still required

### Latest follow-up evidence

- Linux `67894f6` corrects the non-console sudo loop helper's process group while
  preserving its session. Native GNU-sudo and sudo-rs password/keyfile flows
  passed; Ubuntu's 168 process samples showed no stopped tasks. The targeted
  installed-artifact gates and explicit carry-forward rules are in
  `LINUX_FOLLOWUP.md`. Historical `576505d` rows retain their original identities.
- The newer Linux packaging audit exposed a real policy contradiction: the old
  creator closure bundled FUSE. Producer `7d8f9a4` and consumer `fa063ac` now
  validate it as a host dependency instead. Native SONAME 4 exports and an
  openSUSE SONAME 3 creator rebuild passed the corrected checks. Final package
  inventories and runtime gates continue; earlier package-install success did
  not establish compliance with the host-only FUSE requirement.
- Root independently audited openSUSE's seven `67894f6` ordinary-reboot cases:
  six file/directory backends and one physical VeraCrypt/btrfs volume. The
  original payload, five appended hashes after reopen and reboot, new fsynced
  post-reboot files, successful normal helper closes, changed boot identity and
  unchanged stock kernel all matched. Exact executed recipe hashes match the
  receipts; their payload workers inherit the required effective UID 1300.
  Installed CLI/helper hashes match the RPM inventory before and after reboot.
  The creator resolves host `libfuse3.so.3`; no private FUSE or LKL helper is
  recorded. The scratch disk is detached and the evidence/build disk retained.
  The private audit is `hosts/opensuse/validation/root-final678-seven-reboot-audit.json`.
  This proves those seven cases, not the remaining hosts or foreign-user denial.
- Root also checked Debian's six file/directory reboot cases and physical LUKS2
  sample: all five hashes survived the changed boot, normal closes succeeded,
  and installed integrity, empty loop/process inventories and the preserved
  official 6.12 default matched. The archive SHA-256 and three executed recipe
  identities matched. The private audit is
  `hosts/debian/validation/root-final678-seven-reboot-audit.json`. The later
  reopener has no numeric caller-UID sample; its launch/account chain is being
  reconciled separately from the earlier records' `runtime_uid` fields.
- Fedora's new reboot exposed duplicated ZFS mount aliases after the fixture
  bind was restored with shared propagation. The product refused close rather
  than ignoring the extra root. Failed receipts remain separate; the owner is
  correcting persistent fixture propagation and rerunning affected reboot
  cases. Successful unchanged cases retain their original execution identities.
- A Debian physical UFS2/CryFS failure reproduced in the native driver alone:
  create/fsync/stat succeeded but directory enumeration omitted the new entry.
  Fedora's prerequisite correction `9e27dcc` removes premature EOF at nonzero
  directory offsets. Native old/new controls, unit tests and barrier/rename
  regressions passed on the six hosts. All six owners now report the twelve
  affected UFS2 source-kind and backend rows passed on their new artifacts;
  receipt reconciliation and remaining reboot gates continue. The filesystem
  driver remains external to Lurker.
- DragonFly desktop physical forms are committed at `35af4c7`. Native workspace
  tests, sixteen layout frames, both real UID 1300 helper callbacks and an
  actual native window passed. An initial background test launcher inherited
  ignored signals; its failure and the corrected launcher rerun are separate.
  Root inspected the callback counts, helper identities and 120% bottom frames.
- macOS publisher correction `12a3f93` preserves validated native AppleDouble
  companions with anchored, exclusive, durable publication. Its affected
  regressions passed. New freeze `025f558` passed workspace checks and rebuilt
  the DMG; the extracted package has passed 280 conformance checks and all
  thirty selected native filesystem cells. The corrected app-layout callbacks
  and remaining virtual-partition/helper gates remain separately identified.
  The earlier VeraCrypt callback failure came from placing the test executable
  outside the app bundle, not a demonstrated product discovery defect.
- Root has now audited the `025f558` extracted-package receipts directly: all
  fourteen conformance profiles and their 280 individual outcomes, plus all
  thirty selected filesystem cells with four payload opens each. All expected
  command exits, pinned original hashes, five-file persistence hashes and
  normal final closes matched. The 1,166-file evidence collection matched its
  manifest. Private audits are `hosts/macos/validation/root-package-conformance-audit-025f558.json`
  and `hosts/macos/validation/root-renewed025-thirty-cells-audit.json`. These
  positive-case audits do not validate every blocked/unavailable assessment.
- The macOS frontend/API follow-up is independently checked in
  `hosts/macos/validation/root-renewed025-helper-desktop-partition-audit.json`:
  six actual native Slint callbacks, 39 expected helper JSON responses with
  four payload opens per foreign Apple filesystem, and the CLI/desktop virtual
  VeraCrypt partition gates. Production helper identities, actual callback
  execution, expected refusal outcomes, persistence hashes and normal closes
  matched; the separate test-layout app did not replace the production app.
- macOS's three renewed ZFS directory cases passed their payload lifecycles but
  initially failed cleanup because Spotlight retained mappings. Scoped forced
  detach recovered only those identified fixtures; it is recovery evidence.
  Root independently checked the three fresh no-index/non-browsable reruns:
  original inputs, both final reopens, all 27 CLI operations, matching standalone
  and embedded receipts, normal backing unmount/export and no remaining owned
  mounts, images, sessions or workers. The final audit verifies 240 installed
  files and the app signature. The private receipt is
  `hosts/macos/validation/root-renewed025-zfs-cleanup-audit.json`; other macOS
  cells remain a separate audit. The earlier cleanup failures remain recorded.
- NetBSD's earlier fourteen CGD matrix payloads used UID 0; `SUDO_UID=1300`
  selected ownership, not the writer's credentials. Actual ordinary-user helper
  coverage was narrower. New owner/foreign-user controls exposed real native
  NTFS/exFAT permission bugs, including foreign reads despite private reported
  modes. A same-mode refresh was rejected as insufficient. An isolated native
  userland-library correction now passes owner access and unrelated-user
  list/read/write denial, including the full CryFS payload lifecycle. It changes
  access-mode ABI translation, cached attributes and ReFUSE owner admission;
  no kernel or cache flags are changed. The pinned external prerequisite recipe
  starts at `a90fcfd`; `091bd71` adds parent-operation authorization for the
  ext4 foreign-mkdir failure and versions the ReFUSE contract at 2. The corrected
  libraries are installed separately with backups. Root checked their native
  ELF/SONAME/marker identities, actual mapped providers, three filesystem
  caller/foreign-user controls, full CryFS payload/normal-close/privacy receipts
  and seven unchanged-target stock-provider negatives. The private audit is
  `hosts/netbsd/validation/root-installed-fuse-v2-control-audit.json`.
  Guard `8e09ac8` checks trusted provider and symbol ownership; its native
  workspace passed 131 tests. Fresh installed-product matrix qualification is
  still pending. `ee5fc3f` documents the host prerequisites; no FUSE library
  enters the Lurker package.
- FreeBSD's new real-user gate ultimately passed all 33 fixture access checks
  with UID 1300 write/read and unrelated-user denial through searchable
  plaintext ancestors. PEFS/HAMMER2 failed with a target on ZFS but passed at
  its original same-HAMMER2 target; the matched native PEFS control reproduced
  the cross-filesystem-target limitation. Earlier root PDF payloads remain
  labeled separately from the new caller nonce controls. The subsequent
  ordinary shutdown stalled after reporting the physical VeraCrypt/ZFS pool
  suspended with unrecoverable I/O. Root inspected the retained console, which
  also reports swap exhaustion. Transport/shutdown ordering is an investigation
  hypothesis; memory contribution is unproven. No reboot pass, forced recovery
  or custom shutdown hook is credited.
- FreeBSD's failed shutdown state is now preserved in a root-private native
  RAM save and all four original qcow2 files. Offline copies matched all four
  original hashes before a separate cold recovery boot. Root independently
  checked the live domain UUID and that every disk source points to a verified
  copy, plus the saved memory's owner/mode/size and comparison receipt. The
  private audit is `hosts/freebsd/validation/root-shutdown-recovery-identity-audit.json`.
  Recovery does not establish an ordinary-reboot pass. The first ext4 recovery
  row refused writable mounting; read-only native diagnostics found an unclean
  filesystem and bitmap/count inconsistencies. No repair is credited.
- OpenBSD's stable CryFS source port has passed the full native payload
  lifecycle. Post-exec engine confinement and pathname-replacement negatives
  passed; an earlier pre-exec confinement claim was explicitly retracted after
  a negative test proved that exec clears it. Native port `c8c36e0` subsequently
  passed caller-owned ciphertext/state, foreign-user denial and eight source/
  mountpoint redirection negatives, with privilege confined to native mount
  setup. The final factory rebuild and shared core/CLI/desktop product gates
  remain separate requirements before product enablement. DragonFly CryFS also
  has a root-only native payload control, but
  ordinary-user memory locking and FUSE mounting remain concrete privilege
  barriers; a root diagnostic does not establish product support.
- OpenBSD product startup exposed a native record-publication race: captured
  session records had the expected regular-file type, root owner and 0600 mode,
  but transiently reported two links during atomic replacement. A native
  rename/fstat probe reproduced that state. Shared correction `c3dd43b` retries
  metadata on the same held descriptor for a bounded interval while preserving
  immediate type/owner/mode checks and persistent-hardlink rejection. Its native
  overlay regression passed; Debian's exact-commit workspace check/test passed
  with 171 tests and no failures (`670d11f`). The repaired normal OpenBSD CLI
  and helper each passed the full caller-owned payload/privacy lifecycle and
  configured-close identity negatives; the desktop gate remains pending. NetBSD's
  comparable native FFS(log) probe completed 10,000 renames without observing
  an extra link, so its installed `8e09ac8` candidate proceeds with that scope
  decision and retained probe, rather than rebuilding solely for the source hash.
- OpenBSD's preview desktop suite now passes 43 tests with no failures. Root
  checked the native source-format equivalence and 100%/120% create/manage
  frames; the new test now renders the initial lazy form before changing its
  format. The actual CryFS callback passed as UID 1300 with normal close and
  exact sudo-policy restoration. Coherent commit `123d177` includes the native
  core, CLI, capability model, desktop and documentation. Its exact-commit Linux
  workspace check/test passed 173 tests (`7e931fd`); the native OpenBSD package
  refresh and installed/full-host qualification remain separate gates.
- New NetBSD installed-product containers pass seven CGD and six CryFS backing
  rows as the actual caller. CryFS on exFAT fails with a captured native
  `publish new encrypted block: Function not implemented` diagnostic, matching
  native hardlink ENOSYS. The OpenBSD preview also exposed an exFAT failure and
  lost backing-daemon access; its exact cause/cleanup remains under diagnosis.
  Failed receipts and incomplete vaults are retained; neither is a pass or an
  inapplicable backend. Artifact/log publication now uses host filesystems so
  backing-driver failures cannot erase the product diagnostic.

Host owners are performing current update inventories, builds, applicable
container combinations, native controls and device preparation. Initial product
builds use `8adcdcf` or the recorded later script-only revision; record harness
and independent engine identities separately rather than rebuilding unchanged
product inputs or relabeling old engines as new builds.

NetBSD CGD, FreeBSD GELI/VeraCrypt and OpenBSD CRYPTO/RAID1C/VeraCrypt physical
dispatch are implemented with native proofs. Their final packaged matrices
remain distinct gates. DragonFly physical product dispatch is being qualified.
Native controls inform those adapters; file-container results cannot close the
device requirement. DragonFly's native NTFS/exFAT/ext4 adapters pass applicable
file-container payload flows with the documented host prerequisites. macOS uses
native macFUSE VeraCrypt and native hdiutil foreign-filesystem routes; final
extracted-package validation remains open.

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

Whole-round final validation, cleanup and deferred customized-kernel assessment
remain incomplete. The round remains active. Per-host reports and private
receipts are authoritative for their explicitly recorded revisions and scopes.
