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

### OS-provided encryption

Lurker uses your OS's tools for the methods marked ✅.

| Platform | Encryption | System tools |
| :--- | :--- | :--- |
| Linux | LUKS1 and LUKS2 | `cryptsetup`, Btrfs tools, and disk and mount utilities from your distro |
| macOS | Apple encrypted disk images | `hdiutil`, included with macOS |
| FreeBSD | GELI | `geli` and GEOM, plus `mdconfig`, `newfs`, and mount utilities |
| OpenBSD | softraid CRYPTO and RAID1C | `bioctl` and softraid, plus `vnconfig`, `disklabel`, `newfs`, and mount utilities |
| NetBSD | CGD | `cgdconfig` and CGD, plus `vnconfig`, `newfs`, and mount utilities |
| DragonFly BSD | LUKS1 | `cryptsetup` and `dmsetup`, plus `vnconfig`, `kldload`, `newfs`, and mount utilities |

The BSD tools ship with the OS. On Linux, the native Lurker package declares
the required tools so your package manager can install any that are missing.

DragonFly's LUKS1 developer builds passed normal vault operations. Public release qualification still requires the base `cryptsetup` parser fix or equivalent reviewed protection. Abrupt power-cut tests also lost acknowledged writes. See the [validation results](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md) for the tested routes and recovery limits.

### Other encryption methods

Lurker packages bundle VeraCrypt, gocryptfs, CryFS and Cryptomator on the
platforms shown. FreeBSD PEFS needs the `pefs-kmod` package matching the running
kernel. On macOS, install FUSE-T separately for VeraCrypt and Cryptomator, and
macFUSE separately for gocryptfs and CryFS. The [macOS build guide](docs/macos-backends.md)
records the tested versions and package requirements.

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
