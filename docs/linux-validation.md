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
| Desktop | Live release event loop on isolated Wayland and X11/Xwayland; form-to-command conversions; exact passphrase whitespace preservation; confirmation mismatch; unsupported keyfile auth; software rendering of create/mount/manage views for both formats at 100%, 125% and 150%; text zoom regression |

## Findings fixed

- Passwordless VeraCrypt header probing prevented automatic detection. Authentication now happens during TCRYPT open after checking for LUKS metadata.
- TCRYPT `--key-file -` mixed keyfile input with the password. Password-only volumes now receive the passphrase directly on stdin.
- Different native/fallback mount paths hid VeraCrypt mounts from the desktop and selected incompatible unmount paths. Both formats now use cryptsetup-managed `lurker_*` mappings.
- Cleanup registration before open could close an existing mapper after a duplicate mount failed. Cleanup now owns only successfully opened mappings.
- Readonly originally protected only the filesystem. It now protects the crypto mapper as well.
- Bare filenames failed parent-directory resolution; sources passed through symlinks outside `/dev` could fail device-mapper activation. Paths are resolved before crypto operations.
- LUKS create suppressed stdout, hiding terminal prompts. Interactive output is now inherited.
- VeraCrypt device creation passed `--size=max`, which failed for device-hosted volumes. Device size is now determined by VeraCrypt; the archived profile and cipher directive are synchronized.
- Block-device safety ignored unsuccessful lsblk commands and active unmounted holders. Inspection now fails closed and checks holders throughout the device tree.
- Desktop form validation trimmed passwords and confirmation independently. Secret values now preserve whitespace exactly.
- The desktop selected Skia despite the software-renderer requirement. It now selects winit/software, and the unused Skia feature is removed.
- Atomic no-clobber publication uses Linux `renameat2` through rustix, so it also works on FAT/exFAT hosts without hard links.
- Helper root detection trusted optional UID environment variables. It now reads the effective identity reported by the core.
- Early child-process input failure skipped reaping. The process is now reaped and crypto stderr is returned as a bounded error message.

## Scope and limits

Only Linux x86_64 is validated and declared by the local AUR recipe. Physical hardware is represented by dedicated whole-loop and GPT-partition fixtures; no personal or physical disk is used. Hidden volumes, custom PIM, keyfiles, system-encrypted Windows disks, and unsupported cipher algorithms are outside the exposed product interface. UI rendering is tested headlessly and the real winit/software event loop is smoke-tested in an isolated virtual display; desktop helper operations are tested through their JSON interface with sudo, without requiring an interactive polkit dialog.

LUKS Argon2 profiles retain their configured memory/time costs. The real matrix takes several minutes. Expected negative cases produce busy-unmount or invalid-filesystem messages; the final test process exit status and per-test results determine success.

`cargo clippy --workspace --all-targets` also runs as an advisory check. Current toolchain warnings in the existing SHA-256 chunk loop and desktop form argument count are unrelated to crypto behavior; strict `-D warnings` is not a project verification gate.

## Results and local package

Validated on 2026-10-22 (Asia/Saigon):

- `cargo check --workspace --locked`: passed.
- `cargo test --workspace --locked`: 36 passed; six privileged tests explicitly ignored by this default command.
- `scripts/test-linux-core.sh`: all six privileged suites passed, including all 12 cipher/source combinations; 726.18 seconds; zero failures.
- Release CLI/helper frontend suite: passed for LUKS and VeraCrypt.
- Release desktop virtual-display smoke tests: passed on Wayland and X11/Xwayland.
- Shell syntax, desktop-entry validation, PKGBUILD metadata and installed build/runtime dependency checks: passed.
- Post-test inventory: no loop devices or mounted filesystems backed by the disposable test directories remained.

Evidence is saved in `dist/validation/`. The local package is built from the current worktree snapshot with `scripts/build-local-aur.sh`; its fresh output directory contains the PKGBUILD, `.SRCINFO`, source snapshot, build/check/package logs and package archive. No AUR publication or system installation is performed.

## Verification after upstream integration

The newer macOS-support commit `f5f57ad` was integrated before pushing the Linux changes. The Linux implementation now lives in `crates/lurker-core/src/linux_workflow.rs`; shared cipher profiles and the platform dispatcher remain in `workflow.rs`.

- Workspace check and all 41 workspace tests passed on Linux.
- The full 12-combination Linux crypto/lifecycle matrix passed again in 494.61 seconds.
- The merged release build, interactive CLI/helper flows for both formats, and isolated Wayland desktop smoke test passed.
- The relocated Linux operation bodies and crypto profiles were compared against the previously validated implementation; explicit Apple-container rejection is retained on Linux.
- Upstream macOS backend, installer, platform models, and Slint UI files were preserved. Native macOS operations were not rerun from this Linux host.

Post-integration logs are saved as `dist/validation/rebase-*.log`.
