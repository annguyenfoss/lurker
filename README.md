# lurker

<p align="center">
  <img src="extras/assets/lurker.png" alt="lurker logo" width="320">
</p>

Encrypted file containers made easy for Linux and macOS, with a shared Rust core, a CLI, and a Slint desktop app.

## Repository Layout

```text
lurker/
├── crates/
│   ├── lurker-core/    # shared Rust domain + Linux/macOS integration
│   ├── lurker-cli/     # `lurker` terminal binary
│   └── lurker-helper/  # internal privileged helper used by the desktop app
├── apps/
│   └── lurker-desktop/ # Slint desktop app
├── legacy/
│   ├── bash-cli/               # archived Bash implementation
│   └── rust-cli-monolith/      # archived single-crate Rust layout
└── extras/
```

## Current Products

### CLI

```text
lurker create [--type TYPE] [--cipher CIPHER] <file> <size-gb> [-F|--force]
lurker create [--type TYPE] [--cipher CIPHER] <block-device> [-F|--force]
lurker mount [--type TYPE] [--readonly] [-t TAG] <source> <mountpoint>
lurker unmount [--type TYPE] [-t TAG] <target>
```

Build it:

```bash
cargo build --release -p lurker-cli
```

Run it:

```bash
./target/release/lurker help
```

Install it:

```bash
install -m 755 ./target/release/lurker /usr/local/bin/lurker
```

Linux examples:

```bash
./target/release/lurker create ./vault.img 4
./target/release/lurker create --cipher serpent ./vault-serpent.img 4
./target/release/lurker create --cipher twofish /dev/sdb1 --force
./target/release/lurker create --type veracrypt --cipher serpent ./vault.hc 4
./target/release/lurker createvc --cipher twofish /dev/sdc1 --force
./target/release/lurker mount ./vault.img /mnt/vault
./target/release/lurker unmount /mnt/vault
```

### Desktop App

The desktop app is intentionally simple for now:

- one main window
- create / mount / unmount tabs
- active `lurker_*` volume list
- inline status and errors only
- built-in zoom controls for larger displays

It uses:

- `lurker-core` for all shared logic
- `lurker-helper` for privileged desktop operations
- Slint + winit + software renderer for the GUI shell

Run the desktop app in development:

```bash
cargo build -p lurker-helper
cargo run -p lurker-desktop
```

Build the desktop app:

```bash
cargo build --release -p lurker-helper -p lurker-desktop
```

Install the desktop binaries:

```bash
install -m 755 ./target/release/lurker-desktop /usr/local/bin/lurker-desktop
install -m 755 ./target/release/lurker-helper /usr/local/bin/lurker-helper
```

For system packaging, keep `lurker-desktop` and `lurker-helper` together.

## Linux Runtime Requirements

- `cryptsetup`
- `mkfs.btrfs`
- `mount`
- `umount`
- `lsblk`
- `pkexec` for preferred desktop privilege escalation
- `sudo` for desktop fallback when launched from a terminal
- optional `blkid`
- optional `veracrypt` (required for VeraCrypt creation; Linux mounting and unmounting use cryptsetup)

## macOS Drag-and-Drop Installer

Build a native release DMG on the Mac where you want to install it:

```bash
# One-time build tooling setup (not needed by people installing the app):
python3 -m venv build/macos-dmg-tools
build/macos-dmg-tools/bin/python -m pip install -r scripts/requirements-macos-dmg.txt
python3 scripts/build-macos-dmg.py
```

Open `dist/Lurker-0.1.0-macos-arm64.dmg` on Apple Silicon (or the `x86_64` DMG when built on Intel), then drag **Lurker.app** onto **Applications** and eject the image. The app bundles its helper, fonts, icons, and other UI resources. No `.pkg`, terminal installation, or separately installed Lurker CLI is needed.

The installer opens an 800-point-wide Finder window with an illustrated Retina background, fixed app/Applications icon positions, and hidden supporting files. Its saved Finder settings are authored directly, so building does not require Finder automation or changing system preferences.

The script builds the architecture of the current Rust toolchain, derives the minimum macOS version from the binaries, verifies the bundle signature and disk-image checksum, and writes a `.sha256` checksum next to the DMG. Apple encryption uses built-in system tools; VeraCrypt and its matching FUSE runtime remain separate optional dependencies.

Local builds are ad-hoc signed and are not Apple-notarized. Distribution signing can use `CODESIGN_IDENTITY`, but notarization is a separate release step requiring a Developer ID distribution certificate and Apple credentials. See https://developer.apple.com/developer-id/.

## macOS Containers

Both the CLI and desktop app support:

- **Apple Encryption** (default): an AES-256 encrypted, read/write `.dmg` virtual disk with APFS inside. Encryption protects the disk image, not individual files or the host filesystem. This uses Apple DiskImages encryption, not system FileVault.
- **VeraCrypt**: a normal file container with AES, Serpent, or Twofish, SHA-512, and an exFAT filesystem inside.

Apple encryption uses the built-in `/usr/bin/hdiutil`; VeraCrypt unmount also uses `/usr/sbin/diskutil` to reject busy filesystems. For VeraCrypt, install VeraCrypt for macOS together with the matching FUSE-T or macFUSE runtime required by that build. Lurker discovers `/Applications/VeraCrypt.app/Contents/MacOS/VeraCrypt` even when started from Finder.

Build and install the same binaries as above. Keep `lurker-helper` next to `lurker-desktop`. On macOS the helper runs in the login user session; Linux pkexec/sudo escalation is not used.

```bash
lurker create ./vault.dmg 4
lurker create --type apple ./apple-vault.dmg 4
lurker create --type veracrypt --cipher serpent ./vault.hc 4
mkdir -p "$HOME/vault-mount"
lurker mount ./vault.dmg "$HOME/vault-mount"
lurker unmount "$HOME/vault-mount"
```

macOS create supports regular files only (minimum 32 MiB). Mount points must be empty; the final directory is created by the native tool if it is missing and its parent exists. It refuses physical disks, partitions, symlink targets, and overwriting an attached container. `--force` can replace an existing unmounted regular file only after creation succeeds. Apple images accept only `--cipher aes`, which selects AES-256. Passphrases are passed through stdin, never command arguments; spaces are preserved and embedded NUL/newline characters are rejected.

The desktop offers Apple Encryption and VeraCrypt on macOS, uses the same create/mount/unmount flows, and supports read-only mounts. Its active list includes the current user's encrypted Apple disk images and VeraCrypt volumes, including containers mounted outside Lurker. Unmount accepts the container file, mountpoint, or its virtual device and detaches that one container. Tags remain Linux LUKS-only.

Existing Linux LUKS containers require Linux. Existing Linux VeraCrypt containers created by Lurker contain Btrfs, which macOS cannot mount natively. New macOS VeraCrypt containers use exFAT; Linux can read them with VeraCrypt. Apple disk images require macOS.

Native tool limitations are returned as errors and never justify replacing a previous container. During native testing, macOS rejected creating an encrypted APFS image directly on a mounted exFAT disk-image host; mounting an existing encrypted image from that host succeeded. Create on a supported host filesystem and copy the closed container if this native error occurs.

Apple disk-image background: https://support.apple.com/guide/disk-utility/create-a-disk-image-dskutl11888/mac

VeraCrypt command reference: https://veracrypt.io/en/Command%20Line%20Usage%20for%20Unix.html

## Notes

- `create` defaults to `luks` on Linux and `apple` on macOS. Use `--type veracrypt` or `createvc` for VeraCrypt creation.
- `create` defaults to `--cipher aes`. The only supported single-cipher create profiles are `aes`, `serpent`, and `twofish`.
- The create profiles are intentionally hardcoded and opinionated. They are defined in the code, not in an external config file.
- AES uses the updated LUKS2 + Argon2id profile from `CIPHER.md`.
- Serpent uses the heavier Argon2id profile from `CIPHER.md`.
- Twofish uses the `CIPHER.md` sector-size tuning and automatically applies the documented open-time perf flags when a Twofish LUKS header is detected.
- VeraCrypt create supports the same three single-cipher choices and keeps `SHA-512` fixed. Device creation uses the full device size detected by VeraCrypt, with quick format.
- On Linux, `mount` recognizes LUKS metadata first; other sources are authenticated as VeraCrypt when opening. Encrypted VeraCrypt headers cannot be identified without the passphrase. `unmount` reads the active mapper type. Use `--type` to pin the format.
- Linux VeraCrypt mounts use `cryptsetup --type tcrypt --veracrypt`, with the same `lurker_*` mapper naming, active-volume listing, filesystem handling, and cleanup as LUKS.
- `mount --readonly` opens the mapper and mounts the filesystem read-only. The desktop readonly checkbox does the same.
- VeraCrypt supports normal password-only volumes with the default PIM. Keyfiles, hidden-volume controls, and custom PIM are not exposed.
- VeraCrypt passphrases supplied through the helper cannot be empty or contain NUL or line breaks. Leading/trailing spaces and Unicode are preserved.
- `-t TAG` is supported for LUKS source-path mount/unmount.
- VeraCrypt tags are still unsupported.
- Btrfs volumes are mounted with `compress=zstd`.
- File-backed create uses same-directory temporary files, atomic publication without overwriting unless forced, and atomic replacement when forced.
- The desktop app uses passphrase entry inside the app. Create passphrase confirmation is enforced by the UI. The Generate button uses system randomness (128 bits).
- The desktop app does not call the CLI binary. It talks to `lurker-helper` over a local stdin/stdout JSON contract.
- The CLI supports interactive terminal prompting; macOS create also confirms the passphrase. Passphrases are passed to crypto tools through terminal input or stdin, never command-line arguments.
- New Btrfs filesystems initially have root-owned directories. After mounting a new personal volume, set its ownership once with `sudo chown "$(id -u):$(id -g)" /your/mountpoint` if needed.
- The archived Bash implementation in `legacy/bash-cli/` was updated to match the same three create cipher profiles.

## Verification

Native tests use only disposable file containers. The installed FUSE-T runtime can leave VeraCrypt service processes alive after dismount during repeated runs; native fixtures clean up only services naming their unique test files. Production operations return native runtime errors without killing unrelated processes.

Current verification target:

```bash
cargo check --workspace --locked
cargo test --workspace --locked
# macOS: native create/mount/read-only/remount/error tests using disposable files
cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1
cargo test -p lurker-helper --test macos_native -- --include-ignored --test-threads=1
cargo build -p lurker-cli
python3 tests/macos_cli.py
```

Real Linux tests create fresh `/tmp` images and dedicated loop devices. They never target an existing vault. See `docs/linux-validation.md` for the full matrix and commands.

## Local Arch/AUR Package

`packaging/aur/PKGBUILD` builds the CLI, desktop app, and internal helper together. The Git recipe targets https://github.com/annguyenfoss/lurker.git and needs the changes upstream before publication.

Build the current worktree locally, including uncommitted changes:

```bash
./scripts/build-local-aur.sh
```

The package, `.SRCINFO`, source snapshot, and logs are written under a fresh `dist/aur-build-*` directory. Nothing is uploaded or installed. Building requires the dependencies listed in the PKGBUILD; VeraCrypt remains optional for mounting existing containers.

## License

This project is licensed under the GNU General Public License v3.0 only. See [LICENSE](LICENSE).
