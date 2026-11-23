<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, macOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories with the CLI or native desktop app. Linux also supports creating vaults on physical devices.

## Encryption dependencies

Host packages depend on the backends you use. Linux package names follow Arch Linux.

| OS | Host packages | Bundled components |
| :--- | :--- | :--- |
| Linux | `btrfs-progs`<br>`cryptsetup`<br>`fuse2`<br>`fuse3`<br>`util-linux` | VeraCrypt, gocryptfs, CryFS, and Cryptomator with its Java runtime. |
| macOS | `FUSE-T>=1.2.9`<br>`macFUSE` | VeraCrypt, gocryptfs, CryFS, and Cryptomator with its Java runtime. |
| FreeBSD | `pefs-kmod` (matching kernel) | VeraCrypt with its FUSE library, and gocryptfs. |
| OpenBSD | — | VeraCrypt. |
| NetBSD | — | — |
| DragonFly BSD | — | — |

DragonFly LUKS1 has limited power-loss resilience. See the [qualification results](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md).

See [Linux and BSD packaging](docs/unix-packaging.md) and [macOS packaging](docs/macos-backends.md) for package variants, desktop dependencies, and setup.

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
