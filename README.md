<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, macOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories with the CLI or native desktop app. Linux also supports creating vaults on physical devices.

## Encryption support

- ✅ Available in the CLI with OS-provided encryption tools
- 📦 Available in the CLI with software outside the base OS
- — Unavailable · † Limited power-loss resilience

| Encryption | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| LUKS1 | ✅ | — | — | — | — | ✅† |
| LUKS2 | ✅ | — | — | — | — | — |
| Apple encrypted disk images | — | ✅ | — | — | — | — |
| GELI | — | — | ✅ | — | — | — |
| CGD | — | — | — | — | ✅ | — |
| softraid CRYPTO | — | — | — | ✅ | — | — |
| softraid RAID1C | — | — | — | ✅ | — | — |
| VeraCrypt | 📦 | 📦 | 📦 | 📦 | — | — |
| gocryptfs | 📦 | 📦 | 📦 | — | — | — |
| CryFS | 📦 | 📦 | — | — | — | — |
| PEFS | — | — | 📦 | — | — | — |
| Cryptomator | 📦 | 📦 | — | — | — | — |

† [DragonFly LUKS1 qualification](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md)

### OS-provided encryption

Lurker wraps the platform's tools for these methods.

- Linux — LUKS1 and LUKS2
- macOS — Apple encrypted disk images
- FreeBSD — GELI
- OpenBSD — softraid CRYPTO and RAID1C
- NetBSD — CGD
- DragonFly BSD — LUKS1

### Lurker-bundled encryption

- VeraCrypt — Linux, macOS, FreeBSD, OpenBSD
- gocryptfs — Linux, macOS, FreeBSD
- CryFS — Linux, macOS
- Cryptomator — Linux, macOS

## Encryption dependencies

Install the host packages for the backends you use. System tools ship with the OS. Linux package names follow Arch Linux.

| OS | Host packages | System tools | Bundled runtimes |
| :--- | :--- | :--- | :--- |
| Linux | `btrfs-progs`<br>`cryptsetup`<br>`fuse2`<br>`fuse3`<br>`util-linux` | Kernel Btrfs, device mapper, FUSE, loop devices | Java for Cryptomator |
| macOS | `FUSE-T>=1.2.9`<br>`macFUSE` | `hdiutil`, `diskutil`, FSKit | Java for Cryptomator |
| FreeBSD | `pefs-kmod` | GEOM, `geli`, `mdconfig`, `newfs`, `mount`, `umount`, `fusefs`, `mount_fusefs` | FUSE library for VeraCrypt |
| OpenBSD | — | softraid, `bioctl`, `vnconfig`, `disklabel`, `newfs`, `mount`, `umount`, FUSE | — |
| NetBSD | — | CGD, `cgdconfig`, `vnconfig`, `newfs`, `mount`, `umount` | — |
| DragonFly BSD | — | `cryptsetup`, `dmsetup`, `dm_target_crypt`, `dm_target_linear`, `vnconfig`, `kldload`, `newfs`, `mount`, `umount` | — |

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
