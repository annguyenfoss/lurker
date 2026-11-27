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

Device, helper, desktop and ordinary-reboot runs are in progress; no pass is claimed before their terminal receipts. A continuous 2 GiB root/build free-space observer is active. No failed storage or recovery credential is discarded.
