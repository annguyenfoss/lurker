# Usage and build guide

Run commands from the repository root. See the [README](../README.md) for current platform support and the [validation report](../DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md) for tested behavior and remaining limits.

## Repository Layout

```text
lurker/
├── crates/
│   ├── lurker-core/    # shared domain, backend lifecycles and native OS adapters
│   ├── lurker-cli/     # `lurker` terminal binary
│   └── lurker-helper/  # internal helper used by the desktop app
├── apps/
│   └── lurker-desktop/ # Slint desktop app
└── extras/
```

## Current Products

### CLI

```text
lurker create [--type TYPE] [--cipher CIPHER] <file> <size-gb> [-F|--force]
lurker create [--type TYPE] [--cipher CIPHER] <block-device> [-F|--force]
lurker mount [--type TYPE] [--readonly] [-t TAG] <source> <mountpoint>
lurker unmount [--type TYPE] [-t TAG] <target>
```

Build it:

```bash
cargo build --release -p lurker-cli
```

Run it:

```bash
./target/release/lurker help
```

Install the Rust CLI alone (native/private backend dependencies remain required):

```bash
install -m 755 ./target/release/lurker /usr/local/bin/lurker
```

Linux examples:

```bash
./target/release/lurker create ./vault.img 4
./target/release/lurker create --cipher serpent ./vault-serpent.img 4
./target/release/lurker create --cipher twofish /dev/sdb1 --force
./target/release/lurker create --type veracrypt --cipher serpent ./vault.hc 4
./target/release/lurker createvc --cipher twofish /dev/sdc1 --force
./target/release/lurker mount ./vault.img /mnt/vault
./target/release/lurker unmount /mnt/vault
```

### Opt-in session and shutdown close on Linux and BSD

Add one `session-close` call for each managed vault to the script that ends your
application session. Stop its consumers and wait for them to exit first. Run
the hook in the same mount namespace, while the vault source, mount and native
tools are still available. Continue the logout or shutdown only after the close
returns zero. For example, a Linux administrator can stop the service that
uses a native LUKS vault, then run the close before requesting shutdown.

```sh
systemctl stop example-profile.service &&
    /usr/local/bin/lurker session-close --type luks --timeout 30 /srv/vaults/profile.img
```

Use the explicit backend and the absolute path of the encrypted directory or
container file. Native hooks require existing administrator authority and an
administrator-owned CLI installation and vault source parent hierarchy. Run
directory backends other than PEFS as the login user that opened the vault.
For Linux user-mode VeraCrypt, add `--unprivileged` and keep the opening user's
runtime directory available. Use manual `unmount` for tagged mappings or
physical devices.

The default close deadline is 30 seconds and `--timeout` accepts 1–300 seconds.
A busy filesystem returns the usual close error and remains mounted. Exit
status `124` means the normal close may still be running. The hook reports
its PID and leaves the drain running. Keep the session available and check
the mount and worker before retrying. The hook has no credential prompt.

On Linux, run the hook before `systemctl poweroff` or login-session teardown.
An `ExecStop` job that kills leftover processes in its unit cgroup can
interrupt a worker after the hook returns `124`. On BSD, call the hook from
the explicit session or service stop workflow before invoking `shutdown`.
Late shutdown scripts may run after consumers, runtime directories or mounts
have already been torn down, and a failed stop job alone does not cancel an
operating-system shutdown. Lurker installs no login or shutdown hooks.

### Desktop App

The desktop app is intentionally simple for now:

- one main window
- create / mount / unmount tabs
- active volumes from the shared backend discovery
- backend choices and create defaults from the approved OS matrix
- file, directory and Linux physical-device targets, with backend-specific fields
- status and errors in the application
- built-in zoom controls for larger displays
- Linux Privileged / Unprivileged selector; unprivileged mode handles VeraCrypt files and directory engines without administrator prompts

It uses:

- `lurker-core` for all shared logic
- `lurker-helper` for desktop operations; native operations authorize when required, while portable directory engines run as the login user
- Slint + winit + software renderer for the GUI shell

Run the desktop app in development:

```bash
cargo build -p lurker-helper
cargo run -p lurker-desktop
```

Build the desktop app:

```bash
cargo build --release -p lurker-helper -p lurker-desktop
```

Install the desktop binaries:

```bash
install -m 755 ./target/release/lurker-desktop /usr/local/bin/lurker-desktop
install -m 755 ./target/release/lurker-helper /usr/local/bin/lurker-helper
```

For system packaging, keep `lurker-desktop` and `lurker-helper` together.
BSD form defaults and backend choices are implemented; native BSD desktop
runtime qualification remains separate from the tested CLI backends.

## Linux Without Administrator Prompts

Choose **Unprivileged** in the desktop header. It supports VeraCrypt file containers with AES, Serpent or Twofish, the same SHA-512/PIM defaults, and Btrfs with zstd compression. LUKS and physical devices remain available under **Privileged**. Both modes' volumes appear in Manage; unprivileged sessions keep their original mode when unmounted.

Build the additional helpers as your normal user:

```bash
python scripts/build-linux-userspace.py
cargo build --release --workspace --locked
./target/release/lurker-desktop
```

The script downloads fixed upstream VeraCrypt and Linux Kernel Library revisions, applies the tracked integration patches, and places a private creator and two userspace executables in `target/release/userspace/`. It never installs packages, invokes sudo, or modifies system configuration. It needs a C/C++ toolchain, Python 3.12+, make, patch, pkg-config, wxWidgets base, FUSE3 development files, flex, bison, bc and Perl. For a debug desktop, set `LURKER_USERSPACE_DIR="$PWD/target/release/userspace"` when launching it. For a system-wide installation, place the helpers under `/usr/lib/lurker/userspace`.

The CLI exposes the same route:

```bash
./target/release/lurker create --unprivileged ./vault.hc 1
./target/release/lurker mount --unprivileged ./vault.hc ~/Lurker-mount
./target/release/lurker unmount --unprivileged ~/Lurker-mount
```

Mount points must be empty. A missing final directory is created when its parent exists. New containers belong to your user. Existing containers preserve their permissions. Read-only mounts protect both the encryption and filesystem layers. Standard user FUSE support, including access to `/dev/fuse` and the installed `fusermount3` helper, is required; that system helper performs the restricted FUSE mount without prompting for administrator credentials.

Native userspace components require transport protocol 4. It keeps an exclusive
source-inode lock to prevent duplicate kernel/userspace attachment and forwards
filesystem writeback barriers to the encrypted backing file. Close active
sessions before upgrading the Rust helper and native components together.
For kernel operations on unreadable root-owned images, run the CLI as root;
the source-inode guard must open the image before permitting attachment.

This mode is experimental. It supports ordinary file access rather than kernel Btrfs management ioctls such as snapshots or scrub. See `docs/linux-userspace.md` for architecture, supported filesystems, recovery, and tests. macOS keeps its existing native workflows.

## Linux Runtime Requirements

Privileged mode uses these system tools; unprivileged requirements are listed above.

- `cryptsetup`
- `mkfs.btrfs`
- `mount`
- `umount`
- `lsblk`
- `pkexec` for preferred desktop privilege escalation
- `sudo` for desktop fallback when launched from a terminal
- optional `blkid`
- the private `lurker-veracrypt-create` component for VeraCrypt creation; a separately installed VeraCrypt application is not used

Complete Linux/BSD distributions are staged with `scripts/stage-unix.py`; see
`docs/unix-packaging.md` for build inputs, dependency manifests and archive layout.
A Cargo-only build supplies the Rust products, not the bundled encryption engines.
Directory engines run as the login user: gocryptfs needs host FUSE support, stable
CryFS 1.0.3 uses FUSE ABI 2, and Cryptomator carries a private Java runtime.

## Directory and native BSD CLI backends

```bash
lurker create --type gocryptfs ./encrypted-profile
lurker mount ./encrypted-profile ./profile-mount
lurker unmount ./profile-mount
```

Directory create takes no size, `--cipher` or replacement flag. Select `cryfs`,
`cryptomator` or FreeBSD `pefs` in the same form where the approved matrix allows
it. Backend builds and host prerequisites are separate; do not infer support from
an executable compiling. macOS directory qualification requires a compatible
separately installed macFUSE; the tested FUSE-T routes failed SQLite WAL checks.

Native BSD defaults are GELI/UFS (FreeBSD), CGD/FFSv2 (NetBSD), softraid CRYPTO/FFS
(OpenBSD), and LUKS1/UFS (DragonFly). Native containers take `<file> <size-gb>` and
administrator authorization. BSD physical-device creation is excluded. Keep
NetBSD `.cgdparams` and PEFS `.pefs.conf`/`.pefs.db` metadata with their encrypted
data. OpenBSD CRYPTO and RAID1C refuse read-only attachment.
See `docs/backend-testing.md` for the exact tested revisions and remaining gates.

For a new FreeBSD GELI image, plan for the full requested size on the backing
filesystem. Lurker reserves and syncs that space before formatting when the
filesystem supports native preallocation. On filesystems that decline
preallocation, new images use sparse files.

DragonFly uses the stock `dm_target_linear` module. Unlock accepts the
DragonFly LUKS1 cipher and keyslot profiles in
`DIRECTIVES/LLM/CIPHER.md` and uses a private metadata copy during each
session. Preserve the complete original container as usual.

OpenBSD RAID1C is an explicit two-file mirror. Size is per member.

```bash
lurker create --type raid1c --member ./mirror-b.img ./mirror-a.img 1
mkdir ./mirror-mount
lurker mount --type raid1c --member ./mirror-b.img ./mirror-a.img ./mirror-mount
lurker unmount ./mirror-mount
```

Keep both members and the `.raid1c.json` descriptor. To unlock an existing
array with only one surviving member, use
`lurker mount --type raid1c --degraded ./mirror-a.img ./mirror-mount`.
RAID1C uses a password and refuses forced replacement of existing files.

## macOS Drag-and-Drop Installer

Build a native release DMG on the Mac where you want to install it:

```bash
# One-time build tooling setup (not needed by people installing the app):
uv venv build/macos-dmg-tools
uv pip install --python build/macos-dmg-tools/bin/python -r scripts/requirements-macos-dmg.txt
build/macos-dmg-tools/bin/python scripts/build-macos-dmg.py
```

Open `dist/Lurker-0.1.0-macos-arm64.dmg` on Apple Silicon (or the `x86_64` DMG when built on Intel), then drag **Lurker.app** onto **Applications** and eject the image. The app bundles its CLI, helper, selected private backends, fonts, icons and UI resources. No `.pkg`, terminal installation, or separately installed Lurker CLI is needed.

The installer opens an 800-point-wide Finder window with an illustrated Retina background, fixed app/Applications icon positions, and hidden supporting files. Its saved Finder settings are authored directly, so building does not require Finder automation or changing system preferences.

The script builds the architecture of the current Rust toolchain, derives the minimum macOS version from the binaries, verifies the bundle signature and disk-image checksum, and writes a `.sha256` checksum next to the DMG. Apple encryption uses built-in system tools. The private VeraCrypt backend is bundled; its matching FUSE runtime remains a separately installed prerequisite. Optional directory backends include their private runtimes; a system Java or Go installation is not required. See `docs/macos-backends.md` for the qualified runtime boundaries and build arguments.

Local builds are ad-hoc signed and are not Apple-notarized. Distribution signing can use `CODESIGN_IDENTITY`, but notarization is a separate release step requiring a Developer ID distribution certificate and Apple credentials. See https://developer.apple.com/developer-id/.

## macOS Containers

The macOS backend implementations are:

- **Apple Encryption** (default): an AES-256 encrypted, read/write `.dmg` virtual disk with APFS inside. Encryption protects the disk image, not individual files or the host filesystem. This uses Apple DiskImages encryption, not system FileVault.
- **VeraCrypt**: a normal file container with AES, Serpent, or Twofish, SHA-512, and an exFAT filesystem inside.

Apple encryption uses the built-in `/usr/bin/hdiutil`; VeraCrypt unmount also uses `/usr/sbin/diskutil` to reject busy filesystems. The packaged VeraCrypt backend needs its qualified separately installed FUSE runtime, and does not require VeraCrypt.app. Source-only builds retain installed-app discovery as a compatibility fallback; use the complete native backend builder for a standalone app. FUSE-T and macFUSE are not interchangeable without qualification.

Build and install the same binaries as above. Keep `lurker-helper` next to `lurker-desktop`. On macOS the helper runs in the login user session; Linux pkexec/sudo escalation is not used.

```bash
lurker create ./vault.dmg 4
lurker create --type apple ./apple-vault.dmg 4
lurker create --type veracrypt --cipher serpent ./vault.hc 4
mkdir -p "$HOME/vault-mount"
lurker mount ./vault.dmg "$HOME/vault-mount"
lurker unmount "$HOME/vault-mount"
```

macOS create supports regular files only (minimum 32 MiB). Mount points must be empty; the final directory is created by the native tool if it is missing and its parent exists. It refuses physical disks, partitions, symlink targets, and overwriting an attached container. `--force` can replace an existing unmounted regular file only after creation succeeds. Apple images accept only `--cipher aes`, which selects AES-256. Passphrases are passed through stdin, never command arguments; spaces are preserved and embedded NUL/newline characters are rejected.

The desktop offers Apple Encryption and VeraCrypt on macOS, uses the same create/mount/unmount flows, and supports read-only mounts. Its active list includes the current user's encrypted Apple disk images and VeraCrypt volumes, including containers mounted outside Lurker. Unmount accepts the container file, mountpoint, or its virtual device and detaches that one container. Tags remain Linux LUKS-only.

Existing Linux LUKS containers require Linux. Existing Linux VeraCrypt containers created by Lurker contain Btrfs, which macOS cannot mount natively. New macOS VeraCrypt containers use exFAT; Linux can read them with VeraCrypt. Apple disk images require macOS.

Native tool limitations are returned as errors and never justify replacing a previous container. During native testing, macOS rejected creating an encrypted APFS image directly on a mounted exFAT disk-image host; mounting an existing encrypted image from that host succeeded. Create on a supported host filesystem and copy the closed container if this native error occurs.

Apple disk-image background: https://support.apple.com/guide/disk-utility/create-a-disk-image-dskutl11888/mac

VeraCrypt command reference: https://veracrypt.io/en/Command%20Line%20Usage%20for%20Unix.html

## Notes

- `create` defaults to `luks` on Linux and `apple` on macOS. Use `--type veracrypt` or `createvc` for VeraCrypt creation.
- `create` defaults to `--cipher aes`. The only supported single-cipher create profiles are `aes`, `serpent`, and `twofish`.
- The create profiles are intentionally hardcoded and opinionated. They are defined in the code, not in an external config file.
- AES uses the updated LUKS2 + Argon2id profile from `CIPHER.md`.
- Serpent uses the heavier Argon2id profile from `CIPHER.md`.
- Twofish uses the `CIPHER.md` sector-size tuning and automatically applies the documented open-time perf flags when a Twofish LUKS header is detected.
- VeraCrypt create supports the same three single-cipher choices and keeps `SHA-512` fixed. Device creation uses the full device size detected by VeraCrypt, with quick format.
- On Linux, `mount` recognizes LUKS metadata first; other sources are authenticated as VeraCrypt when opening. Encrypted VeraCrypt headers cannot be identified without the unlock credential. `unmount` reads the active mapper type. Use `--type` to pin the format.
- Privileged Linux VeraCrypt mounts use `cryptsetup --type tcrypt --veracrypt`, with the same `lurker_*` mapper naming, active-volume listing, filesystem handling, and cleanup as LUKS. Unprivileged mode uses the file-only userspace helpers.
- `mount --readonly` opens the mapper and mounts the filesystem read-only. The desktop readonly checkbox does the same.
- VeraCrypt supports normal volumes with either a password or one keyfile, using the default PIM. Password and keyfile together are deliberately unsupported. Hidden-volume controls and custom PIM are not exposed.
- Supplied passwords cannot be empty or contain NUL or line breaks. Leading/trailing spaces and Unicode are preserved. VeraCrypt keyfile-only authentication uses an empty native password internally.
- `-t TAG` is supported for LUKS source-path mount/unmount.
- VeraCrypt tags are still unsupported.
- Btrfs volumes are mounted with `compress=zstd`.
- File-backed create uses same-directory temporary files, atomic publication without overwriting unless forced, and atomic replacement when forced.
- The desktop app offers passphrase or keyfile authentication for LUKS and VeraCrypt. Passphrase creation requires confirmation; the Generate button uses system randomness (128 bits). Linux Privileged mode also offers a LUKS keys screen to list, add and remove independent credentials.
- The desktop app does not call the CLI binary. It talks to `lurker-helper` over a local stdin/stdout JSON contract.
- The CLI supports interactive terminal prompting; macOS create also confirms the passphrase. Passphrases are passed to crypto tools through terminal input or stdin, never command-line arguments.
- New Linux/BSD containers and recovery metadata belong to the user who authorized creation. Newly formatted filesystem roots use that user's ownership and private permissions, so ordinary applications can write after mounting. Existing filesystem owners and permissions remain unchanged when unlocked.

### Credentials and LUKS keyslots

`create` and `mount` accept `--key-file FILE` for LUKS, VeraCrypt, FreeBSD GELI and PEFS. Select an existing, non-empty regular file; its binary contents are used without trimming or text conversion. Without this option, the CLI prompts for a password. Apple disk images retain password authentication.

```bash
lurker create --type luks --key-file ./vault.key ./vault.img 4
lurker createvc --key-file ./vault.key ./vault.hc 4
lurker mount --key-file ./vault.key ./vault.hc /mnt/vault
```

On Linux, manage independent LUKS credentials with `keyslots`, `add-key`, and `remove-key`. LUKS1 has slots 0–7; LUKS2 has up to 32 slots, numbered 0–31, subject to header space. Each enrolled password or keyfile unlocks independently.

```bash
lurker keyslots ./vault.img
lurker add-key ./vault.img --new-key-file ./backup.key  # existing password is prompted
lurker add-key ./vault.img --key-file ./backup.key     # new password is prompted and confirmed
lurker add-key ./vault.img --key-file ./backup.key --new-key-file ./second.key
lurker remove-key ./vault.img --slot 1 --key-file ./second.key
```

`add-key --slot N` selects a free slot explicitly; omitting it lets cryptsetup choose. Existing slots are never overwritten. `remove-key` requires a credential for a slot you are keeping and refuses to remove the last usable slot. Slot management uses Linux Privileged mode; accessible regular-file headers can be managed by the CLI without elevation or mounting.

VeraCrypt keyfiles are an alternative to a password, not an independent backup slot: a container created with a keyfile requires that keyfile's contents. Lurker does not combine a password with a VeraCrypt keyfile.

## Verification

Native tests use only disposable file containers. The installed FUSE-T runtime can leave VeraCrypt service processes alive after dismount during repeated runs; native fixtures clean up only services naming their unique test files. Production operations return native runtime errors without killing unrelated processes.

Current verification target:

```bash
cargo check --workspace --locked
cargo test --workspace --locked
# Linux: native keyslot tests on disposable headers, no root needed
cargo test -p lurker-core --test keyslots_native -- --ignored --test-threads=1
cargo build -p lurker-cli -p lurker-helper
python scripts/test-credentials.py target/debug
# Linux: keyfile create/mount/readonly matrix using disposable loops and files
./scripts/test-linux-core.sh keyfile_round_trips
# Linux: run as your normal user after building the userspace helpers
./scripts/test-linux-userspace.sh
# macOS: native create/mount/read-only/remount/error tests using disposable files
cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1
cargo test -p lurker-helper --test macos_native -- --include-ignored --test-threads=1
cargo build -p lurker-cli
python3 tests/macos_cli.py
```

Real Linux tests create fresh `/tmp` images and dedicated loop devices. They never target an existing vault. See `docs/linux-validation.md` for the full matrix and commands.

## License

This project is licensed under the GNU General Public License v3.0 only. See [LICENSE](../LICENSE).

### Cross-platform backend development

The approved backend matrix and implementation plan are in
`DIRECTIVES/LLM/CIPHER.md` and `DIRECTIVES/PLAN/CROSS_PLATFORMS.md`.
Native qualification and remaining limits are recorded in
`DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md`. CLI directory creation uses
`create --type gocryptfs|cryfs|cryptomator|pefs <directory>` without a size or
cipher option. These backends retain their native fixed encryption profiles.
Explicit Linux `--type luks1` uses XTS/PBKDF2-SHA-512; the `luks` default remains
LUKS2/Argon2id. Backend/OS combinations outside the approved matrix are rejected.
BSD native containers use GELI/UFS (FreeBSD), CGD/FFSv2 (NetBSD),
softraid CRYPTO/FFS (OpenBSD), and LUKS1/UFS (DragonFly). Run native container
operations through the OS administrator authorization tool. NetBSD images must
be kept together with their `.cgdparams` recovery sidecars. NetBSD writable
mounts require FFS journaling (WAPBL). The first writable mount of an older
image may allocate a journal and requires free space inside the image.

If an interrupted GELI, CGD, softraid CRYPTO, BSD VeraCrypt, or
DragonFly LUKS1 create leaves a native session open while the requested
file is still absent, run `lurker unmount --type auto <requested-path>`
with administrator authority. Lurker verifies and closes the recorded
session and reports where the unpublished image remains for inspection.
For CGD, keep any staged `.cgdparams` file with that image.

OpenBSD mounts writable FFS synchronously for softraid CRYPTO, RAID1C and
VeraCrypt. OpenBSD read-only crypto attachment is unsupported. After a crash,
FFS can require a manual filesystem check before a writable mount. Keep an
untouched copy before performing any repair.
Native packages carry private third-party engines; host kernel tools and FUSE
remain prerequisites, with macOS FUSE installed separately. A Cargo-only build
does not include every private native engine.

Desktop administrator elevation requires an administrator-owned installed helper
and backend, including their parent directories. A user-writable Cargo build is
suitable for login-user workflows; it is not automatically elevated through
pkexec/sudo. Install the verified package for privileged desktop operations.
