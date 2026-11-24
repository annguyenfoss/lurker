# macOS filesystem round: implementation evidence

Owner: macOS implementation agent. Host: `adjutant@buran`, macOS 27.2.1
(26B131), arm64. Disposable root:
`/tmp/lurker-filesystem-round-macos-20261125`. No physical storage, GUI, SQLite,
fault workloads, reboot, security reduction or new upstream patches were used.

## Source and artifacts

- Adapter commit `3075806`: pass the selected inner filesystem to native Apple
  and VeraCrypt builders; validate it before credentials or staging allocation.
- Shared builders: `67c0903`; Apple FAT32 native spelling correction: `a9a5fb0`.
  Shared API/CLI changes remain coordinator-owned.
- `a9aeaa6` fixes Darwin AppleDouble companions during directory publication.
  Native exFAT automatically moves `._name` with `name`; the iterator can still
  return the vanished companion. Only that verified native metadata case is
  accepted. Arbitrary missing vault data still fails; a targeted guard test was
  added. Exact native create regressions cover gocryptfs and Cryptomator.
- Supplementary bounded test script: `9d1705b`.
- Final exact commit `a9aeaa65237b678e6c1d783fe646fc5b6b6a2ece` passed
  63 core/CLI/native nonignored tests (9 ignored), built CLI/helper, and passed
  both focused native exFAT publication regressions. Its CLI SHA-256 is
  `10651297bf9c862e9aa8b151e90644a09c8f6bc3aed7be8c0d36ccec0658ba6a`;
  helper SHA-256 is
  `06cb1bdce53d99a338be036c97bcfec541946464ba48af72db6d96230eb2a317`.
  `verified-publication.json`, `verified-tests.log` and `cleanup.json` retain
  the final evidence. The earlier container results retain their original
  identities because the final fix changes directory publication only.
- Native candidate `8892f3e2177265cb5624001065eed7511fd94acc` passed
  `cargo test -p lurker-core -p lurker-cli`: 62 passed, 9 ignored, 0 failed;
  CLI and internal helper built. These are development artifacts, not a newly
  qualified release package. The ignored broad native suite was not run.
- Exact final FAT32 candidate `a9a5fb0591148fe249284d5e6df209d4bd29b8f7`
  repeated those checks and the targeted Apple FAT32 payload lifecycle.
- Original CLI SHA-256:
  `df30f7601c8169af061b4aa35f72c1586c394b9918c50c50139ec31388b05366`.
  Final CLI:
  `776a6c101ecfee991229b05f636332720c6541857f9a98c2adff0907584018b3`.
  Original helper:
  `2ee5a2d02d80636d109758c360f66b664cbf5532ba31ea8117b2cabb710f40ca`.
  Private FUSE-T VeraCrypt:
  `8aad857922b8f3a11bd3ec475cfd6f4418767cf9e1c120dddf45582786f88f6e`.
- Local structured evidence:
  `target/goal-validation/filesystem-round/macos/`. Remote native build/test
  logs and receipts remain under the disposable root above. `native-results.json`
  retains each fixture, source commit, binary digest, action log path, payload
  digests and close result. Preview builds are explicitly labeled separately.

## Observed container results

Every RW pass used a fresh encrypted file and real CLI password prompts. The
common harness wrote a valid one-page PDF and UTF-8 text, closed normally,
reopened and verified hashes, appended text and added a second file, then closed,
reopened and verified again. Every pass completed seven CLI actions and left no
plaintext mount. Only the default AES/password profile was exercised.

| Encryption route | Inner filesystem | Result |
|---|---|---|
| Apple AES-256 disk image | APFS | RW persistence pass |
| Apple AES-256 disk image | exFAT | RW persistence pass |
| Native APFS encryption | APFS | RW persistence pass |
| VeraCrypt, private FUSE-T engine | APFS | RW persistence pass |
| VeraCrypt, private FUSE-T engine | exFAT | RW persistence pass |
| Apple AES-256 disk image | FAT32, auxiliary existing selection | RW persistence pass after native spelling correction |
| VeraCrypt, private FUSE-T engine | FAT32, auxiliary existing selection | RW persistence pass |
| Apple AES-256 image import | NTFS | Two CLI RO mounts/read/hash/normal-close passes; writes refused with `EROFS` |

The NTFS fixture was independently formatted by unmodified native ntfsprogs,
populated using `ntfscp`, then copied from its attached virtual device into an
AES-256 Apple image by `hdiutil`. Lurker never claimed NTFS writable creation.
PDF SHA-256: `f62d9bb8cdf4722e68937ce0fd939201585a74e28ea839b3bc1441a2dd29597b`.
The NTFS page digest is
`f1965ce3ab3aeba1effb16c040778f12ed04dfc3b2e15014e8fdd4a8bcf80e33`.

Native APFS encryption is structurally APFS-only. Directory methods have backing
filesystems, not interchangeable inner filesystems. Other native block-encryption
routes do not exist on macOS in the current integrated product.

## Ten-filesystem disposition

“Blocked” identifies an unmet installed/native prerequisite, not a product pass.
RO capability is recorded separately from writable creation. The small payload
proves content persistence only; it does not certify ACL, journal, snapshot,
sparse-file, xattr or other metadata parity.

| Filesystem | Installed or investigated route | Create/format | CLI pairing and RO/RW evidence | Features and limits |
|---|---|---|---|---|
| ext4 | Native `e2fsprogs` 1.47.4; signed/notarized Ext4Kit 0.1.2 installed | Native formatter produced independent raw fixture | Blocked mount: exit 69, `mount: Unable to invoke task`; no CLI RW claim | Ext4Kit upstream documents journaled writes and limited xattrs; inline-data volumes unsupported. Extension registration/enablement requires normal operator setup |
| Btrfs | xlinuxfs upstream FSKit source; paid App Store binary; no GitHub release asset | No compatible installed formatter/mount combination | Blocked paid binary/restricted-signing prerequisite; no CLI pass | Upstream uses LKL Linux Btrfs; source build needs restricted FSKit provisioning profile. No new entitlement or upstream changes attempted |
| XFS | Same xlinuxfs route | No compatible installed formatter/mount combination | Blocked paid binary/restricted-signing prerequisite; no CLI pass | Same restricted FSKit provisioning prerequisite; no kernel/bridge substitution |
| APFS | Built-in `hdiutil`, `diskutil`, `newfs_apfs` | Pass | Apple/APFS, APFS/APFS and VeraCrypt/APFS RW pass; dedicated RO path not exercised | Native APFS features available; only content persistence observed |
| UFS1 / FFSv1 | No native formatter/mounter or supported signed upstream FSKit route found | Unavailable | No CLI RO/RW pass | Current fuse-ufs targets FreeBSD UFSv2, not UFS1 |
| UFS2 / FFSv2 | Unmodified fuse-ufs source `ee5c4a447da98220a11edb69a225d51379afc4fb` | No native formatter | One build plus ordinary pkg-config correction failed: fuser requires `osxfuse >= 2.6.0`; installed FUSE-T advertises 1.2.9 | No false pkg-config version or macFUSE/kernel replacement. Upstream defaults RO; writes experimental and soft updates not supported |
| HAMMER2 | No native or compatible upstream macOS mount/format route found | Unavailable | No CLI RO/RW pass | Porting a BSD filesystem/driver exceeds this round |
| NTFS | Independently built ntfsprogs 2026.9.28; native RO driver; signed/notarized TT NTFS Native 0.4.2 installed | Standalone formatter works; Lurker writable create remains refused | Apple encrypted import RO pass; TT RW blocked: `Module ch.techtag.ntfs.extension is disabled!` | Native RO passed exact PDF/text hashes. TT upstream documents RW, hard links/symlinks, sparse/compressed reads; no compression creation or formatter. Not observed here |
| exFAT | Built-in native formatter/mounter | Pass | Apple/exFAT and VeraCrypt/exFAT RW pass; dedicated RO path not exercised | No native POSIX ownership/permissions or journaling promised; no directory-fsync assumption |
| ZFS / OpenZFS | Matching signed/notarized OpenZFSonOsX 2.4.1p2 macOS 27 arm64 installer downloaded and inspected | Installer not executed | Blocked kernel-extension approval/restart prerequisite; no CLI pass | Installer carries `Library/Extensions/zfs.kext`. Apple-silicon KEXT enablement requires Reduced Security and restart, both excluded; ZFS native encryption was not added |

## Installation and prerequisite receipts

Initial preflight found 489 GiB available, system APFS mounts only, and existing
FUSE-T core/FSKit 1.2.9 receipts. Existing Rust, Python, private backend and Cargo
caches were reused. Installed packages were left installed as authorized.

- `HOMEBREW_NO_AUTO_UPDATE=1 brew install e2fsprogs`
- Build prerequisites: `brew install autoconf automake libtool pkgconf libgcrypt`.
  Observed versions: autoconf 2.73, automake 1.20, libtool 2.6.2, pkgconf 3.0.11,
  libgcrypt 1.12.4 and dependency libgpg-error 1.61. A Homebrew inspection command
  automatically enabled developer mode; it was immediately restored with
  `brew developer off`.
- Ext4Kit and TT NTFS were downloaded from their upstream signed releases,
  checked with strict `codesign` and `spctl`, then copied with system `ditto` into
  `/Applications`. `lsregister`, `pluginkit -a` and ordinary `pluginkit -e use`
  were tried; plug-in preference does not prove FSKit authorization. TT shows
  `+` in pluginkit but the actual native mount says the module is disabled.
- Ext4Kit ZIP SHA-256:
  `be53836a94f425e108f57963ce3971821a8a75c0daae00568e75708d91fa05de`.
  TT NTFS DMG:
  `e4c1af51a1a1f38250fc9e226cf571be771655ef99b5deabb40dd02e230fa8c7`.
- ntfsprogs source archive SHA-256:
  `8a75293f07bf40df5701d296de32a4cb0e46d6ae135abb4f41b438df9868a390`.
  `autoreconf -fi`, configure with `--disable-ntfs-3g --disable-mount-helper
  --disable-crypto --without-uuid`, and `make -j4` built the native formatter.
  `make install` failed in an upstream Linux `.so` move hook; the built tools
  ran unprivileged directly for the fixture. No source patch/install-hook bypass.
  `mkntfs` SHA-256:
  `21e17abbc4810582488e6c5d1697976b14ae3d681b6b7bbdc9cda9aa88822311`.
- OpenZFS installer SHA-256:
  `4123057fe0a4ff6fe56d4d500255cae1fc94057c9b08389069cbf411b7b27c43`.
  `pkgutil --check-signature` reported a trusted notarized Developer ID installer.
  The package was expanded for prerequisite inspection only.

The coordinator requested one bounded App Store check. Standard Homebrew `mas` 7.0.0
was installed; `mas info 6785355678` reported xlinuxfs 1.0.4 at **99.000đ** in the
Vietnam storefront. Paid procurement is excluded, so no purchase or installation
was attempted. Current `mas` has no `account` subcommand; no new login was tried.
The exact listing and install log are retained under `install/` in the remote root.

Normal next operator step for signed FSKit modules on macOS 27 is System
Settings → General → Login Items & Extensions → By Category → File System
Extensions, enabling the relevant installed app. Ext4Kit also documents opening
its installed host app once to register the module. No GUI or private entitlement
workaround was used. Current `fskitd` logs state `Attempt to start disabled
extension` for the tested FSKit attempts; retained in `fskit-disabled.log`.

Upstream references:

- https://github.com/rayhanadev/Ext4Kit/releases/tag/v0.1.2
- https://github.com/rayhanadev/Ext4Kit
- https://github.com/HuanchuanTech/xlinuxfs/blob/main/docs/build.md
- https://github.com/HuanchuanTech/xlinuxfs/blob/main/docs/filesystem-capabilities.md
- https://github.com/realchonk/fuse-ufs
- https://github.com/dr-kbadawi/ttntfs/releases/tag/v0.4.2
- https://github.com/dr-kbadawi/ttntfs
- https://github.com/tuxera/ntfs-3g/releases/tag/2026.9.28
- https://github.com/openzfsonosx/openzfs-fork/releases/tag/zfs-macOS-2.4.1p2
- https://support.apple.com/guide/security/kernel-extensions-sec8e454101b/web

## Directory routes, cleanup and review handoff

Current FUSE-T gocryptfs and CryFS were rebuilt with the unchanged, checked-in
project integrations, reusing native source/Conan caches. No new upstream patches
were introduced. gocryptfs v2.7.0 source
`756cd453d7afe69fd2a00a7592fd2d9869a6f756` built to SHA-256
`854b4949b25fbdcc679d5aa7f9fca3e620d1a916b927017862e8dd6f551c3f85`;
CryFS 1.0.3 source `5d0cc36785d93e12250696a3ffc928042f2e8d61` built to
`2aea71f23f1a2e7bd5804969d85a25032484055466ad425936c60935d499d765`.
Their manifests are retained in `folder-current/` and summarized locally in
`directory-backend-identities.json`. Cryptomator used its existing private
FSKit runtime. Directory results remain separate from container inner filesystems.

| Directory method | APFS backing image | exFAT backing image |
|---|---|---|
| gocryptfs | Initialization passes; mount blocked by disabled FSKit | Initialization passes after AppleDouble fix; mount blocked by disabled FSKit |
| CryFS | Initialization mount blocked by disabled FSKit | Existing bundled-engine encrypted-block publication requires hard links; exFAT returns Operation not supported. No backend patch attempted |
| Cryptomator | Initialization passes; mount blocked by disabled FSKit | Initialization passes after AppleDouble fix; mount blocked by disabled FSKit |

All six combinations were attempted with current FUSE-T engines and retained
individual receipts. None is counted as an RW persistence pass. The three
APFS-backed and two exFAT-backed FSKit mounts require normal operator approval.
The CryFS exFAT failure is specifically the existing Lurker integration's
hard-link publication requirement, not a claim that upstream CryFS inherently
cannot use exFAT. Reworking that backend patch is excluded from this round.

The earlier raw-cache and 20261122 DMG directory attempts used old macFUSE-linked
gocryptfs/CryFS artifacts, so their failures are explicitly not current-engine
support results. All failed source fences were reconciled through ordinary
scoped CLI unmount; failed-session records and fixture images were retained.

A plain raw APFS import for FreeBSD/OpenBSD was also produced natively and
cleanly detached:
`apfs-import/apfs-with-pdf.cdr`, SHA-256
`50037c6612d6a5e5d1b46f48c2d44ff19e249cbed2958db3470f6c632ef8c9e2`.
Its `paper.pdf` matches the common payload above; its `page.txt` hash is
`9cd617210a58d52a62ceee8402a2f15d3f59ec53408a15f965e60ed8e852fef0`.

Implementation is terminal; Buran is released for the fresh reviewer. Final
inventory confirms zero owned attached images, mounts, active folder sessions
or processes. All jobs are terminal. Scope-owned failed-session evidence and
fixture images remain on disk; packages and caches remain installed. System
free space is 480 GiB. No unrelated user session was touched.

Reviewer next action: read local `result.json` and this note first, inspect the
narrow Darwin AppleDouble guard and its native reproduction, verify the five
requested RW container pair receipts plus NTFS RO import, and retain the explicit
FSKit, paid-source, UFS/HAMMER2 and KEXT limitations. Broad blocked-case reruns
need an operator prerequisite change; no GUI/security/signing bypass is allowed.

## Independent review, 2026-11-25

Fresh review completed after implementation released Buran. No serious Lurker
or harness defect was found; no product patch or unnecessary matrix rerun was
needed. Reviewed candidate remains `a9aeaa6`, with the original per-fixture
source identities preserved. The remote `source-verified` tree matches SHA-256
for all 58 tracked core/CLI/helper source and manifest files at that candidate.
Committed macOS adapter, folder publication, API/CLI, workflow builder and Cargo
inputs remain unchanged through review head `9b1bc0d`; later FreeBSD
code and concurrent Linux/harness work do not replace these native identities.

The reviewer independently checked the remote receipts and action-log presence
for five requested RW pairs plus two FAT32 regressions, excluding the earlier
preview and failed Apple FAT32 run. Each accepted receipt contains seven
successful ordinary CLI actions, original/updated PDF and text hashes, the added
file, final hash agreement and no remaining mount. The encrypted NTFS import
contains two successful RO read/normal-close cycles with matching hashes and
`EROFS` write refusals. All six current-engine directory/backing attempts are
accounted for separately; none is a payload persistence pass. Retained backend
diagnostics corroborate disabled FSKit startup and CryFS's existing exFAT
encrypted-block publication limit. Signed FSKit operator enablement, paid
xlinuxfs procurement, UFS/HAMMER2 availability and excluded ZFS KEXT setup
remain exactly the limitations above. No operator prerequisite was bypassed.

The Darwin guard is scoped to ENOENT for a missing `._` companion whose published
regular file has AppleDouble magic and a published regular-file/directory base;
other rename failures still abort before configuration publication. Native
stream-rename reproduction, the focused guard test and both exact-candidate
gocryptfs/Cryptomator exFAT create regressions support the fix. The actual native
check log totals 63 passed, 9 ignored, 0 failed; these include ordinary unit/CLI
checks and one nonignored native validation test, not 63 storage lifecycles.
The broad ignored native suite remains unexercised. CLI/helper hashes still
match the final candidate recorded above.

Fresh SSH inventory found zero owned images, mounts, backend/build processes or
active folder sessions. Failed-session evidence and fixtures remain retained.
No unrelated sessions were changed. Review evidence is
`target/goal-validation/filesystem-round/macos/review.json` and
`review-remote.json`; the latter also remains at
`/tmp/lurker-filesystem-round-macos-20261125/review-remote.json` on Buran.
Buran is released. Operator enablement would be required before the blocked
FSKit cases can acquire lifecycle evidence.
