<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, macOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories with the CLI or native desktop app. Linux also supports creating vaults on physical devices.

## Encryption support

✅ Available in the CLI · — Unavailable · † Developer builds pending release qualification.

| Encryption | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| LUKS2 | ✅ | — | — | — | — | — |
| LUKS1 | ✅ | — | — | — | — | ✅† |
| VeraCrypt | ✅ | ✅† | ✅ | ✅ | — | — |
| Apple encrypted disk images | — | ✅ | — | — | — | — |
| GELI | — | — | ✅ | — | — | — |
| CGD | — | — | — | — | ✅ | — |
| softraid CRYPTO | — | — | — | ✅ | — | — |
| softraid RAID1C | — | — | — | ✅ | — | — |
| gocryptfs | ✅ | ✅† | ✅ | — | — | — |
| CryFS | ✅ | ✅† | — | — | — | — |
| PEFS | — | — | ✅ | — | — | — |
| Cryptomator | ✅ | ✅† | — | — | — | — |

The macOS developer package includes VeraCrypt, gocryptfs, CryFS and
Cryptomator. Install FUSE-T separately for VeraCrypt and Cryptomator, and
macFUSE separately for gocryptfs and CryFS. The [macOS build guide](docs/macos-backends.md)
records the tested versions and package requirements.

DragonFly's LUKS1 developer builds passed normal vault operations. Public release qualification still requires the base `cryptsetup` parser fix or equivalent reviewed protection. Abrupt power-cut tests also lost acknowledged writes. See the [validation results](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md) for the tested routes and recovery limits.

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
