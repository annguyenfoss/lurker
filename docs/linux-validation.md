# Linux validation

This suite uses newly allocated files under `/tmp/lurker-*` and loop devices backed by those files. It never accepts a user volume as an argument. Cleanup checks a loop's backing-file identity before detaching it, because native VeraCrypt can detach a loop during unmount and Linux can reuse the number.

The existing personal vault `/dev/mapper/lurker_stk.luks_c27242cc3304` and `/home/stk/.luks_vault/mnt/stk.luks` are excluded from all test operations. Active-volume listing is exercised through the normal read-only inventory API; existing mappings are never mounted, unmounted, formatted, or written by these tests.

## Environment

Validation host: Arch Linux x86_64, kernel `7.2.11-arch1-1`, Rust/Cargo `1.98.1`, cryptsetup `2.8.8`, VeraCrypt `1.26.29`, btrfs-progs `7.1`.

## Repeatable commands

```bash
cargo check --workspace --locked
cargo test --workspace --locked
cargo build --workspace --release --locked
./scripts/test-linux-core.sh
python scripts/test-linux-frontends.py target/release
python scripts/test-linux-desktop.py target/release/lurker-desktop
python scripts/test-linux-desktop.py target/release/lurker-desktop --x11
```

The core runner requires cached or passwordless sudo credentials and compiles the exact current test executable before launching it. Tests are serial within that process, since failure injection temporarily changes its PATH. Its fixtures need cryptsetup, VeraCrypt, btrfs-progs, util-linux (including sfdisk/partx), e2fsprogs, dosfstools, and exfatprogs. These extra filesystem tools are test dependencies, not application runtime requirements. The default Cargo suite ignores the root tests explicitly; it does not silently pretend to run them.

The optional desktop smoke runner requires KWin, Xwayland and dbus-run-session, and uses private runtime/config/cache directories and a virtual framebuffer. Its X11 mode disables Xwayland hardware acceleration so the display stays entirely in software.

For an installed-package Wayland probe, use the desktop executable digest
from the verified package manifest. Run as the stated login user in the
qualification guest with Weston available.

```bash
python3 scripts/test-desktop-wayland.py \
  --binary /usr/bin/lurker-desktop --binary-sha256 "$INSTALLED_DESKTOP_SHA256" \
  --commit "$PRODUCT_SOURCE_SHA" --expected-uid "$(id -u)" \
  --weston /usr/bin/weston --output /tmp/lurker-wayland-receipt
```

The probe launches a private headless compositor with the Pixman renderer.
It records the Wayland configure acknowledgement and shared-memory buffer
commit for Lurker's own window, the compositor and executable hashes, and
the result of process and socket cleanup.

The frontend runner creates its own independent temporary files and uses a pseudoterminal for CLI prompts. It tests the release binaries, invokes the helper with sudo for JSON operations, and changes ownership only on its own newly created Btrfs root directory so the non-root CLI user can write test data.

## Coverage

| Suite | Scenarios |
| --- | --- |
| Create matrix | LUKS2 and normal VeraCrypt; AES, Serpent, Twofish; file and loop-backed block sources: 12 combinations; file length and permissions; header cipher/KDF inspection; direct native VeraCrypt interoperability |
| Lifecycle in every matrix case | Explicit and automatic type selection; wrong type and wrong password; write/remount/read persistence; Btrfs zstd compression; readonly mapper flag and rejected writes; busy unmount; duplicate mount; force-create protection; source, mountpoint and mapper unmount; source symlinks; LUKS tags; VeraCrypt tag rejection; no leftover temporary file |
| Failure recovery | Injected mkfs and mount failures; preservation of an existing destination; mapper cleanup; missing VeraCrypt and blkid; filesystem detection through lsblk; random invalid sources; failed lsblk inspection; mounted raw-device protection |
| External volumes | LUKS1 and SHA-256 VeraCrypt headers created outside Lurker; ext4 mounts without Btrfs options; persistence and readonly; opened but unmounted mapper recovery; missing paths; no passphrase/TTY; invalid size/type/passphrase; symlink create rejection |
| Holders | Refuse destructive create on a device with an active unmounted holder; leave unsupported mapper types open |
| Partitions | Fresh GPT disk/partition; both formats; create/mount/persistence/readonly/unmount; parent disk protected while a child has an active holder |
| Portable hosts | LUKS file on FAT and VeraCrypt file on exFAT; atomic no-clobber creation and forced replacement; persistence and readonly; secure host permission masks |
| Frontends | Actual CLI create password/confirmation/open prompts, bare relative filenames, automatic mount, readonly and unmount; helper JSON success, wrong password, missing password and malformed JSON; secrets absent from terminal output/helper logs |
| Desktop | Live release event loop on isolated Wayland and X11/Xwayland; form-to-command conversions; exact passphrase whitespace preservation; confirmation mismatch; password/keyfile exclusivity and LUKS credential forms; software rendering of create/mount/manage views for both formats at 100%, 125% and 150%; text zoom regression |

## Findings fixed

- Passwordless VeraCrypt header probing prevented automatic detection. Authentication now happens during TCRYPT open after checking for LUKS metadata.
- TCRYPT `--key-file -` mixed keyfile input with the password. Password-only volumes now receive the passphrase directly on stdin.
- Different native/fallback mount paths hid VeraCrypt mounts from the desktop and selected incompatible unmount paths. Both formats now use cryptsetup-managed `lurker_*` mappings.
- Cleanup registration before open could close an existing mapper after a duplicate mount failed. Cleanup now owns only successfully opened mappings.
- Readonly originally protected only the filesystem. It now protects the crypto mapper as well.
- Bare filenames failed parent-directory resolution; sources passed through symlinks outside `/dev` could fail device-mapper activation. Paths are resolved before crypto operations.
- LUKS create suppressed stdout, hiding terminal prompts. Interactive output is now inherited.
- VeraCrypt device creation passed `--size=max`, which failed for device-hosted volumes. Device size is now determined by VeraCrypt, as documented in the cipher directive.
- Block-device safety ignored unsuccessful lsblk commands and active unmounted holders. Inspection now fails closed and checks holders throughout the device tree.
- Desktop form validation trimmed passwords and confirmation independently. Secret values now preserve whitespace exactly.
- The desktop selected Skia despite the software-renderer requirement. It now selects winit/software, and the unused Skia feature is removed.
- Atomic no-clobber publication uses Linux `renameat2` through rustix, so it also works on FAT/exFAT hosts without hard links.
- Helper root detection trusted optional UID environment variables. It now reads the effective identity reported by the core.
- Early child-process input failure skipped reaping. The process is now reaped and crypto stderr is returned as a bounded error message.

## Scope and limits

Only Linux x86_64 is validated. Physical hardware is represented by dedicated whole-loop and GPT-partition fixtures; no personal or physical disk is used. Hidden volumes, custom PIM, system-encrypted Windows disks, and unsupported cipher algorithms are outside the exposed product interface. UI rendering is tested headlessly and the real winit/software event loop is smoke-tested in an isolated virtual display; desktop helper operations are tested through their JSON interface with sudo, without requiring an interactive polkit dialog.

LUKS Argon2 profiles retain their configured memory/time costs. The real matrix takes several minutes. Expected negative cases produce busy-unmount or invalid-filesystem messages; the final test process exit status and per-test results determine success.

`cargo clippy --workspace --all-targets` also runs as an advisory check. Current toolchain warnings in the existing SHA-256 chunk loop and desktop form argument count are unrelated to crypto behavior; strict `-D warnings` is not a project verification gate.

## Results

Validated on 2026-10-22 (Asia/Saigon):

- `cargo check --workspace --locked`: passed.
- `cargo test --workspace --locked`: 36 passed; six privileged tests explicitly ignored by this default command.
- `scripts/test-linux-core.sh`: all six privileged suites passed, including all 12 cipher/source combinations; 726.18 seconds; zero failures.
- Release CLI/helper frontend suite: passed for LUKS and VeraCrypt.
- Release desktop virtual-display smoke tests: passed on Wayland and X11/Xwayland.
- Shell syntax, desktop-entry validation and installed build/runtime dependency checks: passed.
- Post-test inventory: no loop devices or mounted filesystems backed by the disposable test directories remained.

Evidence is saved in `dist/validation/`.

## Verification after upstream integration

The newer macOS-support commit `f5f57ad` was integrated before pushing the Linux changes. The Linux implementation now lives in `crates/lurker-core/src/linux_workflow.rs`; shared cipher profiles and the platform dispatcher remain in `workflow.rs`.

- Workspace check and all 41 workspace tests passed on Linux.
- The full 12-combination Linux crypto/lifecycle matrix passed again in 494.61 seconds.
- The merged release build, interactive CLI/helper flows for both formats, and isolated Wayland desktop smoke test passed.
- The relocated Linux operation bodies and crypto profiles were compared against the previously validated implementation; explicit Apple-container rejection is retained on Linux.
- Upstream macOS backend, installer, platform models, and Slint UI files were preserved. Native macOS operations were not rerun from this Linux host.

Post-integration logs are saved as `dist/validation/rebase-*.log`.

## Credential implementation verification (2026-11-11)

All native operations below used disposable files and loop devices. No physical disk or pre-existing vault was changed.

| Check | Result |
| --- | --- |
| `cargo check --workspace` and `cargo test --workspace` | Passed; 56 default tests passed, with 12 native tests explicitly ignored by the default command |
| Privileged password matrix | All 12 LUKS/VeraCrypt × AES/Serpent/Twofish × file/loop-device cases passed, including persistence, readonly, duplicate/busy guards and cleanup; 489.65 seconds |
| Privileged keyfile matrix | All 12 equivalent keyfile-only cases passed, including wrong keyfile/password rejection, persistence, crypto/filesystem readonly and mounted LUKS credential addition/removal; 514.61 seconds |
| Userspace password matrix | All three ciphers passed; 48.91 seconds |
| Userspace keyfile matrix | All three ciphers passed, including stock cryptsetup header authentication; 138.06 seconds |
| LUKS1/LUKS2 credential matrix | All four existing/new password/keyfile combinations passed; independent slot authentication, wrong credentials, occupied/inactive slots, authenticated removal and last-slot protection; 380.62 seconds |
| LUKS slot limits and invalid requests | Full 8-slot and 32-slot headers, version-specific slot bounds, combined credentials, missing files, plaintext sources and last-bound-slot protection with an unbound slot all passed; failed operations preserved header bytes |
| CLI/helper frontend checks | Real terminal password entry, confirmation mismatch, automatic slot reuse, keyfile enrollment/removal, JSON requests, no password echo/logging, 20 immediate-paste attempts, empty input and cancellation passed |
| Desktop | Password/keyfile selection, all four credential conversions, secret clearing, old-helper capability rejection and software rendering of credential forms at 80%, 100% and 120% passed |
| macOS build checks | Core, CLI, helper and their tests cross-checked successfully for `aarch64-apple-darwin`; native macOS runtime cases were added but were not executed on this Linux host |

The terminal checks exposed and fixed a fast-paste echo race: the shared terminal reader now disables echo before writing its prompt. Relative and home-based desktop keyfile paths are resolved before helper elevation.

Additional repeatable commands:

```bash
cargo test -p lurker-core --test keyslots_native -- --ignored --test-threads=1
./scripts/test-linux-core.sh keyfile_round_trips
LURKER_USERSPACE_DIR=/usr/lib/lurker/userspace ./scripts/test-linux-userspace.sh keyfile_only_userspace_workflows
cargo build -p lurker-cli -p lurker-helper
python scripts/test-credentials.py target/debug
python scripts/test-credentials.py target/debug --prompt-only
cargo check -p lurker-core -p lurker-cli -p lurker-helper --tests --target aarch64-apple-darwin
```

The installed userspace helpers matched the builder's pinned upstream revisions. After the native runs, a read-only inventory found zero remaining credential-test mounts or loop devices. Native macOS verification requires a Mac with VeraCrypt and its matching FUSE runtime; run the existing `macos_native` suite there, including `veracrypt_keyfile_only_round_trips`.

## Packaged restart and credential qualification (2026-11-22)

Arch, Debian, Ubuntu, Fedora, openSUSE, and Gentoo each installed the
same native artifact used for their previous stock qualification, built
from `14ede46fb401578f0e8a4ee9b51e72ce17c0ad04`. Each installer and all
899 installed entries matched the recorded digests. The existing package
bytes were reused for every test in this round.

| Test | Result across six guests |
| --- | ---: |
| PDF and text lifecycle with an orderly restart while mounted | 42 routes passed |
| Binary-keyfile create and Auto lifecycle for all three file ciphers | 72 profiles and 936 checks passed |
| Guarded forced replacement on four file routes | 144 checks passed |
| Retained password and directory lifecycle receipts | 1,170 checks passed |
| Retained binary-keyfile block creation on fixture-owned loops | 54 profiles passed |
| Retained native desktop window probes | Six X11 and six Wayland passes |

The restart workload covered LUKS2, LUKS1, kernel and login-user VeraCrypt,
gocryptfs, CryFS, and Cryptomator. The first mount wrote and synced three
PDFs and a page of text. The next mount verified all four files, appended
text, and synced another file. The preflight captured all seven active
filesystems before each guest restarted. After the boot ID changed, all
seven fixtures reopened with matching bytes and closed normally.

For binary keyfiles, the installed CLI created each AES, Serpent, and
Twofish file profile for LUKS2, LUKS1, and both VeraCrypt routes. The Auto
cases checked incorrect keys, duplicate aliases, busy close, SQLite WAL
and filesystem operations, persistence, and read-only reopen. Forced
replacement covered the four file routes with disposable AES fixtures.

The login-user VeraCrypt identity check requires traversal of active
kernel loop backing paths. The fixture setup added traverse permission to
its private parent directories and kept ciphertext and credentials at
mode 0600. Ubuntu's AppArmor fusermount rules accepted the relocated
fixtures under the login user's home. The kernel policies stayed enabled.

The packages were removed and the package inventories restored. The cleanup
receipts confirm zero owned mounts, loops, and backend processes before
fixture removal. Package hashes, boot IDs, mounted payload digests, and
per-case logs are indexed in
`target/goal-validation/ssh-matrix-20261122/linux/matrix.json`. The abrupt
power-cut results are recorded in `docs/vm-powerloss-testing.md`.

### Updated Debian installer

The exact `9ff2b5b` Debian installer also passed all nine loop-device
profiles and rendered a window through both X11 and Wayland. The latter
committed a 1340×860 Wayland surface using offline Weston 14.0.2 inputs
in a private mount namespace. The probe passed. An explicit unmount
reported the test overlay busy, and namespace exit removed it. Follow-up
checks verified the original mounts, loop devices and package inventory
after the installer was removed.

The installer, installed desktop and results are hash-bound in
`target/goal-validation/ssh-matrix-20261122/cryptomator/source-review/debian-release/supplement/`.

### Coordinated Linux installer qualification

Arch, Fedora, Ubuntu, openSUSE and Gentoo each installed their native
`16aa1f4` package offline, matched all 923 installed entries, and passed
195 lifecycle checks and nine binary-keyfile loop-device profiles. On
each guest the Cryptomator append returned `ENOSPC`, preserved committed
data, and accepted new writes after freeing space. Each guest rebooted
with the Cryptomator mount active, then reopened and matched all five
payload digests. Package removal restored the original inventories and
cleared the test scopes.

All five exact installed desktop binaries passed X11 and Wayland. The
display probes used archived compositor dependencies in private mount
namespaces. Gentoo used the openSUSE Weston fixture. The openSUSE fixture
used an Ubuntu `libxcb-render` library. These dependencies belonged to the
test environment. The installed application hashes and fixture sources
are recorded in the per-host receipts.

Including the `9ff2b5b` Debian installer, the updated Linux packages
passed 1,170 lifecycle checks, 54 loop-device profiles, six mounted
Cryptomator reboots, and six each of the X11 and Wayland probes. The
commits between the Debian and five-host builds changed documentation.
See `target/goal-validation/ssh-matrix-20261122/linux/final-native/matrix.json`
for native installers, manifests, artifact hashes, and runtime receipts.
