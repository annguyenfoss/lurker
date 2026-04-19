# lurker

`lurker` is a small Bash wrapper around `cryptsetup`, `mkfs.btrfs`, and `mount` for working with LUKS-backed containers and partitions.

It supports:

- File-backed LUKS containers
- Existing LUKS block devices such as `/dev/sda2` or `/dev/nvme0n1p1`
- Deterministic mapper names derived from the source path
- Optional tagged mapper names via `-t TAG`

## Requirements

Required commands:

- `cryptsetup`
- `mkfs.btrfs`
- `mount`
- `umount`
- `findmnt`
- `sha256sum`

Additional requirements:

- File-backed `create`: `dd`, `bc`
- Block-device `create`: `lsblk`
- Optional: `pv`, `blkid`, `readlink`

## Privileges

Some steps are unprivileged and some require root:

- Unprivileged: allocating a writable backing file and `luksFormat` on a writable regular file
- Privileged: `luksOpen`, `luksClose`, `mkfs.btrfs` on `/dev/mapper/...`, native `mount` / `umount`, and block-device `luksFormat`

`lurker` uses `sudo` for the privileged steps when it is not already running as root.

## Usage

```text
lurker create <luks-file> <size-gb> [-F|--force]
lurker create <block-device> [-F|--force]
lurker mount [-t TAG] <luks-source> <mountpoint>
lurker umount [-t TAG] <target>
```

Examples:

```bash
lurker create ./vault.img 4
lurker create /dev/sda2 --force
lurker mount ./vault.img /mnt/vault
lurker mount -t work /dev/sda2 /mnt/work
lurker umount /mnt/work
lurker umount -t work /dev/sda2
```

## Behavior Notes

- `mount` accepts either a regular file or a block device as the source.
- `umount` accepts any of these target forms:
  - a mountpoint
  - a `/dev/mapper/...` path
  - the original container file
  - the original block-device path
- `-t TAG` forces the mapper name to `/dev/mapper/lurker_<tag>` for source-path based `mount` and `umount`.
- If `umount` is given a mountpoint or an explicit `/dev/mapper/...` path, `lurker` resolves the active mapper from system state and ignores `-t`.
- Passphrase entry requires an interactive TTY.
- File systems positively detected as `btrfs` are mounted with `compress=zstd`. Other file systems are mounted without Btrfs-specific options.

## Safety Notes

- `create` on a block device is destructive and requires `--force`.
- `create` refuses to run on mounted block devices and active mapper devices.
- `create` on a file refuses to overwrite an existing regular file unless `--force` is used.

## Installation

Place `lurker` somewhere on your `PATH` and make it executable:

```bash
install -m 755 lurker /usr/local/bin/lurker
```

If you want non-root users to run the privileged parts through `sudo`, grant access to the required commands in `sudoers`.

## License

This project is licensed under the GNU General Public License v3.0 only. See [LICENSE](LICENSE).
