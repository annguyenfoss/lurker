# Ubuntu UFS1 configured-module qualification

The user explicitly started the previously deferred UFS1 work after the nine
other filesystem rows completed. Existing configuration only: no Lurker kernel
C, ABI, hook or driver logic was added. Historical stock-UFS1 failures remain;
these are new conditional write-enabled-module observations, using installed
Lurker `e927e40bb235815343302677531d48f1afa4f939` without a product rebuild.

## Source and reproducible build

- Running/default kernel: Ubuntu `7.3.0-11-generic`; matching image, modules and
  headers package `7.3.0-11.12`; matching GCC `15.3.0-4ubuntu1`.
- Exact Ubuntu `linux=7.3.0-11.12` source downloaded as UID1300 using isolated
  `deb-src` apt indices authenticated by the installed Ubuntu archive keyring.
  Signed Sources SHA512 binds all downloaded files, including the `.dsc`;
  `.dsc` SHA256 binds the original tar and Ubuntu diff. The absent individual
  packager key warning from `dpkg-source` remains in the extraction log.
- Original archive SHA256:
  `f8d82362396a3a7373c3f6c2ae601e2fcc3d7ed81e03e6e10c76c648a4b52ed0`.
  Ubuntu diff: `ce7059a9614b93198ab3fb8e07e5a9fae6d2ff9d4d8a8370eb40ecdb206d1244`.
- Copy unchanged `fs/ufs` from the extracted exact Ubuntu source to an owned
  module directory. Keep `CONFIG_UFS_FS=m`; select the existing
  `CONFIG_UFS_FS_WRITE=y` option for that module. It remains labelled
  DANGEROUS/experimental by upstream; this report does not change that label.

Run as the login user, substituting the absolute copied-module directory:

```sh
make -C /lib/modules/7.3.0-11-generic/build M=/absolute/owned/ufs \
  CONFIG_UFS_FS=m CONFIG_UFS_FS_WRITE=y \
  KCFLAGS=-DCONFIG_UFS_FS_WRITE=1 -j1 V=1 modules
```

The compiler define selects the existing guarded implementation without editing
installed generated headers. Source C/headers/Makefile hashes, global config,
installed headers and original signed module were unchanged. All 149 required
symbol CRCs and stock vermagic matched; the new module exports no symbols.
BTF generation was skipped because packaged headers lack `vmlinux`; the build
completed. Module SHA256:
`6e0dc1a64355b3c808372db6133e5215006159701a87e6705b0a1cc3f5428f27`.

Install only the hash-verified root-owned module under
`/lib/modules/7.3.0-11-generic/updates/lurker-ufs-write/ufs.ko` (0644, protected
0755 parent), run `depmod` with standard umask022, then normal `modprobe ufs`.
No force-vermagic, signature bypass, policy change or kernel replacement was
used. Existing lockdown and signature enforcement were unchanged. The stock
signed module/kernel remain available. Loaded build-ID-note SHA256:
`ab779b2ac264237faac808277e98cb60482d851ab5009e0c03df2ae44b2f2b18`.

Established upstream guidance:
https://docs.kernel.org/kbuild/modules.html
https://docs.kernel.org/kbuild/kbuild.html#kcflags
https://docs.kernel.org/admin-guide/ufs.html

## Run notes and retained outcomes

| Run | Purpose / command reference | Outcome and cleanup |
| --- | --- | --- |
| Exact-source preflight | `preflight-source.py`; package/config/headers/source/free-memory/disk inventory and isolated apt source download/extraction | Source ready; initial available RAM3.56GB, root5.25GB/work19.1GB. Source extraction stayed above2GiB floor. |
| Initial build controller | `build-module.py` | Refused before compilation because it expected SHA256 in signed Sources, which supplies SHA512. Original receipt retained. |
| Matching module | `build-module-v2.py`; command above | Passed source authentication, one-worker build,149 symbol CRC/vermagic checks and unchanged-input checks. |
| Initial install controller | `install-and-native-proof.py` | Halted before load/format: private umask077 made regenerated module catalog0600, so normal-user modinfo could not read it. Root lookup identified the installed module. |
| Native control continuation | `install-native-proof-v2.py`; standard umask022 depmod, normal modprobe, makefs-freebsd version1, native `ufstype=44bsd` mounts | Passed three downloaded PDF/text write/append/reopen phases as actualUID1300; initial private root ownership/mode retained. All mounts closed. Modes/ownership/hardlinks yes; xattr/ACL probes errno95; no CoW/journal claim. |
| Container batch | `run-containers.py` + independently identified UFS1 payload wrapper | Six passed: LUKS2/LUKS1/VeraCrypt inner; gocryptfs/CryFS/Cryptomator UFS1 backing. Seven actions each, Auto reopen, per-phase actualUID1300/module/superblock proof; initial1300:1300/0700; clean closes. |
| New physical batch | `run-physical.py` | Started only after container batch terminal. New2GiB qcow2 `ubuntu-ufs1-config-20261127.qcow2`, serial `lurker-ufs1-ubuntu`, domain UUID verified, all original four disks preserved. Three separate encrypted partitions plus a fourth native UFS1 folder backing; result pending. |

Private receipts, scripts, launch hashes, source/config diff and raw logs:
`target/goal-validation/native-fresh-20261127/hosts/ubuntu/build/ufs1-config-20261127/`.
The first 41 source/build/native-control receipts are preserved in
`build-native-receipts.tar.gz`, SHA256
`0ad7dd769a7810547e9350bbe09a6a633a2934f9f8e31029e27642b312427ddd`.
Successful native images, all credentials, failures and original kernel bytes
remain retained. Physical, frontend/helper and appropriate reboot follow-up
results will be recorded separately; no repeat of unrelated nine-filesystem
qualification is planned.

## Completion — 2026-11-27

The physical batch is now terminal: six passed. LUKS2, LUKS1 and VeraCrypt each
completed seven strict-helper JSON actions on separate new partitions, including
all three payload phases. The three folder engines each completed seven CLI
actions on the fourth native UFS1 partition. Every payload phase recorded actual
UID1300, UFS1 superblock/module identity and private root metadata. Raw device
node ownership/modes remained unchanged. The common CLI harness resolves its
supplied public launcher before execution; direct helper calls are separate.

| Follow-up | Inputs / command-log reference | Outcome / cleanup |
| --- | --- | --- |
| Installed desktop | `prepare-desktop.py`, `start-window.py`, `desktop-close-observe.py`; `desktop-ready.json`, `desktop-result.json`, native XWD snapshots | Actual UID1300 installed window displayed read/write UFS on the owned physical LUKS1 volume and closed it normally through “Unmount & lock.” Metadata observer captured the default installed normal helper and elevated `--as-root` helper; no test overlay. Idle desktop/Xvfb subsequently closed. |
| Initial reboot recipe | `reboot-cohort.py`; `reboot-containers-prepare-supervisor.log` | Syntax refusal before any fixture work. Corrected recipe received native `py_compile`, was separately hashed, and supplied both successful cohorts. |
| Container reboot cohort | `reboot-cohort-v2.py prepare/verify containers`; `reboot/containers/manifest.json` | Six mounted fixtures, five payload hashes plus fresh fsynced marker each; ordinary reboot; all six Auto reopens verified and closed. Minimum available RAM2,442,870,784B. |
| First SSH monitor | `ordinary-reboot.py`, `postboot-resume-containers.json` | First10s readiness probe timed out during startup. Fresh TCP probe found the new boot; identity-fenced storage restoration resumed without issuing another reboot. Original failure retained. |
| Physical reboot cohort | `reboot-cohort-v2.py prepare/verify physical`; `ordinary-reboot-v2.py`; `reboot/physical/manifest.json` | Same six-method mounted cohort on new partitions; all payloads/markers verified after a second ordinary reboot, then closed. Minimum available RAM2,543,341,568B. |
| Permission denial | `final-audit.py`; `final-audit.json` | Owner UID1300 read passed; UID/GID65534 with groups cleared was denied. `/tmp` and the empty mount underlay were traversable0755, while the actual UFS1 root was0700 and file0640. Native mount closed and empty underlay removed. |
| Final integrity/cleanup | `final-audit.py`; `final-audit.json` | All1,085 installed package entries and dpkg verification passed; original stock module/config/headers unchanged; signature policy unchanged. Zero active Lurker processes, UFS/FUSE fixture mounts, loops or mappers. Root5,240,143,872B/work14,747,504,640B free. |

Ordinary boot IDs, in order:
`e4d75e19-6027-47ff-9b33-6f4add5a81f3` →
`81ee2627-c962-49c1-9730-2fba85b7b4fb` →
`83796410-96aa-48a2-a37c-1d161b3e6848`.
Both boots retained the same default `7.3.0-11-generic` kernel. The configured
module was initially absent and autoloaded through the normal native mount path;
its build identity matched on every verified phase. Each six-fixture cohort was
admitted against available RAM before startup, with LUKS2 opened first, live
memory/disk checks and no compiler/PBKDF overlap.

The unsigned/out-of-tree module uses the host's pre-existing permissive signature
policy; no enforcement was weakened. A future kernel upgrade requires another
matching build. Kernel mount diagnostics report the UFS1 timestamp limit as
2038-01-19; raw diagnostics are retained. No journal/CoW or deep crash-safety
claim follows from these bounded payload, normal-close and ordinary-reboot tests.

The final private `completion-evidence-index.json` has SHA256
`085c23964f136747d89d3b2a02669118545c4df2d07e8a9b699492645af334c5`.
It indexes387 retained files and two append-only130-cell source-kind ledgers:
60 qualified cells (including these six configuration-qualified UFS1 cells) and
70 not-applicable per ledger. The original54 passes and their source identities
are carried unchanged; original stock-UFS1 failures remain explicitly preserved.
The final archive SHA256 is
`46d81f22089fcc39cb38036d29ce878cad9e68eec7455749c0c7fffbfa664c7d`.

All required Ubuntu UFS1 work is complete; there is no remaining agent blocker,
required deliverable or external dependency. Evidence/source/cache disks remain
mounted intentionally; the new2GiB scratch remains attached with four closed
partitions and its final raw SHA256 recorded. Images, credentials, failures,
source/config inputs and the bootable original are retained. No unrelated
nine-filesystem test was rerun, and no product binary was rebuilt.

Folder cases used explicit `--mount-root` plaintext FUSE mountpoints on the ext4
work volume, with ciphertext backing on UFS1, as recorded in their arguments.
Host FUSE policy remained unchanged. The desktop sample above used the native
physical LUKS1/UFS1 row, separately from those folder-backend cases.
