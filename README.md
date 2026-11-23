<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, macOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories with the CLI or native desktop app. Linux also supports creating vaults on physical devices.

## Encryption support

- 🍬 CLI wrapper for a built-in tool or an installed dependency
- 📦 CLI support with an encryption engine bundled by Lurker
- — Unavailable · † Limited power-loss resilience

| Encryption | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly&nbsp;BSD |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| LUKS1 | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — | — | — | — | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20">† |
| LUKS2 | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — | — | — | — | — |
| Apple encrypted disk images | — | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — | — | — | — |
| GELI | — | — | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — | — | — |
| PEFS | — | — | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — | — | — |
| softraid CRYPTO | — | — | — | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — | — |
| softraid RAID1C | — | — | — | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — | — |
| CGD | — | — | — | — | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f36c.png" alt="Wrapper" width="20" height="20"> | — |
| VeraCrypt | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | — | — |
| gocryptfs | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | — | — | — |
| CryFS | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | — | — | — | — |
| Cryptomator | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | <img src="https://github.githubassets.com/images/icons/emoji/unicode/1f4e6.png" alt="Bundled engine" width="20" height="20"> | — | — | — | — |

† [DragonFly LUKS1 qualification](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md)

### Encryption wrappers

Lurker uses built-in tools or installed dependencies for these methods.

- Linux — LUKS1 and LUKS2
- macOS — Apple encrypted disk images
- FreeBSD — GELI and PEFS (requires `pefs-kmod`)
- OpenBSD — softraid CRYPTO and RAID1C
- NetBSD — CGD
- DragonFly BSD — LUKS1

### Lurker-bundled encryption

- VeraCrypt — Linux, macOS, FreeBSD, OpenBSD
- gocryptfs — Linux, macOS, FreeBSD
- CryFS — Linux, macOS
- Cryptomator — Linux, macOS

## Encryption dependencies

Install the dependencies for the backends you use. Built-in tools are the executables Lurker uses from the base OS. Linux package names follow Arch Linux.

<table width="100%">
  <thead>
    <tr>
      <th align="left" width="22%">OS</th>
      <th align="left" width="26%">Dependencies</th>
      <th align="left" width="26%">Built-in tools</th>
      <th align="left" width="26%">Bundled runtimes</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>Linux</td>
      <td><code>btrfs-progs</code><br><code>cryptsetup</code><br><code>fuse2</code><br><code>fuse3</code><br><code>util-linux</code></td>
      <td></td>
      <td>Java for Cryptomator</td>
    </tr>
    <tr>
      <td>macOS</td>
      <td><code>FUSE-&#8288;T&gt;=1.2.9</code><br><code>macFUSE</code></td>
      <td><code>hdiutil</code>, <code>diskutil</code></td>
      <td>Java for Cryptomator</td>
    </tr>
    <tr>
      <td>FreeBSD</td>
      <td><code>pefs-kmod</code></td>
      <td><code>geli</code>, <code>geom</code>, <code>mdconfig</code>, <code>newfs</code>, <code>mount</code>, <code>umount</code>, <code>mount_fusefs</code>, <code>sysctl</code></td>
      <td>FUSE library for VeraCrypt</td>
    </tr>
    <tr>
      <td>OpenBSD</td>
      <td>—</td>
      <td><code>bioctl</code>, <code>vnconfig</code>, <code>disklabel</code>, <code>newfs</code>, <code>mount</code>, <code>umount</code>, <code>sysctl</code></td>
      <td>—</td>
    </tr>
    <tr>
      <td>NetBSD</td>
      <td>—</td>
      <td><code>cgdconfig</code>, <code>vndconfig</code>, <code>newfs</code>, <code>mount</code>, <code>umount</code></td>
      <td>—</td>
    </tr>
    <tr>
      <td>DragonFly&nbsp;BSD</td>
      <td>—</td>
      <td><code>cryptsetup</code>, <code>dmsetup</code>, <code>vnconfig</code>, <code>kldload</code>, <code>newfs</code>, <code>mount</code>, <code>umount</code></td>
      <td>—</td>
    </tr>
  </tbody>
</table>

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
