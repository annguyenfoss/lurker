# Cross-platform implementation status

Work in progress, 2026-11-14. The full objective remains all approved backend/OS pairs; the final combined-revision matrix is not yet complete. The scope/profile authority remains `DIRECTIVES/LLM/CIPHER.md`.

## Implemented architecture

- Shared command model/platform allowlist; shared directory lifecycle; shared BSD container lifecycle with small native adapters; common private VeraCrypt profile and UTF-8 patch.
- Private Linux creator/userspace helpers, native macOS/BSD VeraCrypt builds, pinned Go/CryFS builds and private Cryptomator JRE.
- Administrator-controlled executable resolution, identity-checked sessions, atomic publication, journal-before-filesystem initialization and busy-safe teardown.
- Exact-commit remote jobs, separate Cargo/native caches, concise JSON evidence and verified Unix/macOS staging.

## Development evidence (not one final release matrix)

| Target | Evidence |
| --- | --- |
| Arch Linux | Earlier nine container/cipher cases and three userspace suites pass; all three directory CLI suites pass. Full staged-package rerun in progress. |
| FreeBSD | GELI, PEFS and gocryptfs native CLI checks pass; private VeraCrypt all cipher/credential combinations pass at adapter level. Complete package staged; packaged CLI qualification in progress. |
| NetBSD | CGD CLI checks and real acquired-mapping failure/recovery regression pass. |
| DragonFly | LUKS1 AES/Serpent/Twofish CLI suites pass. |
| OpenBSD | Native CRYPTO CLI checks pass; staged VeraCrypt package passes 39 explicit CLI cases. Unsupported read-only routes fail closed. |
| macOS | CLI/helper/desktop and signed development DMG build. Packaged Apple CLI checks pass; final patched VeraCrypt package checks in progress. Directory runtime qualification needs macFUSE; FUSE-T failed SQLite WAL checks. |
| Other Linux | Ubuntu/Fedora/openSUSE/Gentoo core suites each pass 47 tests at `e6274b9`; Debian workspace/desktop passes 63 tests at `6147294`. These are build/unit results, not native cipher qualification. |

Detailed evidence and exact source/artifact identities: `docs/backend-testing.md`, `docs/linux-userspace.md`, `docs/macos-backends.md`, and ignored remote job/result directories. Preserve failed fixtures and reports; do not combine passing results from different commits to claim a final matrix pass.

## Next gates

1. Finish Linux source-inode serialization and cross-mode locking; rerun the complete staged artifact, including alias and recovery checks.
2. Qualify safe native BSD/credential-authenticated VeraCrypt auto detection, then packaged explicit/auto CLI checks.
3. Freeze one candidate revision and run the final applicable primary-host matrix and current workspace/desktop checks. Keep macOS limitations explicit under the user's best-effort allowance.
4. Reconcile every owned test mount/process; retain concise results, artifact digests and documented host prerequisites. Public release signing, clean-machine/fault qualification and any unfinished selected pair remain explicit gates rather than inferred successes.
