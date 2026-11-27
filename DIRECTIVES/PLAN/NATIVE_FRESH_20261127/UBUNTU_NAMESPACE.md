# Ubuntu mounted namespace supplement

This supplement uses unchanged installed `e927e40` products, existing native
drivers and `0071bd8`'s eleven-action namespace probe. It does not relabel the
original 576/678/e927 payload matrices or repeat their reboots/builds.
Probe SHA256:
`9d4d6cc09d8b87e5503c7a6129554470017821f0f76b6fd8a5fd289ebaef11a6`.

## Namespace outcomes

| Filesystem | Native encrypted filesystem | gocryptfs | CryFS | Cryptomator |
| --- | --- | --- | --- | --- |
| UFS1, configured matching module | VeraCrypt: pass | pass | pass | pass |
| ext4 | LUKS1: pass | pass | pass | pass |
| Btrfs | LUKS2: pass | pass | pass | pass |
| XFS | VeraCrypt: pass | pass | pass | pass |
| APFS | LUKS1: pass | pass | pass | pass |
| UFS2 | LUKS2: pass | pass | pass | **unlink EIO; retained** |
| HAMMER2 | VeraCrypt: pass | pass | pass | pass |
| exFAT | LUKS1: pass | pass | pass | pass |
| ZFS | LUKS2: pass | pass | pass | pass |
| NTFS | coordinator hold | coordinator hold | coordinator hold | coordinator hold |

All 36 authorized non-NTFS scopes are terminal: 35 complete passes (385 actions),
one failed scope after seven passed actions, and four NTFS scopes still held.
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

The preserved pre-close outer native mount was ID119, source
`/dev/mapper/lurker_vault.img_b0a3d89e2c75`, type `fuse.ufs`. The inner mount
was ID221, type `fuse.fuse-nio-adapter`. Owned supervisor10791
(start ticks1009902) and JVM10792 (1009903) were UID1300. The test controller
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

After explicit coordinator approval and verification of both PID birth times
and mount IDs, one normal installed helper close returned `ok:true`. Inner221,
JVM10792 and supervisor10791 disappeared. Normal native outer119 unmount then
succeeded, followed by source-addressed installed CLI closure of the exact LUKS
vault. The final inventories show no mount, mapper, loop or session-directory
remnant. Receipts: `normal-close-ufs2.json` and
`normal-close-ufs2-outer.json`. No backend signal, driver restart, ack rewrite,
forced unmount or repair was used. This cleanup does not turn the failed
namespace action into a pass or prove its protected payload after failure.

Only after that terminal cleanup, PID13279/controller SHA`c487c62f…` completed
all twelve independent HAMMER2/exFAT/ZFS scopes in a new evidence directory.
There was no UFS2 retry. NTFS remains on its separate coordinator hold. The
common CryFS source-only assessment remains frozen and unchanged.

## Terminal verification

`finalize.py` verified all 36 exact per-action receipt hashes and their native
worker exit results without rerunning a test. Installed CLI/helper/desktop and
all recorded driver hashes matched preflight; `dpkg -V lurker` passed. No owned
backend process, mount, mapper or loop remained. The eight new closed ciphertext
images and their factors remain retained; their hashes are indexed. Resource
admission stayed above2GiB free on every checked volume; minimum available
RAM2,436,653,056B. Final free space: root5,239,554,048B,
evidence3,472,699,392B, work10,230,312,960B.

The final metadata archive has382 members, SHA256
`e3f0d4eeacfbaeb2f27bf567f9ad1dfb1216a3c0dd7e01165603a6369a09228a`;
`final-audit.json` SHA256 is
`31035c1cab1de9206f42f2c33920af40c77bde3ee251ac2408ee888846ae3650`.
The private `completion-evidence-index.json` maps every scope to its exact local
receipt and SHA256, with the failed-original off-host custody and cleanup
references. UFS1/ext4/Btrfs/XFS/APFS/HAMMER2/exFAT/ZFS have no remaining owned
namespace work. UFS2 remains quarantined after the newly exposed failure;
NTFS's four scopes remain held. No test is currently running.
