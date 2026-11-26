# Native filesystems in Apple encrypted images

The CLI and helper can create Apple AES-256 encrypted UDIF containers with
ext4, NTFS or OpenZFS inside. APFS/exFAT retain the ordinary native image path.
The desktop keeps its native default filesystem; its UI does not currently
expose these explicit foreign-filesystem selections.

```
lurker create --type apple --filesystem ext4 vault.dmg 0.75
lurker mount --type auto vault.dmg mountpoint
lurker unmount --type apple mountpoint
```

Foreign-filesystem images require at least 384 MiB and password authentication.
The fixed AES-256 profile is unchanged. Filesystem tools and drivers are separate
host prerequisites, discovered through administrator-installed host locations;
they are never bundled or elevated from a private Lurker package.

- ext4 uses the host e2fsprogs formatter and fuse-ext2 with the local FUSE-T
  FSKit transport. Its new filesystem root belongs to the invoking login user.
- NTFS uses the host mkntfs formatter and the permanently free TT NTFS Native
  FSKit driver. A writable request that first encounters Apple's read-only
  NTFS mount is closed normally and remounted through the identified native
  FSKit route. A silent read-only downgrade is refused.
- OpenZFS uses the separately installed native OpenZFSonOsX driver and trusted
  administrator-owned ZFS utilities. Existing single-device pool/sole-vdev GUID
  checks, private mounts and normal pool export apply to Apple images too.

Creation reserves a private zero-only raw file, attaches that exact file as a
native virtual disk, and asks hdiutil to create an AES-256 UDRW image from it.
The temporary raw file contains no existing plaintext or credentials. After the
blank device closes, Lurker authenticates the encrypted image and formats only
its identified virtual device. This avoids relying on an unsupported
`hdiutil create -fs none` combination. No compatibility filesystem or guest
provides the final filesystem.

A private recovery record is durable before formatting, driver startup or ZFS
import can change an authenticated device. It records the crypto backend,
encrypted source inode/device, native attachment and mount identity, and any
owned driver process or ZFS pool identity. Old records without the Apple flag
retain their VeraCrypt meaning. Normal close checks these identities, unmounts
or exports the filesystem, waits for an owned driver to drain, and only then
detaches the matching hdiutil image. An uncertain teardown preserves recovery
state and storage. No credentials are written into the record.

UFS2's regular-file driver route does not qualify its native block-device
transport: backing-device fsync returns ENOTTY/EIO. The current XFS driver also
failed ordinary persistence qualification. These routes remain refused. Btrfs
VM/network-export routes cannot supply native Apple-image support.

Host prerequisites and sources:
https://github.com/alperakcan/fuse-ext2
https://e2fsprogs.sourceforge.net/
https://github.com/macos-fuse-t/fuse-t
https://github.com/tuxera/ntfs-3g
https://ntfs.techtag.ch/
https://github.com/openzfsonosx/openzfs-fork/releases
