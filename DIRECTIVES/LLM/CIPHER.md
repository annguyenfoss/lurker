# Lurker Cipher Directive

Use this file as the source of truth for selected encryption backends and create-time cipher policy.

## Approved Backend Scope

✅ integrated; 🟦 selected for future integration; — outside approved scope. Backend/format names are distinct from cipher algorithms. This matrix records scope, not implementation of planned features.

| Backend / format | macOS | Linux | FreeBSD | DragonFly BSD | NetBSD | OpenBSD |
| --- | --- | --- | --- | --- | --- | --- |
| Apple encrypted disk images | ✅ | — | — | — | — | — |
| LUKS2 | — | ✅ | — | — | — | — |
| VeraCrypt | ✅ | ✅ | 🟦 | — | — | 🟦 |
| CryFS stable 1.x | — | 🟦 | — | — | — | — |
| gocryptfs | 🟦 | 🟦 | 🟦 | — | — | — |

- Do not add other backend/OS pairs without an explicit scope change. EncFS, encrypted archives, backups and rclone crypt are excluded. Research availability is not approval to integrate.
- age/rage is excluded: Lurker targets writable encrypted disks, filesystems and directories unlocked for use during a session. age/rage encrypts files/streams; rage-mount provides only read-only archive access, not writable directory encryption.
- Apple disk images are the existing hdiutil backend, not FileVault/native APFS-encryption management. VeraCrypt on NetBSD/DragonFly remains an exploratory port outside approved integration scope.
- Planned support requires native validation: gocryptfs macOS is beta and FreeBSD is basic; VeraCrypt targets FreeBSD x86-64 14+ and OpenBSD x86-64 7.8+, subject to inner-filesystem support. CryFS 2.0 alpha is excluded.
- Existing profiles below remain authoritative. Select and document profiles for new backends during implementation; do not apply LUKS/VeraCrypt cipher options to folder backends automatically.

## Create Profile Scope

- Applies to create flows only.
- Supported `--cipher` values: `aes`, `serpent`, `twofish` for LUKS/VeraCrypt. Apple images accept only `aes`.
- No cascades.
- No user-tunable low-level crypto knobs.
- Keep these profiles hardcoded in code.

## LUKS Profiles (Linux)

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
- The archived Bash implementation retains the same crypto profiles but only implements the privileged kernel route.

## Sync Rules

When changing any create profile, update all of:

- `crates/lurker-core/src/workflow.rs`
- `legacy/bash-cli/lurker`
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
- Archived Bash is Linux-only and does not implement Apple or macOS workflows.
