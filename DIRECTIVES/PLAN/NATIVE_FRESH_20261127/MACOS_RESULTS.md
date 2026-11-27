# macOS fresh native qualification checkpoint

Status: scoped native qualification complete for frozen candidate `025f558`.
Thirty eligible native filesystem/backend cells passed; the other 100 planned
cells have explicit blocked, unavailable or not-applicable reasons. This is not
blanket filesystem or release qualification. Buran was not rebooted. Three
representative physical USB samples retain their original staged identities;
the renewed package passed 280 conformance checks and final installed validation. Private evidence is under
`target/goal-validation/native-fresh-20261127/hosts/macos/`; the corresponding
remote root is `/tmp/lurker-native-fresh-20261127-macos`.

## Host and updates

- macOS 27.2.1, build 26B131; Darwin 27.2.0 arm64. Initial free space 469 GiB.
- `softwareupdate --list`: no updates available. Scoped Homebrew refresh and
  upgrades completed for uv, zstd and glib; unrelated packages were preserved.
- FUSE-T 1.2.9 is current. Its directory/ext4/XFS FSKit mounts are local and
  eligible. The existing VeraCrypt SMB loopback and Btrfs VM/SSH routes are
  excluded from native acceptance.
- The already installed macFUSE 5.5.0 kernel extension loaded successfully via
  `kmutil` without restart. OpenZFS userspace and loaded kmod report
  `2.4.1-p2`, matching the current GoldenGate 27 arm64 signed package. The
  bundle/pkg receipt's abbreviated `2.4.1` is not evidence of an older driver.
- The FUSE audit found `/usr/local/lib` owned by the login user. Its owner alone
  was corrected to root:wheel; directory mode and contents were preserved.
- Toshiba TransMemory identity: external USB, 124923150336 bytes, serial
  `0060E0526069E260600005BB`, vendor 2352/product 5130. Initial disk4 GPT contains
  EFI and an APFS partition/container with `Untitled`. After repeat identity
  verification, the authorized device was partitioned into about 2 GB encrypted
  APFS and 2 GB exFAT samples, leaving 120.6 GB unallocated. Both samples closed
  normally; APFS is locked and exFAT is unmounted.

## Builds and product changes

Baseline `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d` workspace run
`20261126T203536Z-4c3d602b`: check passed; 122 tests passed, none failed,
11 ignored. CLI SHA-256:
`85d30dfaa47e2a734a1301f3cc8bf08a8d13a1c20339597ca06f5622fafedc34`.

- A later managed-toolchain refresh upgraded stable Rust from 1.99.1 to
  1.100.0. Workspace run `20261126T211340Z-01ca4a1c` checked frozen candidate
  `836bf84`: 124 tests passed, none failed, 11 ignored. Its package stage was
  deliberately cancelled to incorporate the later fixes; it is not a package
  pass. The final package will use a new frozen revision and one Cargo worker.
- `d78799c`: explicit audited macFUSE VeraCrypt build option, preserving the
  FUSE-T default and fixed directory FSKit policy. Eighteen Mac build/package
  tests passed remotely. Fresh pinned VeraCrypt 1.26.31 macFUSE build and all
  algorithm self-tests passed.
- `ced26af`: discover separately installed host filesystem tools and select
  local FSKit for ext4/XFS. Core check, 60 unit tests and one native plaintext
  test passed; nine optional native cases ignored.
- `9a5ca4d`: wait for a filesystem driver's normal exit after unmount, preserving
  encryption if it remains draining. Core checks/tests passed. This alone does
  not resolve the XFS persistence failure described below.
- Coordinator `3178144`: CryFS initial plaintext mount moved to the private
  runtime directory. Matching overlay core check and APFS payload regression
  passed before commit; folder source SHA-256
  `3975fd0e8829b52aaa29efa68a38a09eeebe4bf381aff2fb0d937c0095f60da9`.

All filesystem tools remain host prerequisites. The round installed inspected
existing formatter/driver builds into administrator-owned
`/usr/local/libexec/lurker-filesystems`; no such tool is elevated or bundled
with the product.

## Initial runtime results

The ledger `matrix-initial.json` explicitly assesses all 130 filesystem/backend
cells and distinguishes inner filesystems from directory backing filesystems.
Receipts contain the downloaded Bitcoin, Ethereum, Transformer and text hashes,
normal closes/reopens, append and second-file results. Final validation is pending.

- Apple images: APFS and exFAT product payload cases passed. Native hdiutil
  controls also passed APFS, exFAT, TT NTFS FSKit and OpenZFS inner filesystems;
  the latter two remain Apple-product adapter gaps.
- Native APFS encrypted image: product payload case passed.
- VeraCrypt macFUSE: initial APFS, exFAT, NTFS and ZFS product cases passed;
  after host-tool discovery, ext4 FSKit and UFS2 macFUSE cases passed.
- gocryptfs and Cryptomator payload cases passed on APFS, exFAT, ext4 and ZFS.
  gocryptfs also passed on regular-file-backed UFS2. XFS directory cases passed
  while the backing filesystem stayed mounted, but are not complete native
  persistence passes because the independent XFS close/reopen control failed.
- CryFS passed on ZFS and, with the runtime initialization fix, APFS. Its patched
  hardlink publication fails on exFAT/ext4 and UFS2. Native no-clobber controls
  show Darwin `renamex_np(RENAME_EXCL)` works on UFS2 with file/directory fsync,
  but returns ENOTSUP for a new destination on exFAT/ext4 FSKit. Existing
  destinations remain intact in all three controls. No unsafe fallback was used.
- TT NTFS creates directories as 0755 despite mkdir/chmod 0700, including an
  `owners` mount control. All three directory backends correctly reject that
  private-state limitation. UFS2 Cryptomator publication returns ENOTSUP.
- Current fuse-xfs persistence is inconsistent: encrypted hdiutil block-device
  control passed, whereas regular-file and VeraCrypt reopen cases lost a file.
  A retained native-device integration experiment did not fix this and was not
  committed. A separate host-driver fsync experiment also failed; it was not
  installed or counted as a repair. Commit `8a401a3` now refuses XFS create
  before prompts, tools or allocation, and refuses an authenticated XFS open
  before filesystem writes while cleaning up encryption normally. Native guard
  regression retained unchanged ciphertext and an empty unmounted target. XFS
  remains unavailable pending a trustworthy persistence-capable host driver.
- Btrfs had no qualified native runtime route in the initial sweep; its existing
  VM compatibility branch ran once and is explicitly excluded. The later native
  LKL source assessment below identifies an eligible candidate, not a runtime pass.
  UFS1/HAMMER2 native create/read-write routes remain unavailable. GELI/PEFS,
  CGD and OpenBSD softraid formats have no corresponding macOS native facility.

LUKS candidates were reassessed. `userspaceLUKS` explicitly warns that writes
can destroy key material; it is ineligible. The native `open_ext4_for_mac` FSKit
candidate offers LUKS import, not this round's complete create workflow.
LUKSbox uses its own `.lbx` format and cannot substitute for LUKS1/LUKS2.

The pinned public xlinuxfs source
(`f9da28e8cfdf6a4574a31e8baa9ba1811ef4c2e4`, GPL-2.0) includes a companion
Darwin LKL port (`c4d0895ead9badb52f7bbffe3d529c61c8fc9485`), a self-contained
Mach-O arm64/x86_64 build profile, local file I/O and bridge tests for read/write
ext4, XFS and Btrfs. This is a credible eligible native candidate; source
availability and architecture are not established blockers. Buran lacks the
restricted provisioning profile needed to load its FSKit extension. Native
library/bridge build proof and a separately installed mounted driver remain
distinct qualification gates; neither has a Buran acceptance pass at this
checkpoint. UFS1 is disabled in the fixed profile and needs additional source
integration and validation. The separate btrfs4mac project supports reading only.

Six VeraCrypt Auto conformance profiles (AES/Serpent/Twofish, password and binary
keyfile) each passed 20 checks, totaling 120, including wrong credentials,
duplicates, busy close and downloaded-payload repeated reopening.

The external APFS-encrypted partition passed four downloaded-payload mount
phases, including Buran's extra reopen. A Cryptomator directory on the separate
external exFAT partition also passed. Receipts are `device/apfs-receipt.json` and
`device/cryptomator-receipt.json`; current identity and before/after/final
partition inventories accompany them. No internal disk was modified.

Quick feature controls recorded native permissions/ownership and ACL outcomes
for all seven available filesystems. APFS and ZFS retained ACLs; other selected
drivers refused ACL additions. NTFS ignored private modes as above. Direct
`clonefile(2)` succeeded on APFS/ZFS and returned ENOTSUP on exFAT/NTFS/ext4/XFS;
a native ZFS snapshot retained old bytes after a live overwrite. The original
`cp -c` control silently fell back on unsupported filesystems and is not used as
proof of CoW. Journaling/replay observations stay separate from format flags.

## Later native repairs and controls

- `5e2c01c` repairs host fuse-ufs directory enumeration after the first page and
  requires both advertised UFS2 write and directory protocol capabilities.
  A fresh root-owned host build passed direct CryFS UFS2 write/append/verify/verify.
  The regular-file UFS2 route passes; block-device fsync returns ENOTTY/EIO and
  is not qualified.
- Coordinator `175ea1c` adds a Darwin-only CryFS exclusive regular-file publication
  patch after the unchanged common durability patch. APFS product regression
  and direct UFS2 full payload proof passed. Patch SHA-256:
  `510d4d943b7dc30ef8b025da7b0e14ade901a9bf80d4e1a79d1e7ab6d19dc178`;
  engine SHA-256:
  `70be02d79bf048afc38288b3bd7d99ab4934e4b5d6052e2f34dc13923762d94c`.
  The later higher-level fix below separately qualifies product publication;
  these direct engine receipts remain independent proof for the C++ patch.
- `a540fcc` documents a macFUSE-only fuse-ext2 xattr callback shim. The separately
  installed root-owned ext4 MacFUSE driver supports all three directory engines,
  including CryFS, unlike the preferred FSKit route's atomic publication limits.
- Installed NTFS3G with native macFUSE and explicit private uid/gid/umask supports
  all three directory engines. This distinct driver route resolves the TT NTFS
  mode limitation without claiming that TT NTFS retains private modes.
- Fresh final plain-native controls on both installed ext4 MacFUSE and NTFS3G
  completed four payload phases across normal backing filesystem closes/reopens.
  Receipts: `cases/native-ext4-macfuse-final/receipt.json` and
  `cases/native-ntfs-macfuse-final/receipt.json`.
- `6d27510` binds the package conformance harness to the exact extracted binary,
  commit, child harness hashes, actual lifecycle receipts and downloaded payload.
  Its remote unit test and real 20-case receipt validation passed.

## Native product integration follow-through

- `383c47b` adds an anchored, exclusive Mac directory publication fallback only
  when the ordinary native cross-parent directory rename reports unsupported.
  It refuses existing destinations, symlinks and special nodes; preserves
  directory attributes through open descriptors; syncs the destination and
  source parents; and leaves configuration publication last. Partial failures
  remain inspectable. Four native unit tests passed, including collision,
  partial-error and attribute controls. Native product CryFS/Cryptomator UFS2
  payload cases now pass, alongside APFS regressions and exFAT Cryptomator.
  CryFS on exFAT still lacks the required atomic engine publication primitive.
- `7984df0` implements native Apple encrypted-image ext4/NTFS/ZFS creation and
  mounting. It uses the fixed AES-256 profile centralized in `workflow.rs`,
  separately installed host tools and a crypto-specific private recovery
  journal. This closes the earlier Apple product adapter gaps for those three
  filesystems. The desktop keeps its existing native default; explicit foreign
  filesystem selection is currently a CLI/helper API capability.
- All three new Apple routes passed normal downloaded-payload CLI lifecycles
  with Auto detection. The helper API passed 13 requests per filesystem,
  including wrong password, no overwrite, duplicate mount and busy close, plus
  four payload opens per filesystem. Every test closed normally. On the newer
  frozen-source overlay, 70 core tests passed, existing Apple APFS/exFAT and
  VeraCrypt ext4/UFS2/ZFS regressions passed, and the centralized-profile ext4
  smoke passed. These are staged-artifact results; final package validation
  remains pending.
- A read-only round-owned session observer independently captured UFS2
  superblock magic and OpenZFS sole-vdev labels during the native VeraCrypt
  regressions. It retains the actual mount transport and source/device/inode
  association rather than treating generic FUSE labels as filesystem proof.
- `applicability.json` now separates every one of the 130 native applicability
  cells from product outcome/adapter status and source, driver, provisioning or
  crypto-facility blockers. It will be reconciled to the final package receipts.

## Native physical VeraCrypt integration

- `479dee5` adds native VeraCrypt data-partition create/open/close with an
  administrator-controlled private engine and the existing native authorization
  service. It binds the selected unmounted writable GPT data partition to its
  parent, offset, size, UUID, model, transport and hardware serial read through
  the selected media's IOKit ancestry. Whole/internal/system/reserved devices
  are rejected. Owned virtual partitions instead bind the exact backing-image
  inode and native hdiutil association. Physical creation uses exFAT without a
  file-size operand; foreign file-only filesystem adapters remain excluded.
- Direct native controls passed on ordinary, root-restricted and native-service
  virtual partitions. Product virtual-partition controls passed four downloaded
  payload opens, wrong password, duplicate/busy close, whole-disk, missing-force
  and foreign-filesystem refusals. The actual Slint callbacks passed through an
  administrator-controlled helper. The production IOKit reader independently
  matched the inventoried Toshiba serial.
- After rechecking the exact Toshiba model, size, transport, serial, existing
  partition UUIDs and geometry, the round added a separate approximately 1 GiB
  partition in previously unallocated space. Its native product VeraCrypt
  create, wrong password and four downloaded-payload opens passed. The original
  APFS and exFAT directory samples were preserved. All samples closed normally;
  the new partition's root:operator 0640 node ownership was restored. Receipts
  and before/after identities are in `device/toshiba-veracrypt/`.
- Coordinator `3bfed46` adds the corresponding manual partition controls and
  guarded CLI/helper API filesystem allowance. Actual callbacks and software
  create/open snapshots at 100% and 120% passed against the executed overlays.
  Executed and later test-only formatting hashes remain separate in
  `build/device-executed-frontend-manifest.json` and
  `build/device-frontend-postformat.json`.

## Publisher follow-up and frozen package validation

The first frozen candidate `3bfed465c77d2b859761dc3fb37ff25d9da80123` passed
workspace check, 144 tests and the complete extracted-DMG conformance suite:
14 profiles, 280 checks. The package was built and verified with one Cargo
worker using Rust 1.100.0. DMG SHA-256:
`08959e888f994de5adbdd2b045e2c7c0af0d0824241af8e5cec5c679c42b52c8`.
Its 12 eligible inner-filesystem cases passed four payload opens each.

The complete directory sweep then exposed a new CryFS/UFS2 publication failure:
removing a published directory also removed its native AppleDouble companion,
leaving a stale entry in the outer staging snapshot. That failed artifact and
fixture remain under their original identity in
`cases/final-directory-ufs2-ptv_g8kj/`.

Commit `12a3f93` captures native companions through anchored descriptors before
publication can remove them. It validates the AppleDouble header, version,
entry bounds and association with the base entry; preserves the captured bytes
durably; refuses metadata winners and ordinary files at paired metadata names;
and retains errors for missing ordinary files. Configuration publication stays
last. Eight native unit tests passed, including malformed/symlink metadata,
ordinary standalone files, paired ordinary-file refusal and no-clobber cases.
The exact source overlay passed workspace check and 148 tests, plus 11 expected
positive filesystem regressions: CryFS/Cryptomator on UFS2, APFS, ext4, NTFS and
ZFS, and Cryptomator on exFAT. The known Apple exFAT CryFS refusal remained.
Module SHA-256:
`417d6ac39bb9962e41a330bcad454fb05b38983deabf3c03d8ecc0c2a22061db`.

Supplementary artificial metadata tests on UFS2 remain limitations, not passes.
A manually encoded companion whose native copy produced a different container
was refused without overwriting it. An unsupported malformed/symlink fixture
hit host-driver EIO and stalled its own normal unmount; SIGTERM to the exact
owned driver released the pending operations. Its raw image and logs are
retained. No global filesystem service or unrelated storage was changed.
UFS2 evidence qualifies the ordinary regular-file/directory vault patterns
actually exercised, rather than complete filesystem metadata semantics.

The first installed desktop test binary was placed outside `Contents/MacOS`.
Its in-process inventory consequently lacked the packaged backend while the
actual helper mounted successfully. That test assertion and cleanup failure
left one owned native mount and caused subsequent physical controls to refuse
an unverifiable mount. The exact source/mount was closed normally; the failure
receipts were preserved. With the test binary in a separate app clone's proper
bundle layout, the same frozen package passed the default VeraCrypt callback,
virtual-partition CLI four-open/negative controls, and actual partition callback.
Both inventory and mount engine paths/hashes and runtime provenance are in
`build/final-vc-discovery-layout.json`. The actual production bundle remained
unchanged. All six default callbacks and the 39 Apple foreign-filesystem helper
requests therefore have successful receipts; failed attempts are separate.

Commit `025f558` records a separately installed native macFUSE exFAT alternative
for directory backings. Mature Free exFAT v1.4.0 revision
`0372fdeab2b878736fa20c9c510a0766a7996cdc` builds against macFUSE 2. Its small
host-only patch implements exclusive rename on its existing single dispatcher.
The installed `3bfed46` package passed all three directory engines on this route,
with four payload opens each. Independent plain-native controls passed four
backing unmount/remount phases, 0700/0600 masks and file/parent fsync. Driver
SHA-256: `c11f6960a81e52c37e43d6655d2339796d24cf783e1383385a8700e3fa599a73`;
patch SHA-256: `8da5872d6b49786c6d28fd780f381f486659347979da65a97430f92fc8b932ea`.
The driver, license and provenance are host prerequisites, never app contents.
Apple's exFAT FSKit route continues to refuse CryFS's atomic primitive.

The native exFAT 16-way collision control retained one winner and all 15 losing
sources, but some losers received ERANGE instead of EEXIST through the current
macFUSE high-level path. This is an explicit errno-contract limitation, not a
concurrency pass. No installed macFUSE library was changed. The reproduction
and inferred upstream-library explanation are in `native/macos/exfat-macfuse.md`.

The renewed named final candidate is
`025f558499695a9962ff9754ee9663c0ba7bc95e`. Native run
`20261127T001035Z-fbbade78` passed workspace check and 151 tests, with no failures
and 12 ignored optional tests. Its DMG built and passed hdiutil verification.
SHA-256: `1a4849331e127039f13bc8665c8942a4145972f2089241bd276df243f9943cf9`.
The exact extracted package passed all 14 profiles and 280 conformance checks.

The renewed extracted app and its administrator-owned production installation
passed deep/strict signature verification. All 240 packaged files match; only
owner/mode hardening was applied to the qualification installation. The test
executable is added to a separate bundle clone in `Contents/MacOS`. The native
hardware serial reader passed against the Toshiba, and the software renderer
produced eight create/open frames at 100% and 120%; scrolled action buttons are
visible without horizontal overflow. The package is ad-hoc signed, not notarized.

## Final accepted evidence

The independent final index `validation/renewed-case-index.json` binds the exact
`025f558` commit, DMG hash, extracted application, administrator-owned production
installation, separate test-layout clone, CLI/helper hashes, executed harnesses,
common payload and native mount/driver/superblock identities. All 30 selected
cells passed four payload opens each, totaling 120 opens. Each verifies the
three PDFs, text append, second file, normal crypto closes and Buran's extra
reopen. The selected routes are:

| Filesystem | Native inner filesystems | Directory backings |
| --- | --- | --- |
| APFS | Apple image, APFS encryption, VeraCrypt | gocryptfs, CryFS, Cryptomator |
| exFAT | Apple image, VeraCrypt | gocryptfs/Cryptomator on Apple FSKit; CryFS on the separate native macFUSE exFAT driver |
| ext4 | Apple image, VeraCrypt with local FSKit | all three on separately installed native macFUSE ext4 |
| NTFS | Apple image, VeraCrypt with native NTFS FSKit | all three on native NTFS3G/macFUSE with private masks |
| UFS2 | VeraCrypt native regular-file view | all three on the repaired native UFS2 regular-file driver, within its recorded workload limits |
| ZFS | Apple image, VeraCrypt with native OpenZFS | all three with native OpenZFS |

The installed package passed all six default Slint callbacks through the actual
production helper. The default and manual partition forms also passed software
rendering at 100% and 120%. Guarded virtual-partition CLI and desktop controls
passed, including native authorization on root-restricted nodes; all physical
operations used administrator-controlled private engines. The three explicit
Apple foreign-filesystem helper cases each passed 13 requests and four payload
opens: 39 requests and 12 opens, including wrong credential, overwrite,
duplicate and busy-close refusals. Fresh Apple/VeraCrypt XFS create guards
refused before prompting or allocating a target.

Final production component SHA-256 values:

- CLI: `4054078be9a26388be0d5c7ccfadb9b4a3e610eefc691e40cfdbf95c4974dc54`
- Helper: `8f3e15db8c8772e93fba8e2bc8dc9c05e2ac123ac8cce8238bae6cc253199204`
- Desktop: `18376f69c3cf07f2e053ff721772d7fcc35a5700ebec7f4f8ed326efd9ed16b0`

The frozen desktop test executable is separately identified as
`f2487e1305dcf3a41fc19ba91a7566b560f4303010a0e535d26d3cc84292548a`;
it is not substituted for the production desktop binary. Its bundle ancestry
and runtime provenance match the production layout. The production bundle's
240 files and deep/strict signatures were rechecked after native tests.

`applicability.json` reconciles all 130 cells: **30 passed, 21 blocked,
20 unavailable, 59 not applicable**. The 21 blockers comprise 20 native
filesystem-route cells (Btrfs/XFS/UFS1/HAMMER2 across the five relevant engine
families) and Apple UFS2's block-device sync limitation. LUKS1/2 account for
20 unavailable native create/write routes. The 59 not-applicable cells are
APFS encryption's nine non-APFS selections and five BSD native facilities
absent on macOS, each across ten filesystems. No VM, network transport,
read-only route or prior-round pass fills these gaps.

## Cleanup and retained recovery evidence

Independent reconciliation caught an outer backing-fixture cleanup defect that
the first directory controller's successful payload status did not gate:
Spotlight retained handles on three scratch ZFS pools after vaults closed.
Their exact pool/vdev GUIDs, raw-image paths and Spotlight-only holders were
recorded. Normal unmount/export and scoped indexing controls refused; only
those three owned filesystem mounts were force-detached, followed by normal
pool export. This is recovery of failed fixtures, never a normal-cleanup pass.
The original receipts remain unchanged and are explicitly excluded by the
final index. No global Spotlight service or unrelated volume was stopped.

A fresh ZFS backing fixture used a legacy mount, `cachefile=none`, native
browse/device publication disabled, a private 0700 root and a no-index marker
before crypto operations. All three complete vault lifecycles passed again,
followed by **normal umount and normal zpool export**. Its receipt is
`cases/renewed-zfs-clean2-directory-zfs-fh9dcbji/receipt.json`; the exact three
per-backend results and command logs are retained beside it. A subsequent
read-only archival remount recovered the original receipt bytes and also
closed/exported normally; that archival read is not an extra write-workload pass.
Four older unmounted ext4 test-image attachments were detached normally, and
the old exFAT control mount also closed normally after a scoped no-index marker.

The final audit `validation/renewed-cleanup-audit.json` passed: **zero owned
mounts, hdi images, filesystem sessions or crypto/filesystem workers remain**.
The installed bundle still matches all 240 hashes and signatures. Buran's boot
identity remains `1795714904`; reboot persistence is unexercised by instruction.
All failed images, source/build records and earlier artifact identities remain
available. The complete local collection manifest binds 1,166 files; complete
package logs cover both 280-check DMG attempts. Root's independent package and
ZFS/cleanup audits accompany the evidence.

The physical APFS and Cryptomator samples used the staged CLI
`a38bd2f6ec10effe3f720f05f95fb40ca329053c502ee40652e90def83f9e5d2`
(`ced26af+9a5ca4d+folder-overlay`), with four and three basic payload opens
respectively. The physical VeraCrypt sample used CLI
`bb4accda1dceaec64f7d68f8027318d35a05670178e279e3495282876a9f3ce8`,
with four opens and executed-source hashes in the device frontend manifest.
They are representative same-round staged hardware results, not new `025f558`
USB passes. The publisher-only app change was followed by fresh guarded virtual
partition tests, not another Toshiba mutation. The USB remains connected with
encrypted APFS locked and the exFAT/VeraCrypt samples unmounted. Its final
identity/partition inventory is retained. Native authorization evidence is
specific to Buran's available passwordless route; another host's interactive
flow is not qualified here.

Primary eligibility sources:
https://github.com/macos-fuse-t/fuse-t
https://macfuse.github.io/
https://github.com/openzfsonosx/openzfs-fork/releases
https://ntfs.techtag.ch/
https://github.com/alperakcan/fuse-ext2
https://github.com/karan-vk/fuse-xfs
https://github.com/realchonk/fuse-ufs
https://github.com/h3ct0r/open_ext4_for_mac
https://github.com/tsujamin/userspaceLUKS
https://luksbox.penthertz.com/docs/security/cryptography
https://github.com/HuanchuanTech/xlinuxfs
https://github.com/gereoffy/btrfs4mac

## Supplementary native LKL bridge proof

After the frozen package qualification, the pinned public Darwin LKL source
built on Buran with LLVM 19.1.7 and Xcode 27.2. Seven profile tests passed;
957 used source/profile/header files matched the pinned Git inputs. The
universal archive contains arm64 and x86_64 slices; runtime controls used arm64.
Upstream platform/capability C tests and four common-payload opens passed for
ext4, Btrfs and XFS through the native local-file bridge. Btrfs and modern XFS
seeds were imported from fresh Ubuntu formatter controls, with verified hashes
and independent Mac clones. This is not native Mac formatting acceptance.

The unpatched block transport issued no host fsync and swallowed an injected
flush failure despite successful payload reopens. Commit `59b0a12` adds explicit
flush-capable registration while preserving existing registration semantics.
The final archive forwarded native fsync and propagated injected EIO on all
three filesystems, followed by normal LKL close. Other archive members were
unchanged. The old native XFS formatter's invalid V4 seed and the earlier
combined-lipo verification syntax failure remain retained failures.

The frozen package's 30 passed cells and three earlier USB sample identities
remain unchanged. No host-visible LKL filesystem mount, replacement driver
installation, new USB write or XFS guard removal occurred. Mounted macFUSE
integration still needs native errno/credential handling, permission and busy
controls, and a qualified native formatter. `native/macos/lkl-macfuse.md` records
the exact source and remaining gates. Private receipts, hashes and the proposed
adapter design are under `validation/native-lkl/` in the Mac evidence root.
