# Lurker Cipher Directive

Use this file as the source of truth for create-time cipher policy.

## Scope

- Applies to create flows only.
- Supported `--cipher` values: `aes`, `serpent`, `twofish`.
- No cascades.
- No user-tunable low-level crypto knobs.
- Keep these profiles hardcoded in code.

## LUKS Profiles

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
  - `--filesystem=none`
  - `--random-source=/dev/urandom`
  - `--pim=0`
  - `-k ""`
- If a passphrase is supplied non-interactively, also add:
  - `--stdin`
  - `--non-interactive`
- File-backed create adds:
  - `--size=<MiB>M`
- Block-device create adds:
  - `--size=max`
  - `--quick`

## Opinionated Intent

- `aes` is the default.
- `serpent` is the heavier/slower paranoid option.
- `twofish` is the non-AES alternative with explicit open-time perf tuning.
- The user chooses only the cipher family. The rest of the crypto profile stays fixed.

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
