<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, macOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories with the CLI or native desktop app. Linux also supports creating vaults on physical devices.

## Encryption support and runtime requirements

Host requirements depend on the encryption method you use. Lurker packages include the bundled components for the backends selected at packaging time.

| OS | Supported encryption in the CLI | Host runtime requirements | Bundled components |
| :--- | :--- | :--- | :--- |
| Linux | LUKS1, LUKS2, VeraCrypt, gocryptfs, CryFS, Cryptomator | Btrfs tools for LUKS1, LUKS2, and VeraCrypt.<br>`cryptsetup` plus disk and mount utilities for LUKS1, LUKS2, and administrator-mode VeraCrypt.<br>FUSE 3 library and `fusermount3` for login-user VeraCrypt and Cryptomator.<br>`fusermount` or `fusermount3` for gocryptfs.<br>FUSE 2 library and `fusermount` for CryFS. | VeraCrypt, gocryptfs, CryFS, and Cryptomator with its Java runtime. |
| macOS | Apple encrypted disk images, VeraCrypt, gocryptfs, CryFS, Cryptomator | Built-in `hdiutil` for Apple disk images and `diskutil` for VeraCrypt.<br>Install FUSE-T 1.2.9 or later for VeraCrypt and enable its FSKit extension for Cryptomator.<br>Install macFUSE for gocryptfs and CryFS. | VeraCrypt, gocryptfs, CryFS, and Cryptomator with its Java runtime. |
| FreeBSD | GELI, VeraCrypt, gocryptfs, PEFS | `geli` and GEOM, plus `mdconfig`, `newfs`, and mount utilities for GELI.<br>OS-provided `fusefs` for VeraCrypt and gocryptfs. VeraCrypt uses disk tools. gocryptfs uses `mount_fusefs` and needs `/dev/fuse` access and user mounts enabled.<br>`pefs-kmod` for PEFS. It supplies the `pefs` tool and a module that must match the running kernel. | VeraCrypt with its FUSE library, and gocryptfs. |
| OpenBSD | softraid CRYPTO, softraid RAID1C, VeraCrypt | `bioctl` and softraid, plus `vnconfig`, `disklabel`, `newfs`, and mount utilities for CRYPTO and RAID1C.<br>OS-provided FUSE and disk tools for VeraCrypt. | VeraCrypt. |
| NetBSD | CGD | `cgdconfig` and CGD, plus `vnconfig`, `newfs`, and mount utilities. | — |
| DragonFly BSD | LUKS1† | `cryptsetup` and `dmsetup`, plus `vnconfig`, `kldload`, `newfs`, and mount utilities. | — |

† DragonFly LUKS1 has limited power-loss resilience. See the [qualification results](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md).

See [Linux and BSD packaging](docs/unix-packaging.md) for shared system and desktop dependencies.

## Create. Unlock. Lock.

```bash
lurker create vault.img 1
mkdir -p vault
lurker mount vault.img vault
lurker unmount vault
```

Lurker chooses the native format for your OS. Use administrator authorization where required.

## Get started

[Build & usage](docs/usage.md) · [Linux & BSD packaging](docs/unix-packaging.md) · [macOS packaging](docs/macos-backends.md) · [Validation results](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md)

[GPL-3.0-only](LICENSE)
