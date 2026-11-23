<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, MacOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories with the CLI or native desktop app.

## Encryption support

- 🧅 Wraps an encryption tool on your system
- 🥑 Encryption engine included with Lurker
- 🍰 Planned integration (“The cake is a lie”)
- — Unavailable
- 🛈 See note below

| Encryption | Linux | MacOS | FreeBSD | OpenBSD | NetBSD | DragonFly&nbsp;BSD |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| LUKS1 | 🧅 | 🍰 | 🍰 | 🍰 | 🍰 | 🧅 🛈 |
| LUKS2 | 🧅 | 🍰 | 🍰 | 🍰 | 🍰 | 🍰 |
| Apple Encrypted Disk Image | — | 🧅 | — | — | — | — |
| APFS Encrypted | — | 🍰 | — | — | — | — |
| GELI | — | — | 🧅 | — | — | — |
| PEFS | — | — | 🧅 | — | — | — |
| softraid CRYPTO | — | — | — | 🧅 | — | — |
| softraid RAID1C | — | — | — | 🧅 | — | — |
| CGD | — | — | — | — | 🧅 | — |
| VeraCrypt | 🥑 | 🥑 | 🥑 | 🥑 | 🍰 | 🍰 |
| gocryptfs | 🥑 | 🥑 | 🥑 | 🍰 | 🍰 | — |
| CryFS | 🥑 | 🥑 | 🍰 | 🍰 | 🍰 | — |
| Cryptomator | 🥑 | 🥑 | 🍰 | 🍰 | 🍰 | — |
| securefs | 🍰 | 🍰 | 🍰 | 🍰 | 🍰 | — |

🛈 LUKS1 on DragonFly BSD has limited resilience to sudden power loss.

### Planned integration batches

- **Batch 1** — VeraCrypt on DragonFly BSD, APFS Encrypted on MacOS, and securefs on Linux, MacOS, and FreeBSD.
- **Batch 2** — VeraCrypt on NetBSD.
- **Batch 3**
  - LUKS1 and LUKS2 on MacOS, FreeBSD, OpenBSD, and NetBSD, plus LUKS2 on DragonFly BSD.
  - gocryptfs and securefs on OpenBSD and NetBSD.
  - CryFS and Cryptomator on FreeBSD, OpenBSD, and NetBSD.

## Dependencies

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
      <td>MacOS</td>
      <td><code>FUSE-&#8288;T==1.2.9</code></td>
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

[Build & usage](docs/usage.md) · [Linux & BSD packaging](docs/unix-packaging.md) · [MacOS packaging](docs/macos-backends.md) · [Validation results](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md)

[GPL-3.0-only](LICENSE)
