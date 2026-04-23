# lurker

<p align="center">
  <img src="extras/assets/lurker.png" alt="lurker logo" width="320">
</p>

Encryption made easy for Linux, with a shared Rust core, a CLI, and a Slint desktop app.

## Repository Layout

```text
lurker/
├── crates/
│   ├── lurker-core/    # shared Rust domain + Linux integration
│   ├── lurker-cli/     # `lurker` terminal binary
│   └── lurker-helper/  # internal privileged helper used by the desktop app
├── apps/
│   └── lurker-desktop/ # Slint Linux desktop app
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
lurker mount [--type TYPE] [-t TAG] <source> <mountpoint>
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

Examples:

```bash
./target/release/lurker create ./vault.img 4
./target/release/lurker create --cipher serpent ./vault-serpent.img 4
./target/release/lurker create --cipher twofish /dev/sdb1 --force
./target/release/lurker create --type veracrypt --cipher serpent ./vault.hc 4
./target/release/lurker createvc --cipher twofish /dev/sdc1 --force
./target/release/lurker mount ./vault.img /mnt/vault
./target/release/lurker unmount /mnt/vault
```

### Linux Desktop App

The desktop app is intentionally simple for now:

- one main window
- create / mount / unmount tabs
- active `lurker_*` volume list
- inline status and errors only

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

## Runtime Requirements

- `cryptsetup`
- `mkfs.btrfs`
- `mount`
- `umount`
- `lsblk`
- `pkexec` for preferred desktop privilege escalation
- `sudo` for desktop fallback when launched from a terminal
- optional `blkid`
- optional `veracrypt`

## Notes

- `create` defaults to `luks`. Use `--type veracrypt` or `createvc` for VeraCrypt creation.
- `create` defaults to `--cipher aes`. The only supported single-cipher create profiles are `aes`, `serpent`, and `twofish`.
- The create profiles are intentionally hardcoded and opinionated. They are defined in the code, not in an external config file.
- AES uses the updated LUKS2 + Argon2id profile from `CIPHER.md`.
- Serpent uses the heavier Argon2id profile from `CIPHER.md`.
- Twofish uses the `CIPHER.md` sector-size tuning and automatically applies the documented open-time perf flags when a Twofish LUKS header is detected.
- VeraCrypt create supports the same three single-cipher choices and keeps `SHA-512` fixed.
- `mount` and `unmount` auto-detect `luks` vs `veracrypt` unless `--type` is pinned.
- `-t TAG` is supported for LUKS source-path mount/unmount.
- VeraCrypt tags are still unsupported.
- Btrfs volumes are mounted with `compress=zstd`.
- File-backed create uses same-directory temporary files and atomic rename.
- The desktop app uses passphrase entry inside the app. Create passphrase confirmation is enforced by the UI.
- The desktop app does not call the CLI binary. It talks to `lurker-helper` over a local stdin/stdout JSON contract.
- The CLI still supports interactive terminal prompting.
- The archived Bash implementation in `legacy/bash-cli/` was updated to match the same three create cipher profiles.

## Verification

Current verification target:

```bash
cargo check --workspace
cargo test --workspace
```

## License

This project is licensed under the GNU General Public License v3.0 only. See [LICENSE](LICENSE).
