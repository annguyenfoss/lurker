# OpenBSD unachieved filesystem retest — 2026-11-25

**No new filesystem capability was achieved.** Fresh CLI attempts for ext4,
XFS and APFS remain blocked before authentication by the existing VeraCrypt
read-only refusal. HAMMER2 still has no qualifying unmodified writable driver
on OpenBSD. The previously passed native readers and 12 writable CLI pairs
were not repeated or relabeled as new successes.

## Candidate and host identity

Root SSH inventory identified `openbsd-core-01`, OpenBSD 8.0
`GENERIC.MP#12`, built 2026-11-17 by the official syspatch service. This is the
post-package-refresh/reboot guest; the previous filesystem round recorded
`GENERIC.MP#84`. Only the baseline root filesystem was mounted at entry.

Reused the administrator-owned development CLI from source `d469704`
(full commit recorded in `source-reuse.json`), SHA-256
`d4a11b14aa63af68547b7d3e7f37723114a81b5ae65fbb17b9cbf1348caff978`, and
its private VeraCrypt backend, SHA-256
`0c3db467e0abc33f67997a22f6deba438cfcab228a4095c063715b0a564be99e`.
The shared BSD lifecycle, OpenBSD adapter/filesystem dispatch, RAID1C adapter,
VeraCrypt adapter, API, model and CLI source are byte-identical to inspected
HEAD `8e420b4ced209cb6c97533cb4131bde47c146137`. The only intervening
`workflow.rs` change affects macOS FAT32 creation. No product source was
changed, so no new compilation or unit-test result is claimed.

## Exact CLI attempts

The package refresh preserved the original encrypted import fixtures under
`/root/lurker-package-refresh-preserved-20261125/lurker-filesystem-round-openbsd-20261125`.
Each prior receipt records a successful native PDF/text reader control, not a
CLI mount. This retest invoked the real CLI once per fixture:

```text
/root/lurker-filesystem-round-openbsd-20261125/bin/lurker mount --type veracrypt --readonly --color=never PRESERVED_FIXTURE/vault.img /tmp/lurker-filesystem-retest-openbsd-20261125/FILESYSTEM
```

| Filesystem | Preserved fixture | Fresh result |
| --- | --- | --- |
| ext4 | `import-ext4-ddqslmlz` | Exit 1, explicit OpenBSD VeraCrypt read-only refusal |
| XFS | `import-xfs-r0ft67d4` | Exit 1, same refusal |
| APFS | `import-apfs-8q7mk_w6` | Exit 1, same refusal |

All three terminated without timeout, acquired no mapping/mount, and left the
ciphertext SHA-256 unchanged. Their original ephemeral passphrases were not
persisted. A fresh random probe credential was supplied only to reach the
pre-authentication refusal; these are **boundary retests**, not authenticated
unlock or mounted-payload tests. PDF/text verification and normal close were
not reached. Expected prior payload digests, exact argv, output, and before/after
ciphertext digests are retained in `summary.json` and the three CLI logs.

The CRYPTO and RAID1C read-only routes were inspected rather than replayed on
unrelated passed fixtures: both still stop at OpenBSD's existing
`require_writable` guard. The installed `vnconfig` and `bioctl` expose no
read-only attachment option, and installed `vndioctl.h` supplies no explicit
read-only setting. OpenBSD's vnode driver can infer read-only from an `EROFS`
backing-file open failure; this does not provide read-only softraid assembly
or repair VeraCrypt's own unsupported branch. No writable crypto fallback was
substituted. In pinned upstream VeraCrypt
`7afc449f2bc69366e8b67b82ad66201325d318c3`,
`CoreOpenBSD::AttachFileToLoopDevice(..., readOnly)` throws for `readOnly`.
Removing Lurker's refusal alone would expose that unimplemented backend path.
XFS/APFS also retain the earlier documented libyal block-device-size issue;
no new native reader control was run to disguise either blocker.

## HAMMER2 alternative investigation

Fresh official package queries returned no HAMMER2 package; the running
kernel's `vfs` inventory has no HAMMER2 filesystem. Reviewed unmodified
upstream sources at explicit commits:

- `kusumi/openbsd_hammer2` at `a3747df966605b0db5b3512302dfc5c610f2dea6`:
  the shipped `patch/openbsd77.patch` changes kernel filesystem registration,
  GENERIC configuration, native mount/vnode types, kernel allocator types and
  ioctl dispatch. Its preparation script installs source links into
  `/usr/src/sys`. Its userland formatter/mount programs alone cannot supply
  the missing kernel filesystem. The patch was retained as evidence, never
  applied.
- `kusumi/hammer2-fuse` at `b92433c72dd0925e2ca21e8d06b3183ecb0559ca`:
  the userspace alternative documents Linux/FreeBSD support and unconditionally
  adds `fuser::MountOption::RO` after mounting its filesystem library. It is
  not a writable alternative even if its platform dependencies were portable.
  `kusumi/libhammer2` was resolved at
  `e0b88b9f40709637625d9256735268129bf6c96f` for reference.
- Portable `makefs` supplies image creation, not a writable mounted filesystem.
  Combining it with the read-only userspace reader cannot satisfy this task.

No source build was launched for these explicitly nonqualifying paths. No
kernel/upstream patch, new encryption backend, unencrypted success substitute,
package installation, reboot, GUI, physical-device, SQLite or fault test was
performed.

Primary references:

- https://man.openbsd.org/vnconfig
- https://man.openbsd.org/bioctl
- https://github.com/veracrypt/VeraCrypt/blob/7afc449f2bc69366e8b67b82ad66201325d318c3/src/Core/Unix/OpenBSD/CoreOpenBSD.cpp
- https://github.com/openbsd/src/blob/052ae0db6b5d8dc0bc01bf85d6fa0779b0be4f44/sys/dev/vnd.c
- https://github.com/kusumi/openbsd_hammer2/blob/a3747df966605b0db5b3512302dfc5c610f2dea6/patch/openbsd77.patch
- https://github.com/kusumi/hammer2-fuse/blob/b92433c72dd0925e2ca21e8d06b3183ecb0559ca/src/main.rs
- https://github.com/kusumi/makefs

## Evidence and cleanup

Local evidence: `target/goal-validation/filesystem-retest/openbsd/`, including
`summary.json`, `source-reuse.json`, `inventory.txt`,
`final-native-inventory.txt`, `probe-cli-readonly.py`, source excerpts/pins,
and `evidence-sha256.json`. Remote receipt/logs remain under
`/tmp/lurker-filesystem-retest-openbsd-20261125`; the root-owned probe script
remains beside the earlier administrator-owned harness. Existing fixtures and
packages are preserved.

Final inventory: baseline root mount only, all four vnodes unused, no softraid
array, no VeraCrypt volumes, no session JSON, no owned live mount/build worker,
and approximately 12 GiB free. No forced/lazy close was used. The guest is
released. Remaining work requires an eligible unmodified read-only crypto
route or writable OpenBSD HAMMER2 driver; none was found in the inspected
sources/packages.
