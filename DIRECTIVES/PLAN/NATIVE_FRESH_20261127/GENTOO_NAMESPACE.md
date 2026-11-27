# Gentoo mounted namespace supplement

This targeted follow-up uses installed `e927e40bb235815343302677531d48f1afa4f939` products and committed namespace harness `0071bd80c7e9d45e37a163630836b78f6a0d8cf9`, SHA-256 `9d4d6cc09d8b87e5503c7a6129554470017821f0f76b6fd8a5fd289ebaef11a6`. It supplements the original payload evidence with mounted plaintext namespace operations; the 576/678/e927 payload cohorts keep their original identities. No existing complete eleven-action product/driver proof was available to subtract. Four NTFS scopes remain held for the separately coordinated NTFS3 permissions fix.

Current kernel/boot are `7.3.6-gentoo-dist-bin` / `fd026475-bfa6-4fec-91f5-c4ef187a4e0b`. UFS1 requires the separately qualified matching module with existing `CONFIG_UFS_FS_WRITE` enabled; vendor configuration remains read-only. No kernel change, formatting, build, reboot, full-payload rerun or desktop selector change occurred here.

## Results

All **36 released scopes passed**, **396 actions**, with actual UID/EUID/GID/EGID 1300, exact native mount ID/source/type and both read/write option fields checked around each action. Every worker had a native normal-zero wait status; every disposable test directory was removed. These are explicit mounted namespace results, not inferred from earlier backend conformance.

| Native backing | Retained disk method | Native scope | gocryptfs | CryFS | Cryptomator |
| --- | --- | --- | --- | --- | --- |
| ext4 | LUKS2 | passed | passed | passed | passed |
| Btrfs | VeraCrypt | passed | passed | passed | passed |
| XFS | LUKS2 | passed | passed | passed | passed |
| APFS | LUKS2 | passed | passed | passed | passed |
| UFS1 | LUKS1 | passed | passed | passed | passed |
| UFS2 | LUKS2 | passed | passed | passed | passed |
| HAMMER2 | LUKS2 | passed | passed | passed | passed |
| exFAT | LUKS2 | passed | passed | passed | passed |
| ZFS | LUKS2 | passed | passed | passed | passed |
| NTFS3 | held | held | held | held | held |

Each native scope reopened an identified retained, closed reboot container using its original binary keyfile. The native mounted container then backed three fresh tiny directory vaults. Their plaintext mountpoints and every receipt/factor stayed on the ordinary ext4 work filesystem outside the tested backing. Folder plaintext drivers were `fuse.gocryptfs`, `fuse.cryfs` and `fuse.fuse-nio-adapter`; UFS2 used the qualified external `fuse.ufs` driver. Independent `blkid` version-1/version-2 observations and unchanged driver hashes bind the UFS rows. Native root identity and original protected `Lurker payload` file bytes, ownership and modes matched before/after all child sessions. Original container/keyfile ownership, modes, size and keyfile hashes were unchanged. New vaults/sentinels remain in the retained encrypted files; this is not a claim that ciphertext bytes stayed unchanged.

The eleven actions create/sync a unique directory, create/write/fsync/read/hash two files, rename a file and directory, move across parents, unlink both files and remove the two owned directories. Parent directories were synchronized after mutations. All mounts closed normally; no forced cleanup or chmod of existing data occurred. No native namespace failure or unknown worker state occurred.

## Per-run notes and evidence

Remote job root is `/srv/lurker-native-fresh-20261127-gentoo/namespace-0071bd8/`; local evidence is `target/goal-validation/native-fresh-20261127/hosts/gentoo/build/mounted-namespace/`. The private completion index maps exact local receipts/hashes.

| Purpose / inputs | Executed command or receipt/log | Outcome / cleanup |
| --- | --- | --- |
| Idle/resource/input discovery | `preflight.py`, `preflight.json` | Same qualified kernel/boot, no prior task mounts or engines; original closed fixtures/factors available. Root 2,316,587,008 bytes and work 3,874,000,896 bytes free. Initial read-only inventory output was also inspected; full formal process/mount/device records are retained in preflight. |
| Namespace execution | `run-gentoo-namespace.py`, `launch.sha256`, `controller.log`, `result.json`, per-filesystem/per-engine `namespace.json` and action logs | Controller SHA-256 `d7313ee750ac3c7f5f4371da2caa0fd4c4f720f9ae52de23d480e3842a65726b`, PID 2800; all 36 passed. Unknown helper/worker state would stop subsequent actions without cleanup; none occurred. |
| Installed identity audit | `verify-installed-identities.py`, `identity-launch.sha256`, `identity-controller.log`, `installed-identities.json` | All 1,085 installed manifest entries and root-controlled ownership/modes verified. APFS/HAMMER2/ZFS module hashes match retained `validation/final-environment.json`; UFS1, corrected UFS2 and host FUSE match their qualified hashes. |
| Final receipt/provider/cleanup audit | `final-audit.py`, `final-audit-launch.sha256`, `final-audit-controller.log`, `final-audit.json` | Passed 36 receipts/396 actions, all protected file/held-root/factor metadata comparisons, actual loaded-module build IDs against installed ELF notes, no owned mounts/loops/engine processes/imported pools. Continuous root/work 2 GiB floor remained satisfied. |
| Retain closed fixtures and evidence | `stream-final.py`, `stream-launch.sha256`, `archive-inventory.json` and local archive verification | Streamed receipts/controllers, synthetic factors and all nine closed native ciphertexts directly to the coordinator to avoid guest archive allocation. Sources are hashed before/after; runtime cache excluded and retained on guest. Coordinator private directory/archive modes are 0700/0600; archive mode was normalized after streaming began. All 368 members matched their size/hash/mode records and the entire gzip CRC stream passed locally. Archive SHA-256 is `71cf11d9375ecc33ab773f7e998e5d3768e3e76a8beb9554f1c97b2f75f62e6b`; all selected source hashes matched before/after streaming. |

Historical retention gaps remain disclosed in `GENTOO_RESULTS.md`; this supplement does not recreate lost originals or relabel historical passes. Linux GUI create still has its fixed default; no general new filesystem-selector feature is claimed. The remaining work in this supplement is precisely the four held NTFS3 scopes, pending the named prerequisite/product fix.

The earlier UFS1 report identity is preserved as local `build/mounted-namespace/GENTOO_RESULTS-at-d0cee9c.md` and in commit `d0cee9c`; the prior completion indexes are unchanged.
