# Ubuntu mounted namespace supplement

This supplement uses unchanged installed `e927e40` products, existing native
drivers and `0071bd8`'s eleven-action namespace probe. It does not relabel the
original 576/678/e927 payload matrices or repeat their reboots/builds.
Probe SHA256:
`9d4d6cc09d8b87e5503c7a6129554470017821f0f76b6fd8a5fd289ebaef11a6`.

## Initial outcomes

| Filesystem | Native encrypted filesystem | gocryptfs | CryFS | Cryptomator |
| --- | --- | --- | --- | --- |
| UFS1, configured matching module | VeraCrypt: pass | pass | pass | pass |
| ext4 | LUKS1: pass | pass | pass | pass |
| Btrfs | LUKS2: pass | pass | pass | pass |
| XFS | VeraCrypt: pass | pass | pass | pass |
| APFS | LUKS1: pass | pass | pass | pass |
| UFS2 | LUKS2: pass | pass | pass | **unlink EIO; retained** |
| HAMMER2 | pending | pending | pending | pending |
| exFAT | pending | pending | pending | pending |
| ZFS | pending | pending | pending | pending |
| NTFS | coordinator hold | coordinator hold | coordinator hold | coordinator hold |

There are 23 complete passes (253 actions), one failed scope after seven passed
actions, twelve independent non-NTFS scopes pending and four NTFS scopes held.
All UFS2 scopes are now quarantined: the three completed profiles retain their
passes; the failed Cryptomator profile is a newly exposed defect/status downgrade.
There is no proof that an old passing run exercised this identical failing
action and subsequently regressed.

Every complete pass records actual UID/EUID1300, the held native mount ID,
source/type and read/write options before/after each action, protected file
hashes/ownership/modes, normal zero worker exit and removal of only the generated
namespace. UFS1 retains its matching module note and VERSION=1 proof; UFS2
records VERSION=2 and driver SHA256
`78c7e182b87d1e7ad7d3ba21ad49d79444e55db70c9a740788efff57a936b7cb`.

## Run notes

| Purpose / inputs / command reference | Outcome / cleanup |
| --- | --- |
| `preflight.py`; kernel7.3.0-11-generic, installed CLI/helper/desktop and configured UFS1 module hashes; driver/resource/quiescence inventory | Passed those checks, zero active backends/mounts. Root5.24GB, evidence3.47GB, work14.75GB free; RAM3.59GB available. It did not establish that every old image still existed. |
| `run-namespace.py`, PID4132, SHA3cb14379…; attempted reuse of old678 reboot fixture manifest | First helper reopen cleanly refused the missing ext4/LUKS1 image before any namespace probe or mount. The earlier successful ciphertexts had been reclaimed; their receipts remain historical. No image was reconstructed. |
| `run-ufs1-namespace.py`, PID5230, SHA7e6e3678…; retained e927 configured-UFS1 fixtures | Four passes/44 actions, old five-file hashes unchanged, normal closes. No product rebuild or reboot. |
| `run-fresh-namespace.py`, PID6111, SHA9579c687…; coordinator-approved new minimal fixtures for eight other non-NTFS filesystems | Nineteen passes and one Cryptomator/UFS2 failure, then terminal retained stop. One fixed-profile encrypted native container per filesystem also backs its three folder vaults. Sizes0.25GiB, or0.5GiB for XFS/HAMMER2. Five new fsynced payload seeds are explicitly new inputs, not old678 evidence. |
| `preserve-ufs2-failure.py`, `freeze-ufs2-failure.py`, `freeze-cipher-sequential-v2.py` | Original failure, raw backing, encrypted vault, factors, session, native IDs and logs preserved before any close. Both initial preservation and the stronger sequential full-hash snapshot remain separate. No repair, retry, daemon restart, force-unmount or signal to a backend. |

Controllers/launch hashes, all attempts, helper JSON, per-action receipts and
logs are under
`target/goal-validation/native-fresh-20261127/hosts/ubuntu/build/mounted-namespace-0071bd8/`.
The collected pre-close metadata archive has 242 members, SHA256
`43440fc5a88a67a48204f998cfe4c76a3b58ab0364b9bd2cf5e1422894fa08d9`.

## Cryptomator/UFS2 failure and retained state

The actual worker passed create/write/fsync, file rename, directory rename and
cross-parent rename, then `unlink-child.bin` returned EIO5. Worker10824 exited1
(native wait status256), supervisor10823 exited1; this was not a timeout.
Native `fuse-ufs` was absent afterward; its earlier PID/birth, exit status and
backtrace were not captured and are unproven. Old protected payload hashes were
recorded before the failure; their post-failure integrity is **unproven**.

The outer native mount remains ID119, source
`/dev/mapper/lurker_vault.img_b0a3d89e2c75`, type `fuse.ufs`. The inner mount
remains ID221, type `fuse.fuse-nio-adapter`. Owned supervisor10791
(start ticks1009902) and JVM10792 (1009903) remain UID1300. The test controller
is terminal; no further UFS2 test is running or planned.

Off-host archive `ufs2-failure-before-close-v2.tar` is520,437,760 bytes, SHA256
`57bb7179fa819a861700b91f15e70d4234b57f7198d9925ccb0ac0c5e35c72ef`.
Independent off-host member hashes match the full source-before/copy/after
hashes:

- Encrypted vault268,435,456B:
  `d45dc18df873d53f0e33e20d428fa4f11ca88d20693e83f389fa1c625921836c`.
- Decrypted UFS2 backing251,658,240B:
  `1ea3adc99dc08390a9872d1ff91c52dcc5298910c875f8351aaad7d78fd5e9bc`.

This is stable-byte preservation after the backing daemon disappeared, not an
atomic cold filesystem snapshot or a successful recovery. Exact process and
archive verification are in `offhost-freeze-verification.json`.

Coordinator review precedes the proposed one normal installed product close of
the exact Cryptomator session. Any refusal remains retained. Only a clean inner
close permits normal outer unmount and source-addressed LUKS closure. No manual
recovery, ack bypass, forced cleanup or filesystem repair is authorized here.
The remaining HAMMER2/exFAT/ZFS scopes wait for terminal cleanup/host safety;
NTFS remains on its separate coordinator hold. The common CryFS source-only
assessment remains frozen and unchanged.
