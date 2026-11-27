<p align="center">
  <img src="extras/assets/lurker.png" alt="Lurker" width="220">
</p>

# Lurker — Encryption made easy.

**For Linux, MacOS, FreeBSD, OpenBSD, NetBSD and DragonFly BSD.**

Protect disks, file containers and directories with the CLI or native desktop app. Encrypt individual files with the CLI.

## Encryption support

- 🧅 Wraps an encryption tool on your system
- 🥑 Encryption engine included with Lurker
- 🛈 See note below
- WIP — Work in progress
- ○ — Planned, not started
- — Unavailable

### Disk, volume, and container encryption

<table width="100%">
  <thead>
    <tr>
      <th scope="col" align="left" width="10000">Encryption</th>
      <th scope="col" width="1%" nowrap><samp>Linux</samp></th>
      <th scope="col" width="1%" nowrap><samp>MacOS</samp></th>
      <th scope="col" width="1%" nowrap><samp>FreeBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>OpenBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>NetBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>DragonFly&nbsp;BSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>Windows</samp></th>
    </tr>
  </thead>
  <tbody align="center" valign="middle">
    <tr height="60">
      <td align="left">V&#8203;e&#8203;r&#8203;a&#8203;C&#8203;r&#8203;y&#8203;p&#8203;t</td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td>WIP</td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">L&#8203;U&#8203;K&#8203;S&#8203;1</td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"> <sup>🛈</sup></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">L&#8203;U&#8203;K&#8203;S&#8203;2</td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td>WIP</td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">Apple&nbsp;Encrypted<br>Disk&nbsp;Image</td>
      <td><sup>—</sup></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
    </tr>
    <tr height="60">
      <td align="left">A&#8203;P&#8203;F&#8203;S<br>E&#8203;n&#8203;c&#8203;r&#8203;y&#8203;p&#8203;t&#8203;e&#8203;d</td>
      <td><sup>—</sup></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
    </tr>
    <tr height="60">
      <td align="left">G&#8203;E&#8203;L&#8203;I</td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
    </tr>
    <tr height="60">
      <td align="left" nowrap>softraid<br>CRYPTO&nbsp;/&nbsp;RAID1C</td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
    </tr>
    <tr height="60">
      <td align="left">C&#8203;G&#8203;D</td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
    </tr>
    <tr height="60">
      <td align="left">BitLocker</td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td>○</td>
    </tr>
  </tbody>
</table>

### Directory encryption

<table width="100%">
  <thead>
    <tr>
      <th scope="col" align="left" width="10000">Encryption</th>
      <th scope="col" width="1%" nowrap><samp>Linux</samp></th>
      <th scope="col" width="1%" nowrap><samp>MacOS</samp></th>
      <th scope="col" width="1%" nowrap><samp>FreeBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>OpenBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>NetBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>DragonFly&nbsp;BSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>Windows</samp></th>
    </tr>
  </thead>
  <tbody align="center" valign="middle">
    <tr height="60">
      <td align="left">P&#8203;E&#8203;F&#8203;S</td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
    </tr>
    <tr height="60">
      <td align="left">g&#8203;o&#8203;c&#8203;r&#8203;y&#8203;p&#8203;t&#8203;f&#8203;s</td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td>○</td>
      <td>○</td>
      <td><sup>—</sup></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">C&#8203;r&#8203;y&#8203;F&#8203;S</td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td>WIP</td>
      <td>WIP</td>
      <td>WIP</td>
      <td><sup>—</sup></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">C&#8203;r&#8203;y&#8203;p&#8203;t&#8203;o&#8203;m&#8203;a&#8203;t&#8203;o&#8203;r</td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td>WIP</td>
      <td>○</td>
      <td>○</td>
      <td><sup>—</sup></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">securefs</td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td>○</td>
      <td><sup>—</sup></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">EFS</td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td><sup>—</sup></td>
      <td>○</td>
    </tr>
  </tbody>
</table>

### Individual-file encryption

<table width="100%">
  <thead>
    <tr>
      <th scope="col" align="left" width="10000">Encryption</th>
      <th scope="col" width="1%" nowrap><samp>Linux</samp></th>
      <th scope="col" width="1%" nowrap><samp>MacOS</samp></th>
      <th scope="col" width="1%" nowrap><samp>FreeBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>OpenBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>NetBSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>DragonFly&nbsp;BSD</samp></th>
      <th scope="col" width="1%" nowrap><samp>Windows</samp></th>
    </tr>
  </thead>
  <tbody align="center" valign="middle">
    <tr height="60">
      <td align="left">a&#8203;g&#8203;e</td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td><img src="extras/assets/status/avocado.png" width="24" align="absmiddle" alt="Included"></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">O&#8203;p&#8203;e&#8203;n&#8203;S&#8203;S&#8203;L<br>CMS</td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td>○</td>
    </tr>
    <tr height="60">
      <td align="left">G&#8203;n&#8203;u&#8203;P&#8203;G<br>(OpenPGP)</td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td><img src="extras/assets/status/onion.png" width="24" align="absmiddle" alt="Wrapper"></td>
      <td>○</td>
    </tr>
  </tbody>
</table>

🛈 LUKS1 on DragonFly BSD has limited resilience to sudden power loss.

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

For file encryption, age is included with Lurker. GnuPG requires an installed `gpg`. OpenSSL CMS requires OpenSSL 3 or newer and its matching `libcrypto`. macOS, OpenBSD and DragonFly BSD may require a separate OpenSSL package alongside system LibreSSL. These dependencies are needed only for their respective backends.

## Create. Unlock. Lock.

```bash
lurker create vault.img 1
mkdir -p vault
lurker mount vault.img vault
lurker unmount vault
```

Lurker chooses the native format for your OS. Use administrator authorization where required.

## Encrypt and decrypt a file

```bash
lurker encrypt --type age --passphrase my_data encrypted_my_data
lurker decrypt --type age --passphrase encrypted_my_data restored_my_data
```

Choose `--type age`, `--type gnupg` or `--type openssl` (CMS).

- **age** accepts a passphrase or public recipient files.
- **GnuPG** accepts a passphrase, a passphrase supplied through `--key-file`, public keys, or a combination of shared and public-key credentials.
- **OpenSSL CMS** uses AES-256-GCM and accepts a passphrase, a shared AES `--key-file`, X.509 certificates, or any combination.

Repeat `--public-key` for multiple recipients. Use `--private-key` for age or CMS decryption. GnuPG finds the matching private key in its existing setup. Each enrolled credential can decrypt independently.

Each operation takes one file and produces one file. Archiving, compression and key management are outside the file-encryption workflow.

Native CLI tests passed on all 11 build hosts across six Linux distributions, macOS and four BSDs, with 220 byte-identical PDF restores and 33 expected wrong-passphrase rejections. See the [file-encryption results](DIRECTIVES/PLAN/FILE_ENCRYPTION.md#5-native-cli-results) and [CLI examples](docs/usage.md#encrypt-and-decrypt-individual-files).

## Filesystem Tested

| Filesystem | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ext4 | ✅ | ☑️ | ☑️ | ⚠️ R/O | ☑️ | ⚠️ R/O | ☑️ |
| Btrfs | ✅ | ☑️¹ | ⚠️ R/O | — | — | — | ☑️ |
| XFS | ✅ | ☑️ | ⚠️ R/O | ⚠️ R/O | — | ⚠️ R/O | ⚠️ R/O |
| APFS | ☑️ | ✅ | ⚠️ R/O | ⚠️ R/O | — | ⚠️ R/O | ☑️¹ |
| UFS1 / FFSv1 | ☑️ | — | ✅ | ✅ | ✅ | ✅ | ⚠️ R/O |
| UFS2 / FFSv2 | ☑️ | ☑️² | ✅ | ✅ | ✅ | — | ⚠️ R/O |
| HAMMER2 | ✅² | — | ☑️² | ☑️³ | ☑️ | ✅ | — |
| NTFS | ☑️ | ☑️ | ☑️ | ☑️ | ☑️ | ⚠️ R/O | ✅ |
| exFAT | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| ZFS / OpenZFS | ✅ | ✅ | ✅ | — | ☑️ | — | ✅² |

**Legend**

- **✅** Full feature support.
- **☑️** Largely usable, with some filesystem features missing or unverified.
- **⚠️** Read-only access; write support is missing.
- **—** Unsupported.
- **¹** Disk/container creation not available.
- **²** Largely usable; reliability not established.
- **³** Native writable port exists; compatibility with the current OS release is not established.

Maturity is independent of feature completeness. Linux HAMMER2 physical-device reliability remains unqualified by its scratch-image tests. macOS UFS2 uses an experimental writer; moving directories between parents is unsupported. Windows OpenZFS is largely implemented; its remaining stream-renaming limitation does not prevent the core workflows.

For Linux, macOS and the BSDs, this planning matrix combines achieved and achievable host-local support, excluding VMs and network filesystems at every layer. Linux covers all six distributions with compatible kernels/modules; this does not establish every filesystem on one booted kernel. Achievable cells can still require Lurker integration, including aligned device reads for DragonFly XFS/APFS. Read-only access and missing creation remain shortcomings against Create + Read/Write. See the [native-only assessment and limits](docs/filesystem-support-matrix.md#native-only-planning-update). Windows entries are unchanged by this reassessment. The table does not assert completed integration or runtime qualification in Lurker.

## Get started

[Build & usage](docs/usage.md) · [Linux & BSD packaging](docs/unix-packaging.md) · [MacOS packaging](docs/macos-backends.md) · [Validation results](DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md)

[GPL-3.0-only](LICENSE)
