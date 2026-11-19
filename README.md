<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, macOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories. A simple CLI across six operating systems, with a desktop app for Linux and macOS.

## Encryption support

✅ Supported · — Not supported. CLI support shown.

| Encryption | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| LUKS2 | ✅ | — | — | — | — | — |
| LUKS1 | ✅ | — | — | — | — | ✅ |
| VeraCrypt | ✅ | — | ✅ | ✅ | — | — |
| Apple encrypted disk images | — | ✅ | — | — | — | — |
| GELI | — | — | ✅ | — | — | — |
| CGD | — | — | — | — | ✅ | — |
| softraid CRYPTO | — | — | — | ✅ | — | — |
| softraid RAID1C | — | — | — | ✅ | — | — |
| gocryptfs | ✅ | — | ✅ | — | — | — |
| CryFS | ✅ | — | — | — | — | — |
| PEFS | — | — | ✅ | — | — | — |
| Cryptomator | ✅ | — | — | — | — | — |

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
