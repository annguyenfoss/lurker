# Arch UFS1 existing-configuration follow-up

The user explicitly started the previously deferred UFS1 task after ordinary
qualification finished. The nine completed filesystem rows remain closed.
This follow-up uses the current `7.3.6-arch1-1` kernel and its official matching
`linux-headers 7.3.6.arch1-1`; it introduces no kernel C or ABI changes.

The official source is https://github.com/archlinux/linux at tag
`v7.3.6-arch1`, commit `bba1a17262fbcd7bbc9c40220391c0765ff09c08`.
All fifteen `fs/ufs` files are retained with hashes and remain unmodified.
The sole selected option is existing upstream `CONFIG_UFS_FS_WRITE=y`.
The upstream Kconfig describes this writer as experimental; the stock module
has it disabled. Qualification uses fresh owned storage only.

Reproduce the matching module using the installed prepared headers:

```sh
make -C /usr/lib/modules/7.3.6-arch1-1/build M=/absolute/path/to/ufs-source \
  CONFIG_UFS_FS=m KCFLAGS=-DCONFIG_UFS_FS_WRITE=1 -j2 modules
```

The kernel documents `M=` and `KCFLAGS` at
https://docs.kernel.org/kbuild/modules.html and
https://docs.kernel.org/kbuild/kbuild.html. The full before/selected configuration
and one-line diff are retained. Compiler flags select the existing option for
this module; installed kernel/header configuration and source files are unchanged.
No forced vermagic, signature bypass or global policy change is used.

| Run | Purpose and inputs | Command/log reference | Outcome and cleanup |
| --- | --- | --- | --- |
| Preflight/work disk | Current kernel, headers, policy, RAM/disk, source tag | `preflight.json`, `work-volume-bind.json`, `ufs-source-manifest.json` | Matching headers/compiler; separate 16-GiB work disk bound by domain, source, actual serial, size and guest rdev. Original disks preserved. |
| Module build/install | Existing write option; exact official source | `module-build.json`, `module-build.log`, `config.diff`, `module-install.json` | Built as UID1300; SHA-256 `951dff4900f6e41c290457b0e037e5b1bdfad5afb1ce6e083f7508b4be400d14`; exact vermagic. Trusted `updates/lurker-ufs1` copy selected by normal depmod/autoload. Stock signed module matches the cached official package. |
| Native UFS1 | Three downloaded PDFs/text, VERSION=1, actual kernel `ufs`, both mount fields writable | `native-control/result.json` and phase logs | Write, normal close/reopen, append/new file, close/reopen and hash verification passed as UID1300. No native control mount remains. |
| Controller-only failures | Guard syntax, virtio serial length, path alias | `work-volume-bind.json`, `module-install.json`, retained operator transcript | First guard had a syntax error; the next readiness guard found virtio's 20-byte serial truncation. Both stopped before formatting. Install path-string comparison rejected `/lib` versus `/usr/lib` after copy/depmod; canonical same-file verification reconciled it. The subsequent authorized native mount normally autoloaded the correct module. No force loading or repeat source mutation. |

Raw evidence is private under
`target/goal-validation/native-fresh-20261127/hosts/archlinux/build/ufs1-config-20261127/`.
The live work directory is `/srv/build/lurker-ufs1-archlinux/workspace`.
Installed product source remains `e927e40bb235815343302677531d48f1afa4f939`.
Only affected UFS1 container, new scratch-device and representative frontend,
permission and ordinary-reboot cases are being qualified; no new pass is claimed
for unfinished cases.
