# lurker

![lurker logo](lurker.png)

Encryption made easy for Linux, with LUKS and VeraCrypt.

macOS support (FileVault, VeraCrypt) coming soon.

## What You Can Do

```text
lurker create [--type TYPE] <file> <size-gb> [-F|--force]
lurker create [--type TYPE] <block-device> [-F|--force]
lurker mount [--type TYPE] [-t TAG] <source> <mountpoint>
lurker umount [--type TYPE] [-t TAG] <target>
```

```bash
# LUKS file
lurker create ./vault.img 4
lurker mount ./vault.img /mnt/vault
lurker umount /mnt/vault

# VeraCrypt file
lurker create --type veracrypt ./vault.hc 4
lurker mount ./vault.hc /mnt/vault
lurker umount /mnt/vault

# Block device
lurker create /dev/sda2 --force
lurker create --type veracrypt /dev/sdb2 --force
lurker mount -t work /dev/sda2 /mnt/work
lurker umount /mnt/work
```

## VeraCrypt

- `create` defaults to `luks`. Use `--type veracrypt` for VeraCrypt creation.
- `mount` and `umount` auto-detect `luks` vs `veracrypt` unless `--type luks` or `--type veracrypt` is provided.
- VeraCrypt create is pinned to: `normal`, password-only, default `PIM`, no keyfiles, no hidden, `AES`, `SHA-512`, `--filesystem none`.
- After creating the VeraCrypt header, `lurker` opens it with `cryptsetup`, runs `mkfs.btrfs`, then closes it.
- Block-device VeraCrypt create always uses VeraCrypt quick format.
- VeraCrypt mount/umount prefers `veracrypt -t`. If `veracrypt` is not installed, `lurker` falls back to `cryptsetup` and prints a notice to stdout.
- `-t TAG` is not yet supported for VeraCrypt containers, even when the `cryptsetup` fallback path is used.
- Hidden VeraCrypt volumes are not supported.

## Notes

- `--type auto` is not valid with `create`.
- `umount` accepts a mountpoint, `/dev/mapper/...`, the original file, or the original block device.
- If `umount` gets a block device without `-t`, `lurker` reverse-resolves the active `lurker_*` mapper from system state.
- `-t TAG` works for LUKS source-path mount/umount.
- Btrfs volumes are mounted with `compress=zstd`.
- Passphrase entry requires an interactive TTY.
- `create` on a block device is destructive and requires `--force`.
- `create` refuses to run on mounted block devices and active mapper devices.
- `create` on a file refuses to overwrite an existing regular file unless `--force` is used.

## Requirements

- Required: `cryptsetup`, `mkfs.btrfs`, `mount`, `umount`, `findmnt`, `sha256sum`
- File create: `bc`
- LUKS file create: `dd`
- Block-device create: `lsblk`
- VeraCrypt create and preferred native mount/umount: `veracrypt`

## Install

```bash
install -m 755 lurker /usr/local/bin/lurker
```

## License

This project is licensed under the GNU General Public License v3.0 only. See [LICENSE](LICENSE).
