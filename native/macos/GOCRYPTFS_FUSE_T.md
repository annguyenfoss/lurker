# gocryptfs FUSE-T qualification on macOS

The macOS package build uses the Darwin build tag `lurkerfuset` and selects
FSKit through the signed FUSE-T 1.2.9 helper. This note records the direct
backend experiments that qualified the transport.
[Evaluation plan](../../DIRECTIVES/PLAN/MACOS_FOLDER_FUSE_T.md)

## Native results

Buran ran macOS 27.2.1 arm64 with Go 1.27.4, FUSE-T 1.2.9, and SQLite 3.53.1.
The first build and its diagnostic rerun both mounted through FSKit. The
diagnostic run verified WAL, a FULL commit, second-connection reads, a TRUNCATE
checkpoint, and persistence across reopen. Rename returned EIO. The FUSE trace
showed an extended-rename capability in INIT and source filename bytes in the
decoded rename flags. The corrected candidate negotiates compatibility rename
with FUSE-T and decodes the request layout from the negotiated capability.
It retains ordinary unmount.

| Run | Revision | Direct FSKit evidence |
| --- | --- | --- |
| `20261123T074039Z-cfea1fe7` | `bea7881` | Verified FSKit mount. Rename returned EIO after the write, sync and truncate workload. |
| `20261123T074246Z-c28fa49a` | `56a9135` | Reused the `bea7881` binary. SQLite WAL and file data survived close and reopen. Rename EIO reproduced with the FUSE protocol trace. |
| `20261123T074452Z-8afe8e65` | `4efd9de` | Compatibility and extended rename layout regression cases passed. The direct backend passed write, sync, truncate, rename, SQLite WAL/FULL/second connection/checkpoint, and exact payload plus SQLite integrity and committed-data checks after reopen. |

The passing probe verified the FSKit option in the macOS mount table and the
process identity of its FUSE-T helper. Both ordinary closes left the fixture
mountpoint detached and the helper gone. The boot identity was unchanged.
Evidence is under `target/goal-validation/macos-folder-fuse-t/gocryptfs/`.

## Build provenance

- gocryptfs v2.7.0 commit `756cd453d7afe69fd2a00a7592fd2d9869a6f756`.
  https://github.com/rfjakob/gocryptfs/tree/756cd453d7afe69fd2a00a7592fd2d9869a6f756
- Go-FUSE commit `2d50fff207d78c197869fa492eff3a79bc1d8408` with experiment
  patch SHA-256 `08ea059329c0460afd8f5c5e0c3fcd5d5025baf30e1197bde6448b54f62a00a8`.
  The earlier vendor Go-FUSE fork provides the two-socket transport precedent.
  https://github.com/macos-fuse-t/go-fuse/tree/37474d962a878555599c596487277ad7b1266a70
- Built gocryptfs SHA-256
  `8fb29f2cc8460308beacb610474a060c666d123916a420dd1fd2e7e7085512e5`.
  `otool -L` reports the macOS system and resolver dylibs.
- Installed FUSE-T `go-nfsv4-1.2.9` SHA-256
  `348361a699b768d555ec41803e492075b9179691dbfac6b9020db5c20add5fa7`.
  The probe verifies the binary against the release and checks its code
  signature before opening a mount.
  https://github.com/macos-fuse-t/fuse-t/releases/tag/1.2.9

## Reproduction

Use one coordinated Buran window with the installed FUSE-T 1.2.9 runtime and
FSKit extension. Place the build, caches and fixtures in private locations.
The exact passing Lurker source was
`4efd9de2d0cae56ef96ba96fe3e8ff5735f920bd`. In the native runner recipe,
use the following stages from that source root.

```json
{
  "suite": "macos-gocryptfs-fuse-t",
  "privilege_route": "login-user",
  "stages": [
    {
      "name": "gocryptfs-fuse-t-build",
      "timeout": 1200,
      "commands": [
        ["{python}", "scripts/test-macos-gocryptfs-fuse-t.py", "build", "--cache", "{native_cache}/go", "--output", "target/gocryptfs-fuse-t", "--jobs", "2"]
      ]
    },
    {
      "name": "gocryptfs-fuse-t-fskit-probe",
      "timeout": 180,
      "commands": [
        ["{python}", "scripts/test-macos-gocryptfs-fuse-t.py", "probe", "--output", "target/gocryptfs-fuse-t", "--transport", "fskit"]
      ]
    }
  ]
}
```

Run the recipe with the tracked macOS target. The build stage verifies the
upstream pins, applies the patch, and runs the focused rename regression.

```bash
python scripts/remote-test.py start --hosts macos \
  --commit 4efd9de2d0cae56ef96ba96fe3e8ff5735f920bd \
  --spec "$recipe_path" --concurrency 1 --build-jobs 2
python scripts/remote-test.py collect "$run_id"
```

For an experimental Lurker CLI run, set
`LURKER_EXPERIMENT_FUSE_T_TRANSPORT=fskit` and point
`LURKER_BACKEND_DIR` at the absolute output directory holding the verified
binary. The direct probe's FSKit result applies to that pinned helper and
candidate. SMB, NFS, packaged desktop, and reboot coverage are outside this
direct probe.
