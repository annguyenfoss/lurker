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

Some encryption methods ship with the OS. Lurker wraps the tools below.

| Platform | Encryption | System tools |
| :--- | :--- | :--- |
| Linux | LUKS1 and LUKS2 | `cryptsetup`, Btrfs tools, and disk and mount utilities from your distro |
| macOS | Apple encrypted disk images | `hdiutil`, included with macOS |
| FreeBSD | GELI | `geli` and GEOM, plus `mdconfig`, `newfs`, and mount utilities |
| OpenBSD | softraid CRYPTO and RAID1C | `bioctl` and softraid, plus `vnconfig`, `disklabel`, `newfs`, and mount utilities |
| NetBSD | CGD | `cgdconfig` and CGD, plus `vnconfig`, `newfs`, and mount utilities |
| DragonFly BSD | LUKS1 | `cryptsetup` and `dmsetup`, plus `vnconfig`, `kldload`, `newfs`, and mount utilities |

† [DragonFly LUKS1 qualification](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md)

### Lurker-bundled encryption

For methods marked 📦, the goal is one Lurker install with the engine and runtime included.

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
