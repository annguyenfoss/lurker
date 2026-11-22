# Lurker Cipher Directive

Use this file as the source of truth for selected encryption backends and create-time cipher policy.

## Approved Backend Scope

✅ implemented with packaged native CLI lifecycle coverage; 🟦 selected, native qualification incomplete; — outside approved scope. These are backend/format names, not cipher algorithms. A tick does not establish desktop, security-audit or complete fault/release qualification; exact tested revisions and limits are in `DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md`.

| Backend / format | macOS | Linux | FreeBSD | DragonFly BSD | NetBSD | OpenBSD |
| --- | --- | --- | --- | --- | --- | --- |
| Apple encrypted disk images | ✅ | — | — | — | — | — |
| LUKS2 | — | ✅ | — | — | — | — |
| VeraCrypt | ✅ | ✅ | ✅ | — | — | ✅ |
| CryFS stable 1.x | ✅ | ✅ | — | — | — | — |
| gocryptfs | ✅ | ✅ | ✅ | — | — | — |
| GELI | — | — | ✅ | — | — | — |
| CGD | — | — | — | — | ✅ | — |
| softraid CRYPTO | — | — | — | — | — | ✅ |
| softraid RAID1C | — | — | — | — | — | ✅ |
| PEFS | — | — | ✅ | — | — | — |
| Cryptomator | 🟦 | ✅ | — | — | — | — |
| LUKS1 | — | ✅ | — | ✅ | — | — |

- Do not add other backend/OS pairs without an explicit scope change. EncFS, securefs, fscrypt, OpenZFS, plain dm-crypt, Loop-AES, TCG Opal, commercial-only backends, encrypted archives, backups and rclone crypt are excluded. LUKS still uses dm-crypt internally. Research availability is not approval to integrate.
- age/rage is excluded: Lurker targets writable encrypted disks, filesystems and directories unlocked for use during a session. age/rage encrypts files/streams; rage-mount provides only read-only archive access, not writable directory encryption.
- Apple disk images are the existing hdiutil backend, not FileVault/native APFS-encryption management. VeraCrypt on NetBSD/DragonFly remains an exploratory port outside approved integration scope.
- macOS runtime policy: VeraCrypt uses FUSE-T 1.2.9 or later, without a macFUSE build or fallback. macFUSE is reserved for gocryptfs/CryFS. Cryptomator uses the FUSE-T FSKit provider in the tested package and passed the ordinary CLI lifecycle. Its rapid-close regression still needs qualification. Exact macOS package results are in `docs/macos-backends.md`.
- Planned support requires native validation: gocryptfs macOS is beta and FreeBSD is basic; VeraCrypt targets FreeBSD x86-64 14+ and OpenBSD x86-64 7.8+, subject to inner-filesystem support. CryFS 2.0 alpha is excluded.
- Prefer LUKS2 for new Linux volumes; LUKS1 adds compatibility and the DragonFly native path. Do not apply LUKS2/Argon2id format arguments to LUKS1. Qualify DragonFly cryptsetup versions, patches, profiles and disk/file-container behavior before release; selection is not security approval. PEFS needs a matching FreeBSD kernel module. Cryptomator requires create/mount/credential API and application-filesystem validation.
- Existing profiles below remain authoritative. Select and document profiles for new backends during implementation; do not apply LUKS/VeraCrypt cipher options to folder backends automatically.

## Create Profile Scope

- Applies to create flows only.
- Supported `--cipher` values: `aes`, `serpent`, `twofish` for LUKS/VeraCrypt. Apple images accept only `aes`.
- No cascades.
- No user-tunable low-level crypto knobs.
- Keep these profiles hardcoded in code.

## LUKS2 Profiles (Linux)

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
- NetBSD CGD: AES-XTS512, Argon2id v19 (262144 KiB, 3 iterations, parallelism 2), FFSv2 inside. Writable mounts require active NetBSD FFS journaling (WAPBL), which requests backing-device cache flushes at journal commits. Password only. The mandatory `<image>.cgdparams` sidecar contains salt/KDF/recovery parameters; back it up with the image. Only the strict generated profile is accepted; arbitrary executable `shell_cmd` parameters are rejected.
- OpenBSD softraid CRYPTO and RAID1C: native AES-XTS and password KDF with FFS inside. Both are password-only. Native keydisks differ from regular keyfiles. RAID1C creates a mirror of two independent regular files and publishes a `.raid1c.json` descriptor after both members. Explicit degraded recovery can unlock an existing array from one surviving member. Both formats refuse read-only crypto attachment.
- DragonFly LUKS1: AES/Serpent/Twofish XTS/plain64 with 512-bit keys, PBKDF2/SHA-256 and 5000 ms iteration time, native UFS inside. Accepts either a password or a binary keyfile. Its cryptsetup 1.1.2-derived implementation has different compatibility constraints from modern Linux; native functional tests do not replace security/maintenance qualification.
- DragonFly unlock validates the LUKS1 header and keyslot geometry for those profiles. It copies at most 16 MiB of metadata into the root-private session registry. The stock `dm_target_linear` module presents that metadata to cryptsetup while the payload stays in the original file. Existing images keep their ownership and on-disk layout.
- Native BSD disk adapters initially accept regular file containers only, through root-authorized native file devices. No physical-device creation or automatic filesystem repair.

## Directory create profiles

- gocryptfs 2.7: native AES-256-GCM contents, EME names and HKDF; explicit scrypt `N=2^16` for password wrapping.
- CryFS stable 1.0.3: AES-256-GCM, 16384-byte blocks and the stable upstream scrypt configuration. CryFS 2.x alpha is excluded.
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
- Read-only applies to both the decrypted FUSE image and the filesystem driver.
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

## Apple Disk Images (macOS)

- Create defaults to `apple` on macOS; Linux defaults to `luks`.
- `hdiutil create -type UDIF -fs APFS -volname Lurker -encryption AES-256 -stdinpass`.
- Size is specified in MiB. File containers only, minimum 32 MiB.
- Encryption protects the virtual disk image; APFS inside is not separately encrypted.
- AES-256 is fixed. Reject Serpent/Twofish instead of silently ignoring the selected cipher.
- Mount uses `hdiutil attach -stdinpass`, optionally `-readonly`; unmount detaches the virtual disk without force.
- macOS VeraCrypt retains the existing cipher names, SHA-512, PIM 0, password-or-keyfile authentication, and full initialization; it changes only the inner filesystem to exFAT.
