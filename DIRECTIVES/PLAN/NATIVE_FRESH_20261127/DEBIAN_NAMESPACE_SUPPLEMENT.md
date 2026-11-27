# Debian mounted namespace supplement — 2026-11-27

**28 scopes pass; one UFS2 scope fails; eleven scopes remain unexecuted.**
The previously completed payload matrices retain their original 576/678 source
identities and narrower workloads. This supplement adds plaintext rename,
cross-parent move, unlink and directory removal; its UFS2 failure prevents a
claim of complete namespace support. An earlier identical unlink pass is not
established, so this is a newly exposed functional defect, not a proven regression.

Inputs: committed harness `0071bd8`, SHA-256
`9d4d6cc09d8b87e5503c7a6129554470017821f0f76b6fd8a5fd289ebaef11a6`,
following `LINUX_NAMESPACE_FOLLOWUP.md`. Runtime is unchanged installed
`e927e40bb235815343302677531d48f1afa4f939`, package
`0.1.0+fresh20261127.e927e40`; CLI SHA-256
`26c653bb9381bc25e079bc0c6b87dc24caf8bba6cbc90b71f5dd9025b3d96dfa`.
Actual UID/EUID 1300 is sampled in every namespace receipt and action. Each
passing worker exits normally with zero, holds/checks its native mount-root FD,
checks mount ID/source/type and both writable option fields, performs eleven
actions, preserves protected file hashes/ownership/modes, removes its unique
namespace and is followed by successful normal product close.

| Filesystem | Inner method | Namespace scopes passed | Remaining |
| --- | --- | ---: | --- |
| ext4 | LUKS1 | 4/4 | None |
| Btrfs | LUKS2 | 4/4 | None |
| XFS | VeraCrypt | 4/4 | None |
| APFS | LUKS1 | 4/4 | Existing external native driver prerequisite |
| UFS1 | LUKS1 | 4/4 | Existing write-enabled module prerequisite |
| UFS2 | LUKS2 | 0/4 | Native unlink fails; three folder rows quarantined |
| HAMMER2 | Not executed | 0/4 | Qualified 7.3 cohort; current 6.12 lacks its module |
| NTFS | Not executed | 0/4 | Coordinator's permission hold; no Debian supplement run |
| exFAT | VeraCrypt | 4/4 | None |
| ZFS | LUKS2 | 4/4 | Distribution OpenZFS on 6.12 prerequisite |

Each four-scope group is one native filesystem mount plus gocryptfs, CryFS and
Cryptomator on that filesystem. Sources are file containers and directory
engines on native file-backed mounts. No new physical-device coverage is claimed.
UFS1 remains supplemental `CONFIG_UFS_FS_WRITE` evidence; HAMMER2's earlier
successful 7.3 cohort is not invalidated by this task's no-reboot boundary.
NTFS is held for the coordinator's separate finding, not a measured new Debian
permission failure. No full payload matrix, build, installation, reboot, module
configuration, filesystem formatting or LKL operation was performed.

## Runs, failures and cleanup

Private evidence root, abbreviated below as `E`:
`target/goal-validation/native-fresh-20261127/hosts/debian/build/mounted-namespace-0071bd8/`.
Frozen controllers/manifests precede each dispatch; complete executed sources,
commands, per-action receipts and raw logs are in the metadata archive.

| Run / controller PID | Purpose and exact controller | Outcome / retained evidence |
| --- | --- | --- |
| Attempt 1 / 46014 | `run-namespace.py`, SHA `bd67735672059256c82e809b4fcf9b303997198e6256439e9c81cba3cad1b1d3` | TTY launcher needed an absolute sudo path; early receipt lacked `mountpoint`. Eight launch failures, then controller stop; no successful mounts or namespace actions. |
| Attempt 2 / 46486 | `run-namespace-2.py`; `inputs-2.json` | 31 native mount-observer failures: retained symlink spelling differed from canonical mount path. All opened mounts closed normally; one missing passed fixture. No namespace actions. Launch SSH observation timed out while the job continued; it was observed to terminal, not restarted on timeout. |
| Attempt 3 / 63471 | `run-namespace-3.py`, SHA `df5471d4eebe553d3b11dad7b40ac3910fdcb66f73e0ca78de885e34e5801397` | Five scopes pass; UFS2 fails after seven actions. Cohort stops with disconnected mount retained for review. |
| Attempt 4 / 67108 | `run-namespace-4.py`, SHA `5a6a10fbfdb6d87ea3e842cf1ada6db771da11d36aa2059ccf071c293fa0615e` | Subtracts five passes, quarantines UFS2: 20 passes; two early ext4 backing fixtures actually ext2 and rejected before namespace actions; no passed retained Cryptomator/ext4 fixture. All resources close normally. |
| Fresh ext4 / 80813 | `fresh-ext4.py`, SHA `bf63aac551846e3d00edaabc2cd0dbdd27e01098e7f65de65eadd457c944740e` | Three folder scopes pass on the already qualified retained ext4/LUKS1 vault. New encrypted directories receive a tiny payload, normal close/reopen, then namespace checks. Original outer five-file payload also remains byte/metadata-identical. No backing formatting. |

The earlier successful raw directory-backing harness removed those backing images;
their retained receipts are not themselves reusable ciphertext. Fresh ext4 fills
only the three missing valid fixture scopes. Read-only inventory readers also
had unavailable-command/PATH and quoting errors, corrected before dependent
actions; these are noted in `E/completion-index.json`.

UFS2's exact native mount was `fuse.ufs`, mount ID 218, source
`/dev/mapper/lurker_vault.img_2dfe59f7d5e8`; independent `blkid -p` recorded
UFS version 2. Host driver `/usr/local/bin/fuse-ufs` SHA-256 is
`afcb489fdd1e441d418883c71d5abfe45d89d1f9548a1b2fe94479375641c30d`
(pinned `ee5c4a44` plus the qualified no-replace, barrier and pagination patches).
The eighth action, `unlink-child.bin`, returned `EIO`; the daemon was absent
afterward. Its exit status/backtrace was not captured. Protected old plaintext
hashes after this failure remain unproven.

Before cleanup, a new 512 MiB ciphertext clone was synced and compared against
the source before/after copying. All hashes are
`3fdd8bb48892d51cba4b39c1aac8653bebc5062fca1d3196e3fc9ce1f1cad474`.
The clone is retained mode 0400 at
`/var/tmp/lurker-native-fresh-20261127-debian-storage/mounted-namespace-0071bd8-3/inner-ufs2-luks/failed-ciphertext-snapshot.img`.
After verifying owned workers absent, exact mount/source identity and mapper
open count zero, native normal `umount -i -c` succeeded. Mountpoint-addressed
product close refusals are preserved; the normal source-addressed
`lurker unmount <canonical vault.img>` then succeeded and released mapper/loop.
No force/lazy unmount, direct crypto recovery, reboot or fsck was used. This
resource cleanup does not turn the failed filesystem session into a pass.

## Receipts and terminal state

- `E/completion-index.json`, SHA-256
  `88aefa1d28fbbc2d65c44bd6b0769b5d8ae5b4ec827241043372869c5407deb3`:
  29 actual namespace receipts, 28 passes and one failure, per-filesystem coverage,
  exact controllers and all three unchanged installed executable hashes.
- `E/ufs2-failure-index.json`, SHA-256
  `bb2466273ee997d1a5327f2abebdbb032161f4f2c47aa2fd16af828a3bad198e`:
  raw fault, protected-before hashes, driver identity, clone and cleanup references.
- `E/mounted-namespace-0071bd8-metadata.tar.gz`, SHA-256
  `aa79fce21c342c674e225d2a30b8d4fa413dae474ccc62570c7a95fe3c468238`:
  complete metadata/source/log packet, excluding the separately retained image.
- `E/terminal-evidence/mounted-namespace-terminal-inventory.json`, SHA-256
  `4b8487614459e79788f7627c3c8244ead099b941c4bf6796430c7e19cc390a3b`:
  all controllers terminal; no owned mounts, backend workers, mappers or loops;
  empty `dpkg -V lurker`; installed CLI/helper/desktop match the prior installation.
  Free space remains 2.90 GB on root and 21.30 GB on evidence storage, above the
  continuous 2 GiB floor.

All original fixtures, newly created encrypted directories, failure remnants,
credentials and ciphertext remain retained. Further UFS2 namespace qualification
depends on a separately reviewed host-userspace driver diagnosis/fix; NTFS awaits
its coordinator-owned permission work, and HAMMER2 awaits an authorized matching
kernel-cohort window. No completed matrix is queued for repetition.
