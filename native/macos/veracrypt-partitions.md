# Native macOS VeraCrypt data partitions

The CLI/helper and desktop partition controls support native VeraCrypt creation
and opening on identified external GPT data partitions. The initial qualified
physical filesystem is exFAT. File-only ext4/UFS2/NTFS/ZFS adapter paths are not
used for physical devices.

```
lurker create --type veracrypt --force /dev/diskNsM
lurker mount --type auto /dev/diskNsM mountpoint
lurker unmount --type veracrypt mountpoint
```

Creation takes no file size and requires `--force`. The fixed profile remains
normal VeraCrypt, AES/Serpent/Twofish XTS, SHA-512, PIM 0 and either password or
keyfile authentication. The existing device profile uses `--quick` and lets the
native engine determine the partition's full size; it never passes a file
`--size` argument. These arguments remain in `workflow.rs`.

Before create/open/close, Lurker requires an administrator-controlled private
VeraCrypt executable and ancestor path. A user-owned development backend cannot
start the privileged native core service. The complete installation must satisfy
that trust check, with its separately installed compatible FUSE runtime. This
round uses the explicit audited native macFUSE build; FUSE-T's SMB transport does
not qualify as native-only filesystem support.

The frontend and helper continue running as the login user. VeraCrypt's native
core service obtains device access and restores temporary device ownership.
The round uses Buran's existing authorized passwordless privilege route;
interactive authorization on other installations remains separately unqualified.
The ordinary user can list and close the resulting scoped volume.

Device checks reject whole disks, internal/system storage, mounted or reserved
partitions, non-GPT layouts and existing APFS containers. A parent carrying APFS
system-role volumes is refused. The selected GPT partition UUID, offset/size,
native block-node identity, parent model/transport/tree and hardware serial are
captured and rechecked before create/open. Normal close separately identifies the owned mounted volume and its native service. The serial comes from that exact media's
IOKit ancestors, not a global list of similarly named USB devices. A virtual
partition instead requires its exact login-user-owned image/backing inode and
native attachment association. Missing stable identity is a refusal.

Auto opening recognizes native APFS devices only when the selected device maps
to an actual encrypted APFS data volume or its physical store. An arbitrary
`/dev` path no longer implies APFS. Physical VeraCrypt must publish a native
writable exFAT mount; uncertain or unsupported filesystem state is preserved or
closed normally, never changed into a read-only compatibility mode.

Fresh evidence includes root-restricted virtual partitions, actual Slint
create/open/reopen/close callbacks through the trusted helper, and a small
Toshiba USB data-partition sample. The physical sample retained the downloaded
PDF/text payload through four opens, appended data and a second file, and
restored root:operator device ownership. Its earlier APFS and directory samples
were preserved. See `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/MACOS_RESULTS.md`.
