# lurker

<p align="center">
  <img src="extras/assets/lurker.png" alt="lurker logo" width="480">
</p>

Encryption made easy for Linux, with LUKS and VeraCrypt.

The active implementation now lives in [`cli/`](cli/). The original Bash version remains in [`bash-cli/`](bash-cli/) as the legacy reference.

## What You Can Do

```text
lurker create [--type TYPE] <file> <size-gb> [-F|--force]
lurker create [--type TYPE] <block-device> [-F|--force]
lurker mount [--type TYPE] [-t TAG] <source> <mountpoint>
lurker unmount [--type TYPE] [-t TAG] <target>
```

```bash
# LUKS file
lurker create ./vault.img 4
lurker mount ./vault.img /mnt/vault
lurker unmount /mnt/vault

# VeraCrypt file
lurker create --type veracrypt ./vault.hc 4
lurker mount ./vault.hc /mnt/vault
lurker unmount /mnt/vault

# Block device
lurker create /dev/sda2 --force
lurker create --type veracrypt /dev/sdb2 --force
lurker mount -t work /dev/sda2 /mnt/work
lurker unmount /mnt/work
```

## VeraCrypt

- `create` defaults to `luks`. Use `--type veracrypt` for VeraCrypt creation.
- `mount` and `unmount` auto-detect `luks` vs `veracrypt` unless `--type luks` or `--type veracrypt` is provided.
- VeraCrypt create is pinned to: `normal`, password-only, default `PIM`, no keyfiles, no hidden, `AES`, `SHA-512`, `--filesystem none`.
- After creating the VeraCrypt header, `lurker` opens it with `cryptsetup`, runs `mkfs.btrfs`, then closes it.
- Block-device VeraCrypt create always uses VeraCrypt quick format.
- VeraCrypt mount/unmount prefers `veracrypt -t`. If `veracrypt` is not installed, `lurker` falls back to `cryptsetup` and prints a notice to stdout.
- `-t TAG` is not yet supported for VeraCrypt containers, even when the `cryptsetup` fallback path is used.
- Hidden VeraCrypt volumes are not supported.

## Notes

- `--type auto` is not valid with `create`.
- `unmount` accepts a mountpoint, `/dev/mapper/...`, the original file, or the original block device. `umount` remains available as a compatibility alias.
- If `unmount` gets a block device without `-t`, `lurker` reverse-resolves the active `lurker_*` mapper from system state.
- `-t TAG` works for LUKS source-path mount/unmount.
- Btrfs volumes are mounted with `compress=zstd`.
- Passphrase entry requires an interactive TTY.
- `create` on a block device is destructive and requires `--force`.
- `create` refuses to run on mounted block devices and active mapper devices.
- `create` on a file refuses to overwrite an existing regular file unless `--force` is used.
- File-backed `create` uses same-directory temporary files and atomic rename on success.

## Requirements

- Required runtime: `cryptsetup`
- Create: `mkfs.btrfs`
- Mount: `mount`
- Unmount: `umount`
- Block-device create and filesystem probing: `lsblk`
- Optional filesystem probing: `blkid`
- VeraCrypt create and preferred native mount/umount: `veracrypt`

## Build

```bash
cargo build --release --manifest-path cli/Cargo.toml
```

## Install

```bash
install -m 755 cli/target/release/lurker /usr/local/bin/lurker
```

## License

This project is licensed under the GNU General Public License v3.0 only. See [LICENSE](LICENSE).
