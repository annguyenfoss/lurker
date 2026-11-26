# Optional macOS UFS2 Create + Read/Write tools

This development adapter combines a free formatter and a free writable driver.
UFS2Tool creates the filesystem; fuse-ufs mounts it. Both are BSD-2-Clause.
macFUSE is installed and approved separately. VeraCrypt can use the explicitly
selected native macFUSE build; FUSE-T remains the default build option.
There is one mount/unmount action, with no selectable access mode.

The upstream writer is **very experimental** and warns of data loss. Passing
Lurker's ordinary payload workflow does not establish a complete or production
quality UFS implementation. Cross-parent directory moves are explicitly refused
before mutation because upstream does not update `..` and the parent link counts.
Same-parent file/directory renames and cross-parent regular-file moves pass native
controls. Extended attributes, ACLs, crash recovery and full metadata parity are
unqualified. Use disposable data for development.

## Pinned sources

| Component | Revision | Source archive SHA-256 |
|---|---|---|
| fuse-ufs / rufs 0.6.0 | `ee5c4a447da98220a11edb69a225d51379afc4fb` | `b8b75a51a12f05090388f5aeff1f11b53f0482ab8299edfee7ccac40d4923243` |
| fuser 0.16.0 | crates.io archive | `0bb29a3ae32279fe3e79a958fe01899f5fb23eadccee919cf88e145b54ed9367` |
| UFS2Tool 4.1 | `00317e32bbe7aa359fe7fbc03de6a5bec4a75274` | `a1dd6b277d27a28484ce84a5a252ef472e1b779c18eb4059c9f9548921301b5e` |

Download and verify these archives before extracting into private directories:

- https://codeload.github.com/realchonk/fuse-ufs/tar.gz/ee5c4a447da98220a11edb69a225d51379afc4fb
- https://static.crates.io/crates/fuser/fuser-0.16.0.crate
- https://codeload.github.com/SvenGDK/UFS2Tool/tar.gz/00317e32bbe7aa359fe7fbc03de6a5bec4a75274

Do not modify the shared Cargo registry. Install free build prerequisites using
Homebrew (`pkgconf`, `dotnet@8`) and the existing user-managed Rust toolchain.
The native macFUSE SDK must provide `/usr/local/lib/pkgconfig` and
`/usr/local/lib/libfuse3.4.dylib`. No private tool is run as root.

## Driver build

Apply these patches in order to the private fuse-ufs source checkout:

1. `fuse-ufs-darwin-foreground.patch`: replace the unavailable Darwin daemon
   call with a foreground-only requirement (`-f`).
2. `fuse-ufs-rw-sync.patch`: implement flush/fsync/fsyncdir through the actual
   backing file, propagate sync errors and reject zero-length writes.
3. `fuse-ufs-initialize-owner.patch`: initialize only a freshly formatted image's
   root UID/GID and mode 0700, then synchronize without mounting it.
4. `fuse-ufs-raw-pointer-block.patch`: zero indirect-pointer blocks with raw bytes;
   upstream's encoded vector prepended a length and wrote past the block boundary.
5. `fuse-ufs-lurker-capabilities.patch`: report `ufs2_write_protocol: 1` so Lurker
   refuses older reader builds without these write/sync fixes.
6. `fuse-ufs-cross-parent-directory-guard.patch`: refuse the unsupported directory
   move before touching either directory.

Apply `fuser-0.16-macos-libfuse3.patch`, then
`fuser-macos-libfuse3-rename.patch` to the private fuser source. The first selects
its existing libfuse3 mount backend while retaining Darwin attribute layouts.
macFUSE 5.3+ disables the old `fuse_mount_compat25` entry point. The second uses
macFUSE's classic eight-byte rename request because fuser does not negotiate the
extended rename capability; the old layout consumed eight filename bytes.
RENAME2 remains 16 bytes. Protocol reference:
https://github.com/macfuse/library/blob/85f89460c4962ae94a84e25037326b22efb7f0e0/include/fuse_kernel.h#L838-L846

From the patched fuse-ufs source, with the actual private fuser path:

```sh
PKG_CONFIG_PATH=/usr/local/lib/pkgconfig \
PKG_CONFIG_LIBDIR=/usr/local/lib/pkgconfig \
cargo --config 'patch.crates-io.fuser.path="/absolute/private/fuser-0.16.0"' \
  build --offline --release -p fuse-ufs
otool -L target/release/fuse-ufs
target/release/fuse-ufs --lurker-capabilities
```

The first offline build requires the locked dependencies to be cached. Fetch
those explicitly beforehand if needed. Review the lockfile: only fuser's source
identity should change. Add `--locked` for subsequent builds. Verify linkage to
`/usr/local/lib/libfuse3.4.dylib` and the exact write-protocol response above.

## Formatter build and staging

Apply `ufs2tool-cg0-backup.patch` to the pinned UFS2Tool source. It writes the
otherwise omitted cylinder-group-zero backup superblock. From that source:

```sh
export DOTNET_ROOT=/opt/homebrew/opt/dotnet@8/libexec
export DOTNET_CLI_TELEMETRY_OPTOUT=1
/opt/homebrew/opt/dotnet@8/bin/dotnet build UFS2Tool.csproj \
  -c Release -o /absolute/private/ufs2tool-runtime
```

Install these host prerequisites separately under
`/usr/local/libexec/lurker-filesystems/`; Lurker does not package filesystem tools
or discover them relative to its CLI/app. Stage the following files in a private
build directory, then copy them into that administrator-owned host directory:

- `fuse-ufs`: the patched release executable, mode 0755.
- `ufs2tool`: this directory's `ufs2tool.sh`, copied with mode 0755.
- `ufs2tool-runtime/`: the complete .NET build output, including `.dll`,
  `.deps.json`, `.runtimeconfig.json` and dependencies.
- Original license notices from all three pinned sources.

Build the tools as the login user. Administrator authorization is needed only
to install the reviewed files and keep the host directory administrator-owned;
never execute a user-writable private driver or formatter with sudo.

The development launcher uses the separately installed Homebrew .NET 8 runtime;
this stage is not a relocatable or offline-qualified release package. The
qualified host is Apple-silicon Buran, macOS 27.2.1 with macFUSE 5.5.0.

## Creation and lifecycle

Lurker creates a normal fully initialized VeraCrypt container without an inner
filesystem, opens its authenticated decrypted image, and normally detaches the
unused hdiutil device. It reserves a private mode-0600 blank raw file in the
creation staging directory and formats that file with:

```sh
ufs2tool newfs -O 2 -b 32768 -f 4096 -s EXACT_512_BYTE_SECTOR_COUNT NEW_PRIVATE_RAW
fuse-ufs --initialize-owner UID:GID NEW_PRIVATE_RAW
```

UFS2Tool truncates its output, so never point it at a live decrypted view or a
block device. Lurker checks the new raw file's identity, ownership, private mode,
link count and exact geometry, then copies the initialized blank filesystem to
the decrypted image without truncating it and synchronizes it. The blank staging
file contains only new filesystem metadata/zero blocks, never existing user data
or credentials. It is removed after normal crypto close. No soft updates or SUJ
are requested. Imported filesystem ownership is never changed.

Mount uses `fuse-ufs -f -o rw,nodev,nosuid`. Normal close drains the identified
filesystem before closing encryption. Startup and close use the same private
identity-checked filesystem registry as the other macOS adapters. Native controls
and actual encrypted CLI receipts are recorded in
`DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RETEST.md`.
