# macOS fresh native qualification checkpoint

Status: in progress. These are newly executed observations, not a release pass.
Buran was not rebooted. Physical USB sampling and frozen-candidate validation
remain pending. Private evidence is under
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
  EFI and an APFS partition/container with `Untitled`. No USB writes yet.

## Builds and product changes

Baseline `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d` workspace run
`20261126T203536Z-4c3d602b`: check passed; 122 tests passed, none failed,
11 ignored. CLI SHA-256:
`85d30dfaa47e2a734a1301f3cc8bf08a8d13a1c20339597ca06f5622fafedc34`.

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
  installed or counted as a repair. XFS remains incomplete.
- Btrfs has no eligible native route identified; its existing VM compatibility
  branch ran once in the reused initial sweep and is explicitly excluded.
  UFS1/HAMMER2 native create/read-write routes remain unavailable. GELI/PEFS,
  CGD and OpenBSD softraid formats have no corresponding macOS native facility.

LUKS candidates were reassessed. `userspaceLUKS` explicitly warns that writes
can destroy key material; it is ineligible. The native `open_ext4_for_mac` FSKit
candidate offers LUKS import, not this round's complete create workflow.
LUKSbox uses its own `.lbx` format and cannot substitute for LUKS1/LUKS2.

## Remaining work

Complete the running AES/Serpent/Twofish password/keyfile Auto conformance,
reconcile remaining driver/backend failures, exercise a small verified Toshiba
sample, package the frozen candidate and verify its CLI/helper, run final remote
workspace checks and the complete applicable matrix, collect feature observations
and receipts, and inspect all owned resource cleanup. Reboot persistence remains
unexercised on Buran by instruction.

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
