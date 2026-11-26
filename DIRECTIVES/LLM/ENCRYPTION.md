# Lurker Encryption Scope and Profiles

Use this file as the source of truth for selected encryption backends and create-time cipher policy.

The global filesystem goal is **Create + Read/Write**, with one mount/unmount
workflow and no user-selectable read-only mode. Read-only-only or import-only
support is a shortcoming, not a desired feature or a complete achievement.
Follow the support and acceptance rules in `CODEBASE.md`.

## Approved Backend Scope

✅ implemented with packaged native CLI lifecycle coverage; 🟦 selected, native qualification incomplete; — outside approved scope. These are backend/format names, not cipher algorithms. A tick does not establish desktop, security-audit or complete fault/release qualification; exact tested revisions and limits are in `DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md`.

| Backend / format | macOS | Linux | FreeBSD | DragonFly BSD | Windows | NetBSD | OpenBSD |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Apple Encrypted Disk Image | ✅ | — | — | — | — | — | — |
| APFS Encrypted | 🟦 | — | — | — | — | — | — |
| Startup-volume FileVault management | 🟦 | — | — | — | — | — | — |
| LUKS2 | 🟦 | ✅ | 🟦 | 🟦 | — | 🟦 | 🟦 |
| VeraCrypt | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 | ✅ |
| CryFS stable 1.x | ✅ | ✅ | 🟦 | — | 🟦 | 🟦 | 🟦 |
| gocryptfs | ✅ | ✅ | ✅ | — | 🟦 | 🟦 | 🟦 |
| GELI | — | — | ✅ | — | — | — | — |
| CGD | — | — | — | — | — | ✅ | — |
| softraid CRYPTO | — | — | — | — | — | — | ✅ |
| softraid RAID1C | — | — | — | — | — | — | ✅ |
| PEFS | — | — | ✅ | — | — | — | — |
| Cryptomator | ✅ | ✅ | 🟦 | — | 🟦 | 🟦 | 🟦 |
| LUKS1 | 🟦 | ✅ | 🟦 | ✅ | — | 🟦 | 🟦 |
| securefs | 🟦 | 🟦 | 🟦 | — | 🟦 | 🟦 | 🟦 |

- Do not add other backend/OS pairs without an explicit scope change. EncFS, fscrypt, OpenZFS, plain dm-crypt, Loop-AES, TCG Opal, commercial-only backends, encrypted archives, backups and rclone crypt are excluded. LUKS still uses dm-crypt internally. Research availability is not approval to integrate.
- macOS runtime policy: VeraCrypt packages declare their separately installed FUSE runtime. The default FUSE-T build uses SMB loopback and is excluded from native-only qualification; the explicit audited `--fuse-runtime macfuse` build provides the qualified native transport. gocryptfs, CryFS and Cryptomator use local FUSE-T FSKit. The full bundle requires the qualified FUSE-T 1.2.9 helper for gocryptfs. Exact package/host evidence is recorded with its actual transport; see `docs/macos-backends.md` and `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/MACOS_RESULTS.md`.
- Planned support requires native validation: gocryptfs macOS is beta and FreeBSD is basic; VeraCrypt targets FreeBSD x86-64 14+ and OpenBSD x86-64 7.8+, subject to inner-filesystem support. CryFS 2.0 alpha is excluded.
- Prefer LUKS2 for new Linux volumes; LUKS1 adds compatibility and the DragonFly native path. Do not apply LUKS2/Argon2id format arguments to LUKS1. Qualify DragonFly cryptsetup versions, patches, profiles and disk/file-container behavior before release; selection is not security approval. PEFS needs a matching FreeBSD kernel module. Cryptomator requires create/mount/credential API and application-filesystem validation.
- Existing profiles below remain authoritative. Select and document profiles for new backends during implementation; do not apply LUKS/VeraCrypt cipher options to folder backends automatically.

## Single-file CLI encryption

Age, GnuPG OpenPGP, and OpenSSL CMS are approved for one-file encryption and decryption in the CLI. Age uses the bundled Rust library and its standard payload and passphrase profiles. GnuPG uses installed tooling and the user's existing keyring. OpenSSL CMS uses installed OpenSSL and creates AES-256-GCM recipient envelopes. The file commands run without elevation. The [file encryption plan](../PLAN/FILE_ENCRYPTION.md) records the supported credentials and native validation scope.

## Planned integration batches

- **Batch 1** — DragonFly BSD VeraCrypt file containers via the [native `tcplay` and device-mapper route](../PLAN/DRAGONFLY_VERACRYPT.md), macOS APFS Encrypted data volumes via `diskutil`, and securefs on Linux, macOS, and FreeBSD.
- **Batch 2** — VeraCrypt on NetBSD.
- **Batch 3**
  - LUKS1 and LUKS2 on macOS, FreeBSD, OpenBSD, and NetBSD, plus LUKS2 on DragonFly BSD. Start with file-backed AES profiles. Qualify Btrfs access separately for existing Linux Lurker vaults.
  - gocryptfs and securefs on OpenBSD and NetBSD.
  - Stable CryFS 1.x on OpenBSD and NetBSD, and Cryptomator on FreeBSD, OpenBSD, and NetBSD. FreeBSD and NetBSD CryFS have native private builds and core routes; fresh packaged lifecycle qualification is in `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/`.

### Windows roadmap

Windows packaging and native qualification are planned. Use `cppcryptfs` for the gocryptfs format on Windows. Windows BitLocker (including BitLocker To Go) and EFS are planned. Linux, macOS, and FreeBSD are planned for opening existing BitLocker volumes, with writable access subject to qualification for each inner filesystem.

## Physical-device setup scope

- Support physical-device encryption setup where feasible on Linux, macOS, FreeBSD, DragonFly BSD, NetBSD and OpenBSD. Qualify each backend and device type on its native OS before advertising support.
- Physical-device adapters cover Linux, FreeBSD GELI/VeraCrypt, NetBSD CGD, OpenBSD CRYPTO/RAID1C/VeraCrypt and DragonFly LUKS1/VeraCrypt. FreeBSD checks the complete GEOM topology, partition identity and disk serial before destructive operations; system disks, mounted or consumed providers, and whole disks carrying partitions are refused. NetBSD stores mandatory recovery parameters persistently by checked GPT partition UUID under `/var/db/lurker-cgd/`; wedge numbers are not stable identifiers. OpenBSD mirrored recovery descriptors remain under `/var/db/lurker-raid1c/`. DragonFly binds GPT storage UUID, offset/size and native device identity, checks whole-disk consumers, and requires a physical-capable private wrapper before VeraCrypt initialization. macOS APFS and VeraCrypt accept guarded external data partitions. Implemented adapters do not establish completed package or full filesystem-matrix qualification; fresh evidence and remaining gates are recorded in `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/`.
- The APFS Encrypted scope covers native data-volume setup and unlocking, mounting, and locking existing encrypted APFS volumes via `diskutil`. Apple Encrypted Disk Image uses the existing `hdiutil` backend. Startup-volume FileVault management uses `fdesetup` and native authorization.

## Create Profile Scope

- Applies to create flows only.
- `--filesystem` selects the inner filesystem of a file container or a privileged Linux device where supported by the native adapter and installed filesystem tools. Native macOS APFS data partitions may explicitly select APFS. It does not select a different encryption engine or alter cipher/KDF profiles. Omitting it preserves existing native defaults. Directory engines do not accept this option; native APFS encryption accepts APFS only. Current installation and CLI payload qualification is recorded under `DIRECTIVES/PLAN/FILESYSTEM_ROUND/`; fresh device qualification is in `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/`.
- Linux UFS1 uses `makefs-freebsd` and the native `44bsd` driver. Writable creation requires a matching module built with upstream `CONFIG_UFS_FS_WRITE`; stock distribution modules commonly omit it. LUKS2 creation selects 512-byte logical sectors for UFS1 because the driver's initial probe requires them. This hardcoded filesystem geometry overrides Twofish's usual 4096-byte sectors and leaves every cipher, key size and KDF cost unchanged. Other filesystems retain their existing sector policy; unlocking existing volumes preserves their geometry.
- Supported `--cipher` values: `aes`, `serpent`, `twofish` for LUKS/VeraCrypt. Apple images accept only `aes`.
- No cascades.
- No user-tunable low-level crypto knobs.
- Keep these profiles hardcoded in code.

## LUKS2 Profiles (Linux)

All three profiles use `--sector-size 512` when creating UFS1, as required by
the native driver's probe. The defaults below apply to other filesystems.

- `aes`
  - label: `AES`
  - `cryptsetup luksFormat` args:
    - `--type luks2`
    - `-c aes-xts-plain64`
    - `-s 512`
    - `--pbkdf argon2id`
    - `--pbkdf-memory 1048576`
    - `-i 5000`
    - `--use-urandom`
  - extra open args: none

- `serpent`
  - label: `Serpent`
  - `cryptsetup luksFormat` args:
    - `--type luks2`
    - `-c serpent-xts-plain64`
    - `-s 512`
    - `--pbkdf argon2id`
    - `--pbkdf-memory 2097152`
    - `--pbkdf-parallel 4`
    - `-i 10000`
    - `--use-urandom`
  - extra open args: none

- `twofish`
  - label: `Twofish`
  - `cryptsetup luksFormat` args:
    - `--type luks2`
    - `-c twofish-xts-plain64`
    - `-s 512`
    - `--pbkdf argon2id`
    - `--pbkdf-memory 1048576`
    - `--pbkdf-parallel 4`
    - `-i 5000`
    - `--sector-size 4096`
    - `--use-urandom`
  - extra open args:
    - `--perf-no_read_workqueue`
    - `--perf-no_write_workqueue`
    - `--perf-submit_from_crypt_cpus`
  - apply those open args automatically when a Twofish LUKS header is detected

## LUKS1 Profiles (Linux; DragonFly qualified separately)

- Explicit `--type luks1`; Linux `luks` still creates LUKS2 by default.
- AES, Serpent and Twofish use their respective `*-xts-plain64` modes with 512-bit keys, PBKDF2/SHA-512 and 512-byte sectors.
- Iteration times remain 5000 ms (AES/Twofish) and 10000 ms (Serpent). No Argon2, memory/parallelism options or LUKS2-only format arguments.
- DragonFly's native cryptsetup and kernel algorithms require independent qualification; its adapter must document any compatible fixed profile differences before advertising support.

## Native BSD create profiles

- FreeBSD GELI: AES-XTS with 256-bit component keys (512-bit combined XTS), 4096-byte sectors, PBKDF2 with 600000 iterations for passwords, UFS inside. Password or binary keyfile, never both.
- NetBSD CGD: AES-XTS512, Argon2id v19 (262144 KiB, 3 iterations, parallelism 2), FFSv2 inside by default. Explicit non-FFS selections use a generated disklabel and a data partition. Writable FFS mounts require active NetBSD FFS journaling (WAPBL), which requests backing-device cache flushes at journal commits. Password only. The mandatory `<image>.cgdparams` sidecar contains salt/KDF/recovery parameters; back it up with the image. Only the strict generated profile with the `ffs` verifier (FFS) or `disklabel` verifier (other selected filesystems) is accepted; arbitrary executable `shell_cmd` parameters are rejected.
- OpenBSD softraid CRYPTO and RAID1C: native AES-XTS and password KDF with FFS inside. Both are password-only. Native keydisks differ from regular keyfiles. RAID1C creates a mirror of two independent regular files and publishes a `.raid1c.json` descriptor after both members. Explicit degraded recovery can unlock an existing array from one surviving member. Both formats refuse read-only crypto attachment.
- DragonFly LUKS1: AES/Serpent/Twofish XTS/plain64 with 512-bit keys, PBKDF2/SHA-256 and 5000 ms iteration time, native UFS inside. Accepts either a password or a binary keyfile. Its cryptsetup 1.1.2-derived implementation has different compatibility constraints from modern Linux; native functional tests do not replace security/maintenance qualification.
- DragonFly unlock validates the LUKS1 header and keyslot geometry for those profiles. It copies at most 16 MiB of metadata into the root-private session registry. The stock `dm_target_linear` module presents that metadata to cryptsetup while the payload stays in the original file. Existing images keep their ownership and on-disk layout.
- Native BSD disk adapters accept regular file containers through root-authorized native file devices. FreeBSD GELI additionally accepts checked GEOM data providers, and NetBSD CGD accepts checked GPT data wedges. Physical providers use the same identity-checked session journal without an owned vnode lease. Filesystem repair remains manual.

## Directory create profiles

- gocryptfs 2.7: native AES-256-GCM contents, EME names and HKDF; explicit scrypt `N=2^16` for password wrapping.
- CryFS stable 1.0.3: AES-256-GCM, 16384-byte blocks and the stable upstream scrypt configuration. CryFS 2.x alpha is excluded.
- NetBSD CryFS uses host-provided base ReFUSE API 29/puffs as the login user.
  An administrator must enable native user mounts and grant scoped device
  access; Lurker checks these prerequisites before changing vault data and never
  configures them automatically. Native fsid and process birth identities govern
  session ownership. Ordinary PDF persistence and credential/duplicate/busy
  controls pass, but base ReFUSE rejects shared writable mappings, so SQLite WAL
  remains unsupported. Do not weaken cache or durability behavior to hide that
  host limitation. Packaged and full backing-filesystem qualification remain
  separate gates.
- Cryptomator format 8: upstream `SIV_GCM` suite (AES-SIV names, AES-GCM contents), with upstream scrypt key wrapping.
- gocryptfs, CryFS and Cryptomator accept a password and their fixed format profile. They take no size, keyfile, alternate `--cipher` or destructive replacement option. PEFS has a separate native FreeBSD profile and module qualification.
- FreeBSD PEFS: AES-256-XTS with PBKDF2/600000, a verifier chain in `.pefs.db` and profile metadata in `.pefs.conf`; password or binary keyfile. Requires the matching kernel module and native utility. Preserve these metadata files with the encrypted directory.
- Profiles describe implementation policy, not completed platform qualification. Keep matrix entries marked selected until packaged native lifecycle tests pass.
- Newly created Linux/BSD container files and recovery metadata belong to the authorized invoking user (mode 0600); newly formatted filesystem roots are private to that user (0700). Linux initializes Btrfs root metadata through an empty `mkfs.btrfs --rootdir` seed; BSD uses a journaled private initialization mount. Never change ownership while unlocking existing storage.

## LUKS Keyslot Management (Linux)

- Each slot accepts either a password or a binary keyfile. Slots are independent alternatives; any one enrolled credential can unlock the volume.
- List, add and remove slots using the shared core. LUKS1 supports slots 0–7; LUKS2 supports slots 0–31, subject to header space. Adding without a slot selects a free slot.
- Additional LUKS2 slots use the create cipher profile's Argon2id costs. LUKS1 retains PBKDF2, with the corresponding profile's iteration time. Existing volume keys, ciphers and data remain intact.
- Keep password bytes on stdin. For addition, send existing and new credential bytes consecutively with explicit `--keyfile-size` and `--new-keyfile-size`; never put secrets in argv or temporary files.
- Authenticate removal with a credential for a surviving slot. Explicitly enable native passphrase verification: cryptsetup otherwise treats stdin as batch mode and can remove a slot without authentication.
- Refuse deletion of the last usable slot, including when other slots are unbound. Reject occupied, inactive and out-of-range slot selections.
- Keyslot management requires Linux Privileged mode; header operations on regular files run without elevation when accessible to the user.

## VeraCrypt Profiles

- Supported create ciphers map to VeraCrypt encryption names:
  - `aes` -> `AES`
  - `serpent` -> `Serpent`
  - `twofish` -> `Twofish`
- Hash is fixed: `SHA-512`
- Common create args:
  - `-t`
  - `--create`
  - `--force`
  - `--volume-type=normal`
  - `--hash=SHA-512`
  - `--filesystem=none` on Linux (followed by mkfs.btrfs)
  - `--filesystem=exFAT` on macOS (VeraCrypt formats the virtual disk)
  - `--random-source=/dev/urandom`
  - `--pim=0`
  - `-k ""` for password authentication, or `-k <keyfile>` with an empty password for keyfile-only authentication
- If a passphrase is supplied non-interactively, also add:
  - `--stdin`
  - `--non-interactive`
- File-backed create adds:
  - `--size=<MiB>M`
- Block-device create adds:
  - `--quick`
  - omit `--size`: VeraCrypt determines the full device size; `--size=max` can fail for device-hosted volumes.

### macOS VeraCrypt data partitions

- Native external GPT data partitions require `--force`, no file size, and the currently qualified exFAT filesystem. Reuse the fixed cipher/hash/PIM/credential profile and existing device `--quick` policy from `workflow.rs`; never silently substitute file-only filesystem adapters.
- The private engine and ancestor path must be administrator-controlled before its native core service can authorize device access. Frontend/helper remain in the login-user session. Native device ownership is restored after creation.
- Bind the selected partition UUID, offset/size, native node and parent model/transport/tree/hardware serial. Refuse whole/internal/system/mounted/reserved targets and existing APFS containers or system-role parents; recheck identity before operation. Virtual data partitions must match their owned backing image and attachment. Auto detects actual encrypted APFS data volumes instead of treating every `/dev` path as APFS.
- Fresh native virtual, desktop-callback and Toshiba USB proof is recorded in `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/MACOS_RESULTS.md`. Native-only qualification uses the audited macFUSE build. Interactive authorization outside Buran's pre-authorized privilege route remains unqualified. See `native/macos/veracrypt-partitions.md`.

### DragonFly VeraCrypt file containers

- Select `veracrypt` explicitly for create. LUKS1 remains the native default. Auto open keeps a recognized LUKS signature on the LUKS path and authenticates other regular files through the private wrapper.
- Create normal, fully initialized volumes with AES, Serpent, or Twofish in XTS mode. The wrapper uses `SHA512-VC` (PBKDF2-HMAC-SHA-512, 500,000 iterations, PIM 0) and 512-byte sectors. Candidate creation defaults to native HAMMER2 with the `DATA` label. Select FAT32 for exchange. Both profiles require at least 260 MiB. FAT32 uses clusters of at least 4 KiB and a 4 KiB-aligned data region. Imports must match the header profile and a recognized filesystem profile.
- Creation sizes each FAT explicitly to cover the data clusters and the two reserved entries. Imports must also have enough FAT entries for their declared data region.
- The core writes and syncs random data through the private staging file descriptor before vnode attachment. The private wrapper creates the VeraCrypt header on the attached, fully initialized file.
- Accept a nonempty password of up to 64 UTF-8 bytes or one nonempty binary keyfile. VeraCrypt uses up to the first 1 MiB of keyfile contents. Both credentials enter the wrapper through its inherited input pipe.
- FAT32 assigns private, per-mount ownership to the authorized invoking user. New HAMMER2 filesystems store that user's ownership and mode `0700`. Close syncs the verified ciphertext file after filesystem and mapper teardown and before vnode detach. Read-only VeraCrypt requests fail before vnode attachment because the DragonFly 6.4.2 device mapper does not enforce read-only writes.

## VeraCrypt Authentication Decision

- Support either a password or one non-empty regular keyfile. Keyfile contents are binary and are passed to the native crypto tool without trimming or text conversion.
- Do not implement combined password-and-keyfile authentication in the CLI or desktop, for either create or mount, on Linux or macOS. This decision does not prohibit keyfile-only authentication or mandate password-only authentication.
- With combined authentication, both the password and the required keyfile contents are needed to unlock the container. Permanently losing either, without a recoverable copy, means losing access to the data; neither can substitute for the other.
- This is a deliberate practical choice: requiring both a password and a keyfile adds a second required credential to preserve and back up, increasing the risk of accidental loss of access. Omitting combined authentication is intentional, not an implementation backlog item.

## Linux Privileged VeraCrypt Password Transport

- Cipher, hash, PIM and KDF defaults remain as specified above.
- Mount/open uses `cryptsetup open --type tcrypt --veracrypt`.
- Supplied TCRYPT passphrases go directly through stdin with `--batch-mode`; do not use `--key-file -`, which mixes a TrueCrypt keyfile with the password.
- Supplied passwords cannot be empty or contain NUL or line breaks, because TCRYPT password input is line based. Keyfile-only authentication passes the actual keyfile path with `--key-file` and an empty password as a newline on stdin.
- Mounting does not require the VeraCrypt binary. Creation still does.

## Opinionated Intent

- `aes` is the default.
- `serpent` is the heavier/slower paranoid option.
- `twofish` is the non-AES alternative with explicit open-time perf tuning.
- The user chooses only the cipher family. The rest of the crypto profile stays fixed.

## Linux Unprivileged VeraCrypt

- Same normal-volume AES/Serpent/Twofish, SHA-512, PIM 0, password-or-keyfile profiles as the privileged route. Full initialization; no quick-format file default.
- Create the VeraCrypt file with `--filesystem=none`, expose its decrypted image through the file-only userspace helper, then run `mkfs.btrfs` on that regular FUSE file.
- Btrfs is mounted through the LKL filesystem helper with `compress=zstd`. New filesystem ownership belongs to the login user; existing container permissions are preserved.
- The userspace helper reads passwords from stdin with a newline terminator, in a UTF-8 locale. Preserve spaces and reject embedded NUL/newlines.
- Report actual filesystem access capability; preserve internal write-protection and mount-state checks without exposing a selectable read-only mode.
- No physical-device creation, LUKS, tags, custom PIM or hidden-volume controls are added by this mode.

## Sync Rules

When changing any create profile, update all of:

- `crates/lurker-core/src/workflow.rs`
- this file
- `README.md` if user-visible behavior changed

If cipher choices change, also update:

- `crates/lurker-core/src/model.rs`
- `crates/lurker-cli/src/cli.rs`
- `apps/lurker-desktop/src/logic.rs`
- `apps/lurker-desktop/ui/app-window.slint`

## Apple Encrypted Disk Image (macOS)

- Create defaults to `apple` on macOS; Linux defaults to `luks`.
- Native default: `hdiutil create -type UDIF -fs APFS -volname Lurker -encryption AES-256 -stdinpass`; explicit exFAT uses the same fixed crypto profile.
- Explicit ext4, NTFS and ZFS use the profile from `workflow.rs`: `hdiutil create -srcdevice <identified-zero-only-virtual-disk> -format UDRW -encryption AES-256 -stdinpass`, then authenticate the encrypted image and format its identified virtual device with native host tools. No existing plaintext or credentials enter the blank source. Filesystem drivers/tools are installed separately; native attachment, process/mount and ZFS GUID identities govern recovery and normal close. See `native/macos/apple-filesystems.md`.
- Size is specified in MiB. File containers only, minimum 32 MiB for native defaults and 384 MiB for ext4/NTFS/ZFS. Password only; keyfiles are rejected.
- Encryption protects the virtual disk image; the selected inner filesystem is not separately encrypted. APFS remains the default.
- AES-256 is fixed. Reject Serpent/Twofish instead of silently ignoring the selected cipher.
- Mount uses `hdiutil attach -stdinpass`; any access restriction follows the actual storage/driver capability rather than a user-selectable mode. Unmount detaches the virtual disk without force.
- macOS VeraCrypt retains the existing cipher names, SHA-512, PIM 0, password-or-keyfile authentication and full initialization. Its default inner filesystem is exFAT; explicit ext4/NTFS/UFS2/ZFS use qualified host-native routes. XFS is refused pending driver persistence qualification. A legacy Btrfs VM route does not qualify as native support.

## Native APFS encrypted data volumes (macOS)

- Implemented and tested through native file and virtual-partition lifecycles, CLI prompts and the desktop helper. Packaged release and physical-hardware qualification remain pending.
- Explicit `apfs` uses `/usr/sbin/diskutil apfs` and its native encryption profile. Password only.
- File vaults use a plain GUID-partitioned disk image carrying one encrypted APFS volume. The password unlocks the APFS volume.
- New volumes are encrypted from creation with `addVolume -stdinpassphrase`. File creation stages and publishes through the existing atomic image workflow.
- Partition creation requires `--force` and an unmounted external data partition. Whole disks, internal disks, and existing APFS containers are refused.
- Unlock and mount are separate native operations. Close locks the volume before detaching its containing image.
- Qualification uses disposable disk images, including their virtual partition devices. Physical disks and in-place encryption conversion are unqualified.
