# Fedora mounted namespace supplement — 0071bd8

**32 of 40 logical scopes pass as UID/EUID 1300.** One UFS2-backed gocryptfs
scope failed; three other UFS2 scopes are blocked by that prerequisite failure.
Four NTFS scopes remain on hold for the coordinator's fresh-root permissions fix.
No namespace job remains active. Earlier 576/678 payload, device and reboot
receipts, and the separate e927 affected-runtime qualification, retain their
executed source and artifact identities.

The supplement uses committed `scripts/test-mounted-namespace.py` at `0071bd8`,
SHA-256 `9d4d6cc09d8b87e5503c7a6129554470017821f0f76b6fd8a5fd289ebaef11a6`,
with the retained qualified `67894f6` private prefix. CLI SHA-256 is
`4be8699d321f4e4babbe40d290d5395700aaffd19fd09733de8dfa17e16c95a5`.
Existing closed fixtures supply the original factors and ciphertext. No package,
signing, kernel, build, reboot or filesystem-selector change occurred.

Each passing scope verifies eleven create/write/fsync/read/hash, rename,
cross-parent move, unlink and rmdir actions against the actual caller and held
native mount ID/source/type, including both read/write option fields. Both probe
parent and worker exit normally with zero. Protected existing bytes and
ownership/modes remain equal; product close succeeds and leaves no mount.
The disk column is one native filesystem mount distributed over the three disk
methods, not a repeat of all disk-method combinations.

| Filesystem | Native disk method / namespace | Directory engines on native backing | Remaining / support qualification |
| --- | --- | --- | --- |
| ext4 | LUKS2: pass | 3/3 pass | None in this supplement |
| Btrfs | LUKS1: pass | 3/3 pass | None in this supplement |
| XFS | VeraCrypt: pass | 3/3 pass | None in this supplement |
| APFS | LUKS2: pass | 3/3 pass | Upstream Linux write support remains experimental |
| UFS1 | LUKS1: pass | 3/3 pass | Existing configured writer; stock Fedora writes disabled |
| UFS2 | VeraCrypt: blocked | gocryptfs failed; CryFS/Cryptomator blocked | Host FUSE driver unlink/disconnection failure; no inferred failure of unattempted rows |
| HAMMER2 | LUKS2: pass | 3/3 pass | None in this supplement |
| exFAT | LUKS1: pass | 3/3 pass | None in this supplement |
| ZFS | VeraCrypt: pass | 3/3 pass | None in this supplement |
| NTFS | LUKS2: hold | 3 held | Coordinator's fresh-root permissions prerequisite; no namespace attempt |

Nine initial disk probes stopped before action zero: historical fixture
**ancestors** are root-owned mode 0700, although the mounted filesystem roots
are owned by UID 1300, mode 0700. These were caller-admission proof gaps. Their
protected payloads were unchanged and normal close passed. Eight were then
reopened through normal Lurker operations at new caller-owned plaintext
mountpoints and passed. No bind alias, chmod or chown was used. The UFS2 row
remains blocked. Historical root-private payload protection uses separate bounded
read-only root manifests before/after; the new namespace mutations still run as
UID 1300 and make no claim that the caller read those private historical trees.

The first run passed all three directory engines on ext4, Btrfs, XFS, APFS and
UFS1. On UFS2/gocryptfs, seven actions passed, including file/directory rename
and a cross-parent move. `unlink-child.bin` then returned EIO 5. The probe worker
exited normally with code 1. The `fuse-ufs` process was subsequently absent and
the backing endpoint disconnected; its actual exit cause is unproven, with no
coredump or matching journal entry. Normal product close refused with ENOTCONN
107. Both mounts, backend/supervisor and failed namespace were retained for
review. The protected after-hashes are **unproven**; the before-manifest remains.
The unchanged host driver SHA-256 is
`237e0467fa4da053c721dd171b71243aee4c7e056a9b66054d6e4ed938c70f30`.

After explicit coordinator approval, original source-before/copy/source-after
hashes matched, and a private raw snapshot plus session journals and generated
factor candidates were retained. Source SHA-256, also unchanged after recovery:
`b260bb141c8e7438e868d6c82c56bff5311dcb8ebb4f4f41e73c18c423f7bfde`.
Normal user `fusermount3 -u` detached only plaintext mount 246. The recorded
backend wait result was code 0, no signal, `closed_ack: false`; both backend and
supervisor reached known terminal states. Only then did normal
`umount -i -c` detach the exact disconnected UFS2 backing mount 245. No force,
signal, driver restart, fsck, permission change or journal/path removal occurred.
This is failed-fixture host recovery, not a namespace or normal product-close
pass. All four UFS2 scopes remain quarantined without substituting another driver.

The independent resume passed eight native rows plus the remaining nine
HAMMER2/exFAT/ZFS directory rows. The 40-slot reconciliation contains 32 passed,
one failed, three blocked and four held. Raw history has 42 probe invocations:
nine zero-action admissions, eight approved admission retries and 33 admitted
runs. Successful scopes account for 352 actions, with seven additional actions
before the UFS2 failure. Completed scopes were not repeated.

Private evidence root:
`target/goal-validation/native-fresh-20261127/hosts/fedora/build/mounted-namespace/`.
`RUN_NOTES.md` records every preparation correction, run, failure and recovery;
raw commands/logs remain separate. The completion index has 312 hashed references:

- `completion-evidence-index.json`, SHA-256
  `10155b87cdb5c0215cccedc41e0f40e418e8bd79cdb66a13951affee2e8e6ce2`.
- Initial public receipts: `bcbde7c28def33ef45bf48d0c4926a9d09f48456ae8f98fdcdd0a21d8b29f8d6`.
- Resume public receipts: `180054d6f876152717efaf1719f25caaf361b470ba1c77c0e935d590752b4e6d`.

The public archives exclude private custody and contain no generated test-factor
values. The native private raw image/factor files remain mode 0600 and session
copy mode 0700. Terminal observation finds no owned vault mounts, active
processes, mappers, loops or pools. Both prior private prefixes and driver hashes
are unchanged; global RPM/trust restoration remains intact and SELinux Enforcing.
Failed session journals remain retained. Final free bytes were 19,781,111,808 on
root and 10,227,519,488 on the fixture filesystem.
