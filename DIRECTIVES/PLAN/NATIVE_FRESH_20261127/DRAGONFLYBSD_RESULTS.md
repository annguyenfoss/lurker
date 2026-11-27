# DragonFly BSD fresh ordinary-route qualification

Host: `adjutant@dragonflybsd-core-01`. Final frozen source:
`8ca1a4e26d5f3701c379bb256382a72b388e2078`. Fresh native LUKS1 and VeraCrypt
qualification passed for file containers and the new verified scratch partition
on UFS1, HAMMER2, ext4, NTFS and exFAT. Each dimension has ten filesystem/backend
cells and eight representative additional cipher/credential cases: 36 final
lifecycles in total. Four conformance suites, the installed package, actual
Slint/helper callbacks and six mounted-fixture ordinary reboot checks passed.

The ordinary-route assessment is complete. Five CryFS product cells in each
130-cell ledger remain blocked by a diagnosed native privilege/confinement
integration gap. Other native route exclusions remain explicit; no root-only
control, reader library, previous report or other host pass receives product credit.
Custom kernel work remains deferred.

## Updated host and exact artifacts

The unmodified stock DragonFly source
`e17b9176356e3afb3427d71c21108a8a849cd4d5` completed `buildworld`,
`buildkernel`, `installkernel`, `installworld` and `upgrade`, all exit 0.
The running X86_64_GENERIC kernel is `e17b9176-DEVELOPMENT` on
6.5-DEVELOPMENT. Its SHA-256 is
`dbc7827922fed75bf63ef0678fd5884f007c178ecc610d5a1ffe467d92ba3d62`.
The final ordinary reboot changed `kern.boottime` from 1795733143 to
1795744636. Verified boot/etc rollback, world backup and the original kernel
are retained. No custom kernel patch was applied.

`pkg update -f` and `pkg upgrade -y` completed: nine upgrades and 129 ABI-matched
reinstalls. Rust 1.98.1, Python 3.12.16 and Python 3.14.10 are functional.
Official native build, X11 and SQLite prerequisites were installed as needed;
package vulnerability notices are retained in the upgrade log.

Final native `cargo check --workspace --locked`,
`cargo test --workspace --locked -- --test-threads=1` and
`cargo build --workspace --locked` all passed. The workspace suite executed
127 passing tests with five explicit opt-in tests ignored. The native physical
identity guard and both actual GUI callbacks were then run explicitly and passed.
Builds used one Cargo worker, no incremental data and zero development/test debug
information. All builds and tests ran on DragonFly, not the coordinator.

| Final artifact | SHA-256 |
| --- | --- |
| Source archive | `0d64c60d5e57ccd84f6771b343d70571987dc58df31c4f32cdb4d6cec1870a9b` |
| Source manifest, 422 regular tracked files | `f0a4fd1d900189e3a4e3aa3056f24446d46d8ce368f9064262623333c4fda443` |
| CLI | `9d6fb4e4a24ab192ec7dfcb8a3720035969517276c312354f4b0f622d6e3e93b` |
| Internal helper | `b8900e3e975f06eb30aa6bd49029174ba77d3fe514db83cc5f1ecf03c5d89b8e` |
| Desktop | `ec4e0f1a566f2120db137e0227f22432c7b7cff3f821f0434ef5ee9416b9171d` |
| Canonical package archive | `74d9cf04e30897a263406d3bcec465694f878b3ac84fc6dcaa89f196e69890db` |

The native canonical stager selected LUKS1 and VeraCrypt, collected dependency
notices and pinned private tcplay, and verified 847 manifested files into
`/opt/lurker-native-fresh-20261127/candidate`. Final cleanup checked every file
and directory in that installed prefix remains root-owned and not group/other
writable, and rechecked all three binary hashes after reboot. Host FUSE and
filesystem tools are prerequisites outside the private package.

## Final container and physical results

Every final case used newly created storage and explicitly selected its filesystem.
Native mounts and independent filesystem/superblock identity were verified before
payload I/O. The downloaded Bitcoin, Ethereum and Transformer PDFs plus text
survived write, normal close/reopen/hash, append/new file, and a second normal
close/reopen/hash. Both writers and hash readers explicitly dropped real and
effective UID/GID to 1300 and recorded their actual syscall identities.

Each mount also denied unrelated UID 65534 directory listing, payload read,
existing-file write and nonexclusive create. That process successfully opened
both `/` and `/tmp`, proving denial was not merely an inaccessible fixture
ancestor. NTFS/exFAT use native synthetic ownership; the receipts distinguish
this from stored POSIX metadata. All five native filesystem routes passed.
Historical direct matrices had caller writes but root hash readers, and early
ownership-only observations do not prove caller I/O. Their separate audit is
retained; final receipts supply the stronger proof.

Per dimension, all five filesystems passed with LUKS1 and VeraCrypt. Additional
samples cover each backend's Serpent/password on UFS1, Serpent/keyfile on HAMMER2,
Twofish/password on ext4 and Twofish/keyfile on UFS1. The baseline cells cover AES
and both credentials. Explicit and Auto opens are represented. No full
filesystem/cipher/credential Cartesian product is claimed.

Four conformance suites passed: LUKS1/password/Serpent/UFS1,
LUKS1/binary-keyfile/Twofish/UFS1, VeraCrypt/password/AES/HAMMER2 and
VeraCrypt/keyfile/Twofish/HAMMER2. They cover wrong credentials, no overwrite,
duplicate/hardlink aliases, busy close, SQLite WAL/application I/O, persistence
and VeraCrypt external native-dismount recovery. Their functional workloads run
as root; the separate final matrices prove caller access and denial. Four extra
raw-device wrong-password/wrong-keyfile probes passed with unchanged sampled
ciphertext edges, unchanged mapper inventories and no resulting mounts.

Quick feature receipts record private ownership/permissions and native mount
identity. HAMMER2 is identified as the native CoW filesystem; ext4's journal
feature is recorded from its superblock. ACL lifecycle, deep CoW behavior and
journal replay remain unknown where unexercised; normal fsync/reopen is not a
crash-recovery claim.

The sole destructive device target is the new 1 GiB qcow2:
`/srv/build/lurker-native-fresh-20261127/dragonflybsd/scratch.qcow2`, attached to
this domain as `vda`, requested serial `lurker-fresh-dragonflybsd`, PCI `0:5.0`.
Guest PCI/driver evidence linked it to `virtio_pci1`, `vtblk0` and `/dev/vbd0`.
The guest driver does not expose the requested serial; this limitation is explicit.
Blank partition inventory, zero edges and geometry were recorded before first
write. The original system disk was never a crypto test target.

The GPT partition `/dev/vbd0s1` has offset 1048576 and size 1071644672 bytes,
512-byte sectors, storage UUID bytes `529f6ca8ebe9f1119752010000000000`.
The product checks hardware-node provenance, independent GPT rows, DIOCGPART
geometry/UUID, descriptor/path identity and conservative whole-disk mount,
swap, key-free mapper and full numeric open-descriptor inventories. A held-device
FD positive control is refused. The exact final guard passed again after reboot,
with unchanged sampled ciphertext edges. The encrypted partition remains attached
and closed for review.

## Frontend, reboot and cleanup

All 16 DragonFly physical form snapshots passed at native scales and are byte
identical to the visually reviewed preview set. Both opt-in actual desktop/helper
callbacks ran as UID 1300 on the verified raw partition, one executed pass and zero
ignored tests each. LUKS1 and VeraCrypt passed create, mount, write, close, reopen
and compare through the actual helper, never the CLI. A real UID-1300 winit X11
software-rendered window also passed at 1280 × 860 with no loader overrides;
all owned window/server children were reaped.

Six owned, synced encrypted mounts were deliberately left mounted for the ordinary
reboot: file-backed UFS1, ext4, exFAT, HAMMER2 and NTFS, plus physical
VeraCrypt/Twofish/keyfile/UFS1. After the new stock-kernel boot, each reopened
through Auto detection, verified all downloaded payload hashes as real/effective
UID 1300, denied the unrelated user, appended text and created/fsynced a new file,
then normally closed/reopened/verified and closed. All six passed. This samples
reboot persistence across the native filesystem routes; it is not a full reboot
Cartesian matrix or power-loss test.

Final inventory has only the original base mounts, no encrypted mappings, all
four vnode units idle, and no owned backend/compiler/window processes. The native
session directory contains only its empty lock. The temporary helper sudoers file
was removed and `visudo -c` passed. Attempted user-FUSE configuration was restored:
original `/etc/sysctl.conf` byte hash, original absence of `/etc/devfs.conf`,
`vfs.usermount=0`, and `/dev/fuse` root:operator mode 0660. Failed fixtures,
rollback artifacts and source/build evidence are retained; unrelated jobs and
storage were preserved.

## Ordinary host prerequisite fixes and product changes

Owned product commits `cf6841a` and `1f627f9` add native NTFS/exFAT/ext4 formatting,
private FUSE identity and ext4 geometry/ownership. `cb5d22b` adds read-only physical
identity/idle guards; `6a4b88c` adds verified raw-device LUKS1/VeraCrypt creation,
open, geometry and checked flush. Parent commits `34872dd` and `35af4c7` add
native API and desktop admission. Older wrappers are refused before physical
random initialization, with unchanged sampled ciphertext; legacy vnode protocol
2 is preserved and physical support requires extension protocol 1. No cipher,
KDF, PIM or fixed default was weakened.

- `fusefs-ntfs-2026.2.25_1`: native raw-character advisory-lock compatibility,
  while regular-image held-lock rejection remains effective. Eligible upstream:
  https://download.tuxera.com/opensource/ntfs-3g_ntfsprogs-2026.2.25.tgz
- `fusefs-ext2-0.0.11_3`: existing aligned libext2fs direct I/O for raw character
  devices fixes native two-byte superblock-write EINVAL; real fsync is preserved.
  https://github.com/alperakcan/fuse-ext2/commit/ae35afb9ab08d87c66c1e021df792b3a7c4308b0
- `fusefs-libs-2.9.9_3`: four background-child `exit` calls become `_exit` and
  sysctl length uses native `size_t`. Native fork/syscall controls proved inherited
  C++ destructor SIGABRT. This installed host prerequisite is separate from the
  Lurker archive. Package SHA-256:
  `c532dc2a1da9e347c924a5a89de8a59e9540bcf12a91652666e53be4bce4a5d8`;
  installed library:
  `3e792a8ccbc99e7722b79a98fd9076b8745cccb4d544ca496ba8d0a562a84eb5`.
  https://github.com/libfuse/libfuse/releases/tag/fuse-2.9.9
  https://github.com/DragonFlyBSD/DPorts/tree/40ee064a7f3a03b897130a8193a16feb67684934/filesystems/fusefs-libs

Original host packages are retained as verified rollback archives. Six immediate
block-product regressions passed after the library fix, followed by the complete
final 36-case batch. A discarded early experimental FUSE source-vnode mismatch
caused a kernel panic; the corrected `/dev/fuse` identity contract, stock-helper
negative control and later clean lifecycle/reboot results are retained separately.

Harness setup failures are preserved without pass credit: inherited ignored
signals in an early launcher, missing opt-in execution, missing SQLite module or
auxiliary script, minimum VeraCrypt size and tmpfs guard refusals. An unrelated-user
O_EXCL probe encountered native EEXIST under an unsearchable UFS1 directory;
a separate control proved no file creation. Final denial probes use existing-file
write and nonexclusive create. Failed conformance fixtures were normally closed,
then copied and byte-verified into persistent evidence before volatile copies
were reclaimed. Only identity-matched aliases of successful closed fixtures were
removed.

## Remaining native routes and 130-cell reconciliation

Both `applicability.json` and `device/applicability.json` now contain 130 final
assessed cells: **10 passed, 45 unavailable through assessed ordinary routes,
70 not applicable and five blocked CryFS product cells**. No final-validation
field remains pending. The initial ledgers are preserved separately. Directory
rows mean backing storage, not a selectable inner filesystem.

Btrfs has no qualified native writable driver; XFS/libfsxfs and APFS/libfsapfs
provide readers, not writable creation routes; UFS2 lacks a qualified native
formatter/driver generation; ZFS lacks a qualified matching native module and
formatter. Base cryptsetup 1.1.2 provides LUKS1 only; no ordinary native LUKS2
read/write engine was qualified. Owning-OS facilities are explicitly not applicable.

Stable LGPLv3 CryFS 1.0.3 pin
`5d0cc36785d93e12250696a3ffc928042f2e8d61` completed all 367 native build steps.
Commit `8ca1a4e` only enables its existing sysconf memory implementation for
DragonFly, preserving AES-256-GCM, scrypt, 16384-byte blocks and the common
durability patch. Engine SHA-256:
`94ec8564d2fc9157f61983c9afff48d7b08ffba735e9885296242d4db715bf60`.

With the corrected ordinary installed libfuse and no loader overrides, a protected
root-only HAMMER2 control passed all PDFs/text, append/new file and both normal
reopens; its exact mount/source/fsid and precise PID/start identity were recorded,
all exits were zero and all processes/mounts closed. Actual UID-1300 startup
independently fails `mlock` with EPERM even with unlimited lock limits; a separate
inherited-FD FUSE mount also fails EPERM despite usermount configuration. Stock
native APIs require root capabilities; chroot/jail disables the capability needed
by mlock. No unsafe root-engine exception, memory-lock bypass or product admission
was added. A safe caller-I/O/confinement integration remains the concrete gap.

A separate native primitive-buffer process probe passed live, unreaped zombie
and reaped-child states with matching C/Rust offsets. Native `kinfo_proc` is
1048 bytes, versus libc 0.2.191's older 992-byte tail. Only verified primitive
PID/time/status fields are read; no whole enum-bearing Rust structure is
initialized from kernel bytes. This audited draft remains unintegrated pending
the engine privilege design.

Pinned gocryptfs 2.7.0 `756cd453d7afe69fd2a00a7592fd2d9869a6f756` with go-fuse
`2d50fff207d78c197869fa492eff3a79bc1d8408` was actually compiled using native
Go 1.26.11 DragonFly/amd64, one worker, CGO disabled and `without_openssl`.
Unmodified production sources fail missing DragonFly protocol/dirent constants,
stat time fields and native syscall providers. The bounded port assessment found
12 FreeBSD Go-FUSE production files/392 lines and five gocryptfs files/261 lines,
plus generic guards; this is plausible port scope, not an impossibility claim.
Native ABI, no-follow/xattr/rename/allocation semantics and a trusted mount broker
with unprivileged engine and precise leases need qualification. No broad port,
engine rewrite or custom kernel work was started. Cryptomator jFUSE 0.7.3 has
Linux/Darwin/Windows generated providers; a DragonFly native FUSE/JVM provider
remains unqualified. Missing package names alone are not exclusion proof.

## Evidence

Private local evidence:
`target/goal-validation/native-fresh-20261127/hosts/dragonflybsd/`.
Persistent native evidence:
`/home/adjutant/.cache/lurker/native-fresh-20261127-dragonflybsd/evidence`, exposed
through `/tmp/lurker-native-fresh-20261127-dragonflybsd`.

Final entry points are `build/final-build-8ca1a4e-state.json`,
`validation/final-package-install.json`, `cases/final-8ca1a4e-state.json`,
`device/final-8ca1a4e-state.json`, `validation/final-conformance-state.json`,
`validation/final-desktop-user.json`, `validation/final-window/result.json`,
`validation/ordinary-reboot-result.json`, `validation/final-cleanup.json` and the
two applicability ledgers. Each final lifecycle has its own source/artifact/tool
identities, commands, actual UID probes, native mount/filesystem evidence and
payload hashes. Initial source `8adcdcf` and development receipts remain
historical evidence rather than fresh final passes.
