# macOS fresh native qualification checkpoint

Status: in progress. These are newly executed observations, not a release pass.
Buran was not rebooted. The three representative physical USB samples passed;
frozen-candidate package validation remains pending. Private evidence is under
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
- Btrfs has no eligible native route identified; its existing VM compatibility
  branch ran once in the reused initial sweep and is explicitly excluded.
  UFS1/HAMMER2 native create/read-write routes remain unavailable. GELI/PEFS,
  CGD and OpenBSD softraid formats have no corresponding macOS native facility.

LUKS candidates were reassessed. `userspaceLUKS` explicitly warns that writes
can destroy key material; it is ineligible. The native `open_ext4_for_mac` FSKit
candidate offers LUKS import, not this round's complete create workflow.
LUKSbox uses its own `.lbx` format and cannot substitute for LUKS1/LUKS2.

The later xlinuxfs assessment found GPL-2.0 native FSKit ext4/XFS/Btrfs source
(`f9da28e8cfdf6a4574a31e8baa9ba1811ef4c2e4`) using in-process LKL. Its
repository publishes no release artifacts and actual system mounts require a
restricted FSKit provisioning profile. Buran has no matching FSKit profile; an
unsigned build cannot establish a native mount. This is a blocked source-build
candidate, not evidence that no native FSKit project exists. The separate
btrfs4mac project explicitly supports reading only.

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

## Frozen package validation in progress

The named final Mac candidate is `3bfed465c77d2b859761dc3fb37ff25d9da80123`;
other hosts' later working-tree changes are excluded. Native run
`20261126T231244Z-1c26f1a8` passed workspace check and 144 tests, with no failures
and 12 ignored optional tests, using Rust 1.100.0 and one build worker. The DMG
built and passed hdiutil verification. SHA-256:
`08959e888f994de5adbdd2b045e2c7c0af0d0824241af8e5cec5c679c42b52c8`.

The extracted application passed deep/strict signature verification. Its exact
240-file administrator-owned qualification copy preserves all packaged bytes;
its directory/file group and other write bits were removed before privileged
backend testing, and signatures still verify. The package is ad-hoc signed,
not notarized. The final package conformance profiles, applicable filesystem
matrix, installed CLI/helper/desktop callbacks and owned cleanup audit are
running. All 130 ledger cells will be reconciled to those receipts. Reboot
persistence remains unexercised on Buran by instruction.

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
