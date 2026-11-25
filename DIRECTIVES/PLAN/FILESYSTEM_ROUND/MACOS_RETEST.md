# macOS unachieved-only filesystem retest

Host: `adjutant@buran`, macOS 27.2.1 (26B131), arm64, 2026-11-25.
Remote evidence: `/tmp/lurker-filesystem-retest-macos-20261125`.
Local evidence: `target/goal-validation/filesystem-retest/macos/`.
Final implementation: `82857ba0ccb9786ef079679c5a3781adb1eb6f10`.

## New achieved results

| Filesystem | Exact CLI route | Result |
|---|---|---|
| ext4 | VeraCrypt AES/password, private FUSE-T engine, unmodified fuse-ext2 over FUSE-T NFS | Create/RW persistence pass, seven ordinary CLI actions |
| XFS | VeraCrypt AES/password, private FUSE-T engine, unmodified fuse-xfs over FUSE-T SMB | Create/RW persistence pass, seven ordinary CLI actions |

Each final fixture is a fresh 0.5 GiB encrypted file. The ordinary CLI creates and
mounts it; the common harness creates a private payload directory, writes a valid
PDF and UTF-8 text, closes normally, reopens and checks both hashes, appends text
and creates another file, closes, reopens and checks all hashes again. The CLI
terminal exits between actions. Both final cases leave no plaintext mount,
auxiliary encryption session or filesystem-driver process.

- XFS receipt: `final-cli-cases/lurker-fs-cli-rej4s6tg/result.json`.
- ext4 receipt: `final-cli-cases/lurker-fs-cli-e_aafklj/result.json`.
- CLI SHA-256:
  `932162d3288422ffd80b956fb20e12827016eca201121063687cea8ed9948264`.
- Helper SHA-256:
  `e83f83be11d1fe21b60deab462ac1ea1a31ec9ac4066cbe92faee970c6cc17b6`.
- Existing private VeraCrypt SHA-256:
  `8aad857922b8f3a11bd3ec475cfd6f4418767cf9e1c120dddf45582786f88f6e`.
- PDF SHA-256:
  `f62d9bb8cdf4722e68937ce0fd939201585a74e28ea839b3bc1441a2dd29597b`.
- Final text SHA-256:
  `866f0eba1e2b2a6c4b37bb496c706e156bce5402df6eafbaf3dc848f8e25cb2d`.
- Added-file SHA-256:
  `86439bc0761646c5ed65902279dd551d892c1f038a76974783fd66b814e0f677`.

The final source archive passed `cargo check --workspace` and
`cargo test --workspace` on Buran: **108 passed, 0 failed, 11 ignored**. CLI and
helper built. The default workspace run also included existing headless desktop
software-renderer unit tests; no desktop application flow was exercised or
qualified. Rust was 1.99.1, Apple clang 21.0.0. Native functional receipts are
CLI-only. These are development artifacts, not a newly qualified release package.

## Implementation and native prerequisites

The final adapter is restricted to **VeraCrypt ext4/XFS**. It preserves existing
native defaults and cipher/KDF profiles. It creates a fully initialized VeraCrypt
container with no inner filesystem, opens the authenticated regular decrypted
auxiliary image, and normally detaches only its unused hdiutil virtual device.
The ordinary virtual device otherwise exclusively locks that regular file.
The encryption service stays live while native formatters and filesystem drivers
use the regular image. Close drains the identified filesystem normally, stops
only its verified driver, and then closes the existing VeraCrypt service.

Private session records retain encrypted-source and decrypted-image identities,
the auxiliary mount identity, filesystem mount identity, and PID/start time.
Drivers start with `setsid`; inheriting the CLI PTY originally killed them when
the CLI exited. Failed startup/close retains recovery ownership. No private
executable is elevated. The earlier speculative Apple/NTFS routing was removed;
those experiments are not exposed as writable support.

Qualified source-built tools:

| Tool | Source / version | SHA-256 |
|---|---|---|
| fuse-xfs | `f940a1d3e8cbb7a54897eefb34e522042c61e393` | `e4d2b31fdc9a33ce0d6002b4fc0afdfd184b0f2eca7eba2cbba2ce2776d732bb` |
| mkfs.xfs | Same unmodified source, old xfsprogs implementation | `7002d57d859db5377f07aefee4d5fcc1faada911374b31ef07f3d332b4b7e280` |
| fuse-ext2 | `ae35afb9ab08d87c66c1e021df792b3a7c4308b0` | `e4047e85398dd8785e17ac17b2b8c63c339e559a0761cd8d5f5c2ad9a19e92dc` |
| mkfs.ext4 | Installed native e2fsprogs 1.47.4 | `a751e75a477939f438746d6c36e0c8849834c4ec186830a0c98d5112c21ba4b1` |

All three upstream working trees checked during collection have zero tracked
source changes. Builds used ordinary FUSE-T compiler/link flags. fuse-ext2's
Darwin `getxattr` callback needs a modern-Clang compatibility warning flag; its
independent installer GUI target fails current SDK headers and was not used.
Extended attributes and complete metadata parity remain unqualified. The tool
stage depends on existing FUSE-T/Homebrew libraries and is not relocatable or
offline-package qualified. Exact build/stage instructions are in
`docs/macos-filesystems.md`; provider dependency output is in `result.json`.

## Missing cases and bounded attempts

| Missing case | New investigation or attempt | Disposition |
|---|---|---|
| Btrfs RW | Rechecked xlinuxfs `f9da28e8cfdf6a4574a31e8baa9ba1811ef4c2e4`: source system-mount tests require restricted FSKit provisioning. Its paid binary was not purchased. PolyDrive Disk 1.2.6 documents raw `.img`/`.dmg` plus RW Btrfs/ext4/XFS, but its free download is a three-day trial followed by a paid lifetime unlock; it is ineligible under the free-of-charge requirement | No CLI pass. The earlier download stall has no established cause, but completing installation would not resolve the paid-license restriction. btrfs4mac is RO, so it cannot satisfy the missing RW case |
| UFS2 RO | Tried the upstream official `fuse2` feature for fuse-ufs `ee5c4a447da98220a11edb69a225d51379afc4fb`, then corrected ordinary header/API flags. fuse2rs 0.1.2 fails Darwin compilation with ten actual type/API errors. Default fuser still requires osxfuse >=2.6.0 while FUSE-T accurately advertises 1.2.9 | No mount/CLI pass. No false pkg-config version, dependency patch or macFUSE/KEXT substitution. UFS2Tool can format images but does not provide a macOS mount, so combining it cannot resolve the driver blocker |
| NTFS RW | Built unmodified FUSE-T NTFS-3G `f0e5cb0274e30334dd03aefb4fd5a14c239e6756`. Raw regular-file NFS control passes three payload cycles; SMB control fails initial file flush with ENOSYS/EIO. Existing native formatter works | No encrypted CLI RW pass. NTFS-3G refuses unprivileged block devices; Darwin raw nodes fail its unaligned reads. Existing VeraCrypt auxiliary file rejects NTFS-3G's required record lock with Operation not supported. A separate signed FSKit driver still needs operator enablement. Prior successful NTFS RO persistence was not repeated |
| ZFS create/RW | Rechecked existing matching signed OpenZFS installer/KEXT prerequisite; native OpenZFS needs excluded Apple-silicon security/restart setup. anylinuxfs uses an excluded microVM | No create/mount/CLI attempt after the unchanged prerequisite block; no false failure/pass claim |

FSKit read-only inventory used Apple's public `FSClient` API, not registration
preferences. It reports only Apple built-in modules enabled. Existing Ext4Kit
and TT NTFS plug-in registration does not establish enablement. No available
FSKit provisioning profile was found. open_ext4_for_mac
`b5e53353d24397799c916084591f0e60d1fbb21b` and NTFS4Mac
`dccf4fbb05b0a23e6f96f2fc765cde413ab1daf5` document signing/provisioning and
operator enablement requirements; NTFS4Mac had no latest GitHub release asset.
No entitlement, approval or security workaround was attempted.

Additional retained controls distinguish upstream capability from product
qualification: raw-file ext4/XFS/NTFS-NFS controls pass; encrypted unmodified
VeraCrypt auxiliary-file ext4/XFS controls pass. Apple direct-device experiments
include the Darwin XFS formatter's device rejection, native mounted-device
EBUSY, NTFS block/raw restrictions, and early PTY-lifetime failures. Preview
receipts remain labeled with their original identity and are not counted as
final passes. Initial daemonized NTFS probe workers were stopped only after
normal unmount and exact PID/start-time checks; the bounded SMB reproduction
uses foreground/no-detach mode and retains its own logs.

## Cleanup and exclusions

Final `cleanup.json` confirms **zero owned attached images, filesystem mounts,
backend/build workers, or active registry sessions**. The empty owned registry
was removed. Current login-user VeraCrypt enumeration reports no mounted
volumes. Buran has 465 GiB free. Scope-owned failed fixtures, source trees,
native tool stages and logs are retained; unrelated sessions and caches were
preserved. PolyDrive was not installed. No new system extension was approved.

No paid purchase, VM, new upstream patch, reboot, security reduction, physical
storage, SQLite or fault workload was used. Previous APFS/exFAT/FAT and NTFS RO
achievements were not requalified. Any native RO NTFS attachment inside the new
failed RW experiment was setup only, without repeating its prior RO payload
verification. Buran is released for review.

## Subsequent installation and license audit (2026-11-25)

The user stopped installation after identifying the PolyDrive pricing error.
Only read-only checks continued. The earlier cleanup section describes the
retest checkpoint, before these subsequent installation attempts.

| Tool | Free-of-charge requirement | Actual state when installation stopped |
|---|---|---|
| PolyDrive Disk | Excluded: App Store lists a three-day trial and $9.99 lifetime purchase. Free download does not mean free ongoing use | `/Applications/PolyDrive Disk.app` absent; owned `mas install` PID 89742 no longer running. No purchase was invoked by this work |
| TT NTFS Native | Free NTFS read/write; upstream GPL-2.0 source and downloadable notarized binary | Existing app remains installed; previous actual mount reported its FSKit module disabled. No new successful writable mount |
| OpenZFS on macOS | Free, CDDL-1.0; no paid feature unlock | Signed OpenZFS 2.4.1p2 macOS 27 arm64 package installed successfully; receipt `org.openzfsonosx.zfs`. No kernel-extension approval, reboot or pool qualification |
| macFUSE | Free end-user use under upstream license. Bundling binaries with commercial software, including automatic installation in that context, requires separate permission | 5.5.0 installed through Homebrew; core and preference-pane receipts present. No extension approval or successful new UFS2 mount |
| fuse-ufs | Free, BSD-2-Clause | Unmodified 0.6.0 build with the installed macFUSE SDK fails because `nix::unistd::daemon` is unavailable on Darwin. No UFS2 driver installed |

Installation/build logs remain on Buran under
`/tmp/lurker-macos-driver-install/`. These package installations do not establish
any additional achieved filesystem pair. macFUSE is not a filesystem driver by
itself, and this attempt does not change Lurker's approved FUSE-T runtime policy.

Subsequently, at the user's request, `brew uninstall --cask macfuse` completed
successfully on Buran. Verification found no macFUSE cask, package receipts or
`/Library/Filesystems/macfuse.fs` bundle. FUSE-T 1.2.9 core/FSKit receipts and
`/usr/local/lib/libfuse-t.dylib` remain present. No boot security setting was
changed and no reboot was performed.

After the operator brought Buran back, macFUSE 5.5.0 was present again from the
operator's downloaded package (`~/Downloads/macfuse-5.5.0.dmg`), outside
Homebrew. Read-only checks found the package receipts and bundle installed,
but no loaded macFUSE kernel extension. The boot policy reports Full Security;
the installation-time kernelmanagerd log reports that the extension is not
approved to load. Package installation is complete; kernel-backend activation
is not established. No boot-policy modification or extension-load attempt was
made by this audit. FSKit remains a separate backend that does not require
Reduced Security; fuse-ufs compatibility with it is not established.

At the user's request to install missing tools through Homebrew, installed
Cryptomator 1.19.7 (`cryptomator`) and VeraCrypt FUSE-T 1.26.31
(`veracrypt-fuse-t`). Homebrew verified the upstream download checksums;
both installed applications passed strict deep code-signature verification and
Gatekeeper assessment as Notarized Developer ID applications. Existing FUSE-T
was verified by receipt and library, then preserved using `--skip-cask-deps`
for the VeraCrypt cask. These standalone apps do not replace Lurker's private
backends. e2fsprogs, autoconf, automake, libtool, pkgconf, CMake, Ninja,
libgcrypt, libgpg-error and mas were already installed. TT NTFS and OpenZFS
were already present from upstream and were not reinstalled. macFUSE was left
untouched. Install logs: `/tmp/lurker-homebrew-tools-20261125/` on Buran.
No new filesystem capability or GUI workflow is qualified by these installs.

License and pricing sources checked directly:

- https://apps.apple.com/us/app/polydrive-disk-ntfs-linux/id6815775853?mt=12
- https://github.com/dr-kbadawi/ttntfs
- https://openzfsonosx.org/
- https://github.com/openzfsonosx/openzfs/blob/development/LICENSE
- https://raw.githubusercontent.com/macfuse/macfuse/release/macfuse/LICENSE.txt
- https://github.com/realchonk/fuse-ufs/blob/main/LICENSE.md

Primary references:

- https://apps.apple.com/us/app/polydrive-disk-ntfs-linux/id6815775853?mt=12 (three-day trial and $9.99 lifetime purchase; checked 2026-11-25)

- https://github.com/karan-vk/fuse-xfs
- https://github.com/alperakcan/fuse-ext2
- https://github.com/macos-fuse-t/ntfs-3g
- https://github.com/HuanchuanTech/xlinuxfs/blob/main/docs/build.md
- https://polydrive.net/disk/
- https://apps.apple.com/vn/app/polydrive-disk-ntfs-linux/id6815775853
- https://github.com/realchonk/fuse-ufs
- https://github.com/h3ct0r/open_ext4_for_mac
- https://github.com/vahidzekic/ntfs4mac
- https://www.openzfsonosx.org/
