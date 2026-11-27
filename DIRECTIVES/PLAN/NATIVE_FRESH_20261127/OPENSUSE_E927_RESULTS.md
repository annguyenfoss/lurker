# openSUSE e927 affected unprivileged qualification

Status: **passed**, using exact product source `e927e40bb235815343302677531d48f1afa4f939` and installed native RPM `lurker-0.1.0.20261127.3.e927e40-1.x86_64`. This is the bounded affected follow-up authorized by the reviewed Arch handoff. The original 576 broad matrices and 678 UFS2, frontend and ordinary reboot receipts keep their original identities in `OPENSUSE_RESULTS.md`; this follow-up does not repeat or relabel them.

Host: `adjutant@opensuse-core-01`, actual login UID 1300, Leap 16.1, stock packaged `7.3.6-lp160.4.gf857b30-default`, unchanged boot `4f2b5d8f-8d91-4705-8dfb-b7776e3ba4ab`. Work and retained fixtures are on the existing identified 64 GiB utility disk under `/var/lib/lurker-native-fresh-20261127-data/unprivileged-e927e40-20261127`. Original disks, failed fixtures and prior evidence remain retained. No privileged filesystem matrix or reboot was repeated.

## Exact source, native builds and installed closure

- Source archive SHA-256: `43783e9fccb1ece50aa240d2d10d50a5a7a8ecaafaa2905d86c722152d20582e`; source-manifest SHA-256: `fd51b745892dd8a29a638fe65aecf40cd51fe45b0f12d9a76a790beb76d2455f`. All 497 frozen regular source files matched again after the runtime gates. No product source overlay was used.
- Native `cargo check --workspace`, `cargo test --workspace` and all three product builds passed: **186 passed, zero failed, 15 ignored**. The 101 selected Python build/packaging/harness tests passed. The ignored native desktop callback was then exercised separately with its exact native test binary.
- Native Crypto4 and the separately installed host FS5 prerequisite were rebuilt against the existing native FUSE 3.16.2 / `libfuse3.so.3`. No foreign ABI4, SONAME alias, bundled FUSE runtime or bundled filesystem helper was introduced. The prescribed LKL filesystem build is a userspace host facility; the running host OS kernel is unchanged.
- The full six-backend native package stage verified source, patch, license, host-provider and private dependency closure. Native loader checks for Crypto4, the creator and FS5 resolve the actual host SONAME3 with no missing dependencies. The app package contains private Crypto4 and its creator; FS5 is administrator installed at `/usr/local/libexec/lurker-fs`, with separate root-controlled provenance.
- Native RPM conflict/dependency checking, upgrade, installed hashes and final `rpm -V lurker` all passed. Ordinary native flex/bison prerequisites and the X11 test tools were installed without repository, vendor or host-policy changes.

| Artifact | SHA-256 |
|---|---|
| Installed RPM | `2b34725d1718e4703254d77fa93dcaac25dfc816c95e731811d962376ca56274` |
| CLI | `37d8ce2814df48218719001f869fcde6b5eaebbac5aa4b80b555856f834df8ab` |
| Helper | `2e4407f4fcc7fd3e156dc53b63187a0206d0793c65f4a5dd0435a56566a4eb68` |
| Desktop | `722c24c13317ba4842f76b5f7ce1f46b13f47f5f2495c1ee5b592d7c6f32baf1` |
| Private Crypto4 | `3451f18f50786498da163fba94766f5b4a632761fc5f97be8cf60f9555a5a534` |
| External host FS5 | `0cbf993415562365fff495318b8b240a4034244267b01ffc24ff5d2c302564ce` |
| Private creator | `c22289d1928f97974ea88c2232f9f206e97d9431ff1f9410d464808aed21aee0` |

## Affected native gates

| Gate | Observed result |
|---|---|
| Six normal-user cipher/credential CLI lifecycles | **6/6**, 42 operations: AES/password/XFS, AES/binary-keyfile/ext4, and Serpent/Twofish with both credentials/Btrfs. Full downloaded PDF/text write, normal close, Auto reopen, append/new file, reopen/hash verification and normal close passed. Each phase binds actual UID, FS5 executable hash/PID, held raw inode, session/mount ID and independent inner superblock. |
| Strict helper credentials | **2/2**, password and binary keyfile/Btrfs, 14 complete-stdout JSON responses. Direct ordinary-user helper execution, three bounded payload opens, actual driver identity and normal closes passed. |
| Role readiness | **5/5** strict JSON variants. Only explicit login-user readiness probes the backend. Two actual root/as-root `execve` traces show no private crypto or host-FS probe execution. |
| Transient actual host-FLUSH error | **3/3**, XFS/Btrfs/ext4. A scoped native observer injected exactly one encrypted-backing `fsync` EIO. After disarming, two normal helper close attempts refused known failure, preserved the session and `io-error` acknowledgement, and did not attempt automatic journal recovery. |
| Explicit manual recovery controls | **3/3**, separate from product refusal. Before any operator recovery, complete original ciphertext/key/session/ack/driver log/sync log/both refusal responses were archived, every member hashed, full gzip CRC checked, and unchanged source identity/hash/lock verified. The reviewed test-only operator procedure then passed two independent native recovery opens and a normal product close. The original failed state remains in separate verified archives. No automatic product recovery was added or used for known errors. |
| Genuine absent-ack crash | **1/1**. The identified user FS5 process was actually SIGKILLed; no shutdown acknowledgement existed. Normal helper close took the documented crash-recovery route. Reopened PDF/text hashes matched, normal close passed and the source lock was available. No preload injection was used in this case. |
| Real committed winit callbacks | **1/1** native ignored test, unchanged test artifact, installed default helper, actual UID1300 and unprivileged selection/active-row markers. Its temporary private bind was normally unmounted afterward. |
| Missing backend close | **3/3**: installed CLI, strict helper and actual desktop Manage-row click. Both exact task-owned backend executables were temporarily absent while existing sessions closed normally. Guards bound the only FS5 session to the fixture; admin `finally` restored original component inodes, hashes, owners and modes. Actual X11 screenshots show the row as read/write/unprivileged before close and zero mounted volumes with the success notification afterward. |

## Evidence and retained harness errors

Private evidence root: `target/goal-validation/native-fresh-20261127/hosts/opensuse/build/unprivileged-e927/`. `completion-evidence-index-e927.json` maps each gate to local receipts and hashes. `native-receipts/` contains actual executed VM controllers, build/install logs, component and source manifests, driver provenance and runtime receipts. Each current controller was hashed before launch; launch records also identify controller revisions and native commands. Synthetic credentials and ciphertext archives are private evidence and are not tracked in Git.

- Six healthy cases are archived and completely verified in `healthy-installed-e927.tar.gz`, SHA-256 `0b8c7872eefd832877fc942f5fa728c18137a0f5f4cfac56ad86ec5cba77fbe1`. The aggregate receipt is not counted as a seventh case.
- Original failed XFS/Btrfs/ext4 archives and their independent verification proofs are in `original-negative-archives/`. Their original bytes were frozen before recovery; the later closed-control archive does not substitute for them.
- Recovered/closed controls, strict helper cases and runtime/build receipts are retained in `retained-closed-controls-and-native-receipts.tar.gz`, SHA-256 `f1886d4d50d4d7a315df21b4a05370b7cf9d6b7e3eed1d7c50f9410725414816`; 263 members, stable native before/after hashes and complete gzip CRC passed. The installed RPM is copied locally and hash verified.
- First healthy wrapper run used Arch's `/usr/bin/blkid`; openSUSE supplies `/usr/sbin/blkid`. The original path-error fixture and successful normal close remain retained; fresh corrected v2 passed all six cases.
- First bounded missing-backend wrapper failed its third verification because the parent expected dictionary was not advanced after the forked append worker. Both earlier product close controls had passed. The remaining mount was closed normally, original evidence was retained, and a fresh v3 fixture passed all three close methods with the corrected expectation update.
- The initial local `nohup` coordinator launch did not execute or create any native case. Its empty startup log is retained; the recorded controller was then launched under a persistent foreground exec session.

Final cleanup passed: no affected plaintext mounts, live session records or owned crypto/FS/desktop processes remain; every retained ciphertext source lock is available; the callback bind was normally unmounted; private X11 authority/socket/lock were removed; both backend components and installed package still match. Final free space is 10,027,995,136 bytes on root and 40,741,183,488 bytes on the utility filesystem, above the continuous 2 GiB admission floor.

Scope limitations: this follow-up qualifies the six affected unprivileged VeraCrypt file routes and reviewed failure/frontend controls. It does not add unprivileged physical devices, LUKS or other filesystem combinations, foreign-user denial evidence, new feature-completeness claims, a new reboot gate or refreshed broad privileged matrix evidence. Older limitations and configured native prerequisite distinctions remain in the original completion index. The root's existing seven-reboot audit retains its original 678 scope.
