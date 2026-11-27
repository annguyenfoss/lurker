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
| Module build/install | Existing write option; exact official source | `module-build.json`, `module-build.log`, `config.diff`, `module-install.json` | Built as UID 1300; SHA-256 `951dff4900f6e41c290457b0e037e5b1bdfad5afb1ce6e083f7508b4be400d14`; exact vermagic. Trusted `updates/lurker-ufs1` copy selected by normal depmod/autoload. Stock signed module matches the cached official package. |
| Native UFS1 | Three downloaded PDFs/text, VERSION=1, actual kernel `ufs`, both mount fields writable | `native-control/result.json` and phase logs | Write, normal close/reopen, append/new file, close/reopen and hash verification passed as UID 1300. No native control mount remains. |
| Controller-only failures | Guard syntax, virtio serial length, path alias | `work-volume-bind.json`, `module-install.json`, retained operator transcript | First guard had a syntax error; the next readiness guard found virtio's 20-byte serial truncation. Both stopped before formatting. Install path-string comparison rejected `/lib` versus `/usr/lib` after copy/depmod; canonical same-file verification reconciled it. The subsequent authorized native mount normally autoloaded the correct module. No force loading or repeat source mutation. |

Raw evidence is private under
`target/goal-validation/native-fresh-20261127/hosts/archlinux/build/ufs1-config-20261127/`.
The executed work directory was `/srv/build/lurker-ufs1-archlinux/workspace`.
Installed product source remains `e927e40bb235815343302677531d48f1afa4f939`.
Only affected UFS1 cases were run; the prior nine-filesystem receipts retain
their original source identities. This follow-up is complete and idle.


## Terminal affected-case results

All references below are members beneath `workspace/` in the private
`workspace-complete.tar.gz`, or extracted copies beneath `receipts/`.
The index is `completion-evidence-index.json` in the evidence root above.

| Run | Purpose and inputs | Command/log reference | Outcome and cleanup |
| --- | --- | --- | --- |
| Containers | Installed e927 CLI; LUKS2/LUKS1/VeraCrypt inner UFS1 and three directory engines on UFS1 backing | `run-containers.py`, `container-dispatcher.log`, `containers/matrix.json`, per-case logs | 6/6 passed as login UID 1300. Downloaded payload write, normal close/reopen, append/new file, close/reopen and five hashes; actual VERSION1 and configured module recorded. Synthetic factors retained privately. |
| New physical inner filesystems | Exact new 1-GiB disk serial `LURKER-UFS1-ARCH-DEV`, e927 CLI | `ufs1-device.py`, `physical-inner-launch.json`, `physical-inner/device.json` | LUKS2/LUKS1/VeraCrypt 3/3 passed. Guarded device identity and normal release recorded. These three native payload runs used UID 0; they are not login-user I/O proof. |
| New physical directory backing | Same identified scratch disk, native UFS1 backing, three directory engines | `ufs1-folder-device.py`, `physical-folder/folder-device.json`, `physical-folder/ufs1/preserved-logs/` | 3/3 passed with UID 1300 CLI and payload. Ciphertext and private factors retained; console logs copied outside the backing before later operations. |
| Permissions | Actual physical UFS1, UID 1300 and foreign UID 65534/groups[] | `permissions.py`, `permissions.json` | chmod 0640 persisted; chown 65534:65534/0600 persisted across native close/reopen and denied original caller read. Foreign list/read/write/create returned EACCES through a searchable `/tmp` parent and private filesystem root. Ownership restored. |
| Generic GUI callback attempt | Existing native callback fixture on physical UFS1 | `gui-callback.py`, `gui-callback.log`, `gui-callback-observation.json`, `failed-gui-fixture-preservation.json` | Failed: host `fusermount3` forbids a plaintext FUSE mountpoint on UFS magic `0x11954`; backend exited 19. Encrypted fixture, factor and failed session were preserved before reboot. No FUSE policy change. |
| Actual installed GUI | Physical-UFS1-backed gocryptfs; allowed ext4 plaintext mountpoint | `start-window.py`, `window-action.py`, `actual-window/` logs/screenshots and `result.json` | Observed UID/EUID 1300; actual row close, password reopen, five hashes, write/fsync and normal row close passed. The additional desktop-written file also survived the ordinary reboot. This does not claim plaintext FUSE mounting directly on UFS. |
| Strict helper and reboot | LUKS2/Twofish/binary keyfile, VeraCrypt/Serpent/password, physical-UFS1-backed gocryptfs | `helper-cohort*-executed.py`, `helper-cohort.py`, `helper-reboot/result.json`, complete per-action stdout/stderr | 3/3 representative cohorts and 25 strict complete-stdout JSON actions passed as caller UID/EUID 1300. Normal reboot changed boot ID; same default kernel returned. Module was absent before the first postboot UFS mount and normally autoloaded. Old payload hashes, append/new file, close/reopen and desktop file persistence passed. |
| Additional controller failures | Read-only checkpoint and pending postboot folder continuation | `later-controller-failures.json`, `helper-postboot-dispatcher-first-failure.log`, `helper-reboot/result-first-postboot-harness-failure.json`, retained phase sources | Missing `hashlib` import failed after a normal backing close; corrected metadata-only continuation passed. First postboot extra-file worker returned non-JSON bytes after two native cohorts had passed; those completed cohorts were not repeated. Corrected string-return worker continued the already-mounted folder cohort, which passed and normally closed. |
| X11 input corrections | Existing actual window | `actual-window/` action receipts/screenshots | Initial stale window ID caused BadWindow; initial focus/geometry inputs were corrected before the successful unlock. Current controller bytes are retained, but not every transient failed automation version. No failure is counted as a product pass. |
| Final validation/export | Closed products and exact input/artifact bytes | `final-validation.json`, `workspace-export-verification.json`, `physical-export-verification.json`, `official-package-retention.json` | No owned product processes, UFS mounts or mappers; module references 0, package verification 1744 files/0 altered. Complete archive gzip CRC, every member hash/size/mode, unchanged source before/after, and closed physical image hash/size verified. Official kernel/header packages and signatures retained. |
| Final cleanup | Only the new work and physical disks | `finish-cleanup.py`, `guest-terminal-cleanup.json`, `terminal-cleanup.json`, terminal domain XML | Zero work-volume process references; normal work unmount. Both new qcow2 disks detached live and persistently; original disk inventory unchanged. Images remain private 0600 with hashes. Configured matching module and original signed stock module remain installed; no signature or global policy change. Completed 2026-11-27 07:19:50 UTC, no jobs or deferred Arch work. |

The full workspace archive SHA-256 is
`4631c1a2276a9e007d90c4c893df58c1d918c936b1735d1403ae149509c863f9`.
The closed physical raw image SHA-256 is
`986aa90684b98179a099bb092b4fa614f805afff85fec4a0f4430693bace87cd`;
its gzip archive SHA-256 is
`8c6cf22ff9ca5e5161e57d6d3231c6d0ea78d6727465f68b05c28e6f1d10d44c`.
This proves normal lifecycle and ordinary reboot for the stated cases, not
power-loss durability or a replacement for historical missing evidence.
The upstream experimental writer requirement and plaintext-on-UFS FUSE
restriction remain explicit. No new kernel code, ABI, hook or driver logic was
introduced; only the existing write option was selected.
