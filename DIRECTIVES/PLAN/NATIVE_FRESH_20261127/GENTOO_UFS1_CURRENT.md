# Gentoo current-kernel UFS1 configuration qualification

This user-authorized follow-up enables the existing upstream `CONFIG_UFS_FS_WRITE` option for a matching UFS module on the current/default `7.3.6-gentoo-dist-bin` kernel. It is separate from the passed older `6.18.64` cohort. No Lurker kernel C, ABI, hook or driver behavior was invented. The nine completed filesystem rows are unchanged.

## Inputs and reproducible build

The installed Gentoo `gentoo-kernel-bin-7.3.6` supplies the prepared headers, configuration and `Module.symvers`. GCC matches its recorded compiler: Gentoo 16.2.1_p20260926. The existing configuration has `CONFIG_UFS_FS=m` and write support disabled. The isolated module selects only `CONFIG_UFS_FS_WRITE=y`; stock headers/configuration and the vendor module remain unchanged.

Cached upstream/Gentoo inputs were verified against both BLAKE2B and SHA512 in the signed-synchronized Gentoo repository's `sys-kernel/gentoo-kernel/Manifest`:

| Input | SHA-256 |
| --- | --- |
| `linux-7.3.tar.xz` | `87363c24f10e06321eb362765521094e94471f6bff8bce2d6b3e40f39f004953` |
| `patch-7.3.6.xz` | `11f630161b995ae31bbf79e9056e22b291bd47a389c9ad542091b4f342616718` |
| `linux-gentoo-patches-7.3.5_p1.tar.xz` | `96a3876892e5bb487a51f5ea2a75f3e833b7fa1f0d62c9c55a07de2c267a2fff` |

Extract `linux-7.3/fs/ufs` and `Documentation/admin-guide/ufs.rst` from the verified source archive. Apply the existing stable 7.3.6 patch's `fs/ufs/dir.c` section without changing its hunks; the Gentoo patchset has no UFS changes. The executed controller retains the complete parent patch hash, exact selected patch and all resulting source-file hashes. `Kconfig` and `Makefile` match the prepared Gentoo headers.

Copy the prepared `/usr/src/linux-7.3.6-gentoo-dist-bin` tree into a new normal-user build directory, preserving its generated configuration and build artifacts. With `headers` naming that copy and `ufs` naming the prepared source's `fs/ufs` directory, the executed build is:

```sh
make -C "$headers" M="$ufs" CONFIG_UFS_FS=m CONFIG_UFS_FS_WRITE=y \
    KCFLAGS=-DCONFIG_UFS_FS_WRITE=1 -j1 V=1 modules
```

`KCFLAGS` supplies the selected existing option to this isolated module while leaving the stock generated `autoconf.h` unchanged. `config.diff` records the effective module selection, not a rebuilt boot-kernel configuration. All UFS source and original header/configuration hashes matched after compilation. The normal-user build produced `ufs.ko` SHA-256 `56347fbf63e0ae5d02251fa6cd1801767284ff11831f57d82e48ebd84300a369`, with exact running-kernel vermagic.

An administrator verified/copied that module to `/lib/modules/7.3.6-gentoo-dist-bin/updates/lurker-config/ufs.ko`, kept the vendor original and ran ordinary `depmod -a 7.3.6-gentoo-dist-bin`. Normal resolution selects the configured module; catalogues are 0644. The initial load used normal `insmod` with no force option. No signature enforcement, lockdown, global policy or boot configuration was weakened. No full kernel build was needed.

Established guidance and source selection:

- https://docs.kernel.org/kbuild/modules.html — matching prepared kernel, external `M=` build and module installation.
- https://docs.kernel.org/kbuild/kbuild.html — supported `KCFLAGS` compiler parameters.
- https://raw.githubusercontent.com/torvalds/linux/master/fs/ufs/Kconfig — existing write-support option; the retained exact-version Kconfig is authoritative for this build.
- https://cdn.kernel.org/pub/linux/kernel/v7.x/linux-7.3.tar.xz
- https://cdn.kernel.org/pub/linux/kernel/v7.x/patch-7.3.6.xz
- https://distfiles.gentoo.org/pub/proj/dist-kernel/patchsets/7.3/linux-gentoo-patches-7.3.5_p1.tar.xz

## Per-run notes

Raw evidence is separate under remote `/srv/lurker-native-fresh-20261127-gentoo/ufs1-current-7.3.6/` and local `target/goal-validation/native-fresh-20261127/hosts/gentoo/build/ufs1-current/`. Product runs use the installed exact `e927e40bb235815343302677531d48f1afa4f939` artifacts; their bytes are unchanged.

| Run / purpose | Command or receipt/log reference | Observed result / cleanup |
| --- | --- | --- |
| Preflight | `preflight.json` | Idle; matching 7.3.6 package/headers/compiler; 3.9 GiB RAM, root 2.32 GB and build 15.1 GB free. No UFS mount/module active. |
| Verify source and build configured module | `build-configured-ufs.py`, `build-result.json`, `command-0..2.log`, `config.diff` | Passed as UID 1300; exact inputs and unchanged source/stock-header checks retained. |
| Initial harness setup | `setup-failure-1.json` | Wrong helper filename stopped setup; partial owned harness directory only. No module installed or loaded at that point. Corrected owned setup completed. |
| Trusted module installation | `install-configured-ufs-resume.py`, `install-result.json`, `depmod.log` | Passed; vendor module preserved and normal configured-module selection verified. |
| Native UFS1 control | `native-control/result.json`, three payload logs | Passed UID-1300 full downloaded PDF/text write/normal close/append/normal close/reopen. Actual kernel `ufs` mount is rw with `ufstype=44bsd`; independent version-1 signature. No mount remained. |
| Feature sample | Native control `features` | Ownership 1300:1300 and initial 0700, chmod 0640 succeeds; ACL and reflink unsupported; journaling unknown. |
| Six container cells | `containers/matrix.json`, per-case receipts | LUKS2/LUKS1/VeraCrypt plus gocryptfs/CryFS/Cryptomator on native UFS1 all passed; full payload, Auto reopen and independent version-1 evidence. Passwords/keys and ciphertext retained. Folder backend override names the same root-controlled installed package. |
| Device prelaunch guard | `device-prelaunch-guard-correction.json` | A missing optional cleanup-errors key stopped the read-only guard before any child or device write; corrected guard admitted the clean six-container result. |
| Scratch identity | `scratch-domain-identity.json`, per-create guards | Existing authorized 2 GiB qcow2, domain UUID `e213747f-89c4-45d3-a430-6130c17ad73b`, serial `lurker-fresh-gentoo`, target vdc. No new disk. Root/build disks preserved. |
| Six physical cells | `device-inner/device-matrix.json`, `device-backing/device-backing-matrix.json` | All three disk methods and all three folder engines passed full payload/normal close/reopen. Physical UFS1 backing was natively unmounted/remounted between phases. Plaintext folder mountpoints used the ordinary ext4 work filesystem. Narrow VeraCrypt Quick Format confirmation was restricted to this verified scratch device. |
| Strict helper keyfiles | `helper-files/result.json`, `helper-device/result.json` | Six flows, 42 complete stdout-JSON actions passed; Auto reopen, five payload hashes, version-1 identity and UID/GID 1300:1300/0700 verified. Keys/ciphertext retained. |
| Cipher/credential sample | `variations/result.json` | LUKS2 Serpent/password and native VeraCrypt Twofish/keyfile passed fourteen CLI actions. AES is covered by the matrix. This is a representative sample, not every cipher/credential cross-product. |
| Receipt read | Native-control receipt access | Normal-user reading of a root-0600 JSON receipt failed with permission denied; administrator read it without changing permissions. No product failure. |
| GUI attempt 1 | `ui-first-attempt/`, `ui1-retention-exception.json` | Early selector did not select UFS1; underlying default-Btrfs callback returned success, then the external validator rejected the missing proof/row marker. Original successful-test cleanup had already removed that Btrfs ciphertext/password. Logs, binary and overlay remain; no UFS1 pass credited. This retention exception is explicit. |
| GUI attempt 2 | `ui-second-attempt/`, `desktop-fixtures-v2/gui-48094/` | Late selector assertion passed, independent real-mount check found Btrfs and rejected it. Normal helper close removed mount/mapper. Ciphertext/password retained. Linux ignores this selector by design. |
| GUI attempt 3 preparation | `prepare-ui-v3.py`, corrected `prepare-ui-v3-fixed.py` and logs | Snapshot input naming caused FileNotFound before build/test. Corrected private orchestration only. |
| GUI attempt 3 | `ui-third-attempt/`, `desktop-fixtures-v3/gui-51240/` | Desktop bridge rejected an explicit filesystem before creating the source volume. Generated synthetic password retained; no ciphertext existed. |
| Supported GUI Auto callbacks | `ui-v4-gates/native-desktop-callback.json`, `.log`, `ui-test-overlay-v4.patch` | Passed real Slint/winit callbacks as UID 1300 using the installed helper: separately helper-created UFS1 fixture, GUI Auto open/normal close/reopen, payload verification/normal close. Two independent kernel `ufs`/VERSION=1 mount proofs. Linux GUI UFS1 create selection is not applicable: its create default is fixed; the selector is implemented only for DragonFly VeraCrypt. Test-only orchestration never changed release bytes. |
| Initial preboot collection | `preboot-collector-first-attempt.json` | Administrator traversal met user-FUSE EACCES. Corrected collector pruned exact live mountpoints before stat/walk; no FUSE policy change. |
| Preboot retention | `ufs1-current-preboot-receipts.tar.gz`, `preboot-archive-verification.json`, `closed-ui-archive-verification.json` | Receipt and closed UI ciphertext/password archives transferred; every member hash and entire gzip stream verified before reboot. |
| Ordinary reboot | `reboot/reboot-batch.json`, `postboot-build-mount.json`, `reboot-prepare-launch.json` | Six disk-file/folder-on-physical-backing fixtures passed; all thirty pre/post hashes equal and all normal closes succeeded. New boot `fd026475-bfa6-4fec-91f5-c4ef187a4e0b`, same 7.3.6 kernel. Existing work disk was not automounted; UUID/serial/size verified before ordinary remount, never formatted. Normal UFS autoload selected the configured module. First backing-mount controller hash was recorded after its first mount, explicitly labeled; no universal prelaunch-hash claim. |
| Final validation/cleanup | `final-validation.json`, `final-validate-controller.log` | Passed all 1,085 installed manifest entries, product hashes, unchanged boot/header inputs and live module build ID `a7c2bb531f152fe8341ffc3d1593bec50d1921e2`. No owned mounts, loops, task processes or scratch children/mounts/holders remain. Configured module stays installed/loaded; vendor original preserved. |
| Completion-index construction | Local `index-completion.py` and retained ledger | First read-only reconciliation asserted on historical `luks2` versus CLI `luks` naming before writing an index; added the explicit alias, then coverage/count assertions passed. No host operation. |
| Final evidence collection | `collect-final.py`, `ufs1-current-final-inventory.json`, local `final-archive-verification.json` | 532 files, including closed ciphertext/credentials, source/config/module, controller attempts and test binaries. Before/after source hashes stable; local archive/member size/hash/mode and full gzip CRC checks passed. Archive SHA-256 `24b3cae4912c31919a0a9ad31601b45cf36f4d800e4f7c8a591ff4abdf9fd26a`. Only reproducible headers/UI source/UI Cargo caches excluded; originals remain on guest. |

Both continuous resource observers finished successfully; root/build stayed above 2 GiB. Final validation recorded root 2,316,836,864 bytes and build 7,445,237,760 bytes free before the 3,571,101,920-byte final archive. Collection independently maintained the floor. No new disk, full kernel, boot-image or product rebuild was needed. A read-only final collector inspection found remote `rg` unavailable; archive inventory and local integrity verification established completion instead.

The new private `ufs1-current-completion-evidence-index.json` reconciles this follow-up with the historical 130-cell dimensions without relabeling 576/678 artifacts. The earlier e927 report is retained as `build/ufs1-current/GENTOO_RESULTS-at-af002c4.md`; its old index resolves that immutable report via commit `af002c4`. No current-task gate remains deferred. All failed-attempt evidence is retained except the explicitly disclosed first default-Btrfs GUI ciphertext/password.
