# Cross-platform implementation status

Work in progress, 2026-11-14. Runtime candidate: `76f05fbd2167313273c53201a070252719e4f4ea`. Packaged native suites and disposable-VM shutdown characterization are running. The scope/profile authority remains `DIRECTIVES/LLM/CIPHER.md`.

## Implemented architecture

- Shared command model/platform allowlist; shared directory lifecycle; shared BSD container lifecycle with small native adapters; common private VeraCrypt profile and UTF-8 patch.
- Private Linux creator/userspace helpers, native macOS/BSD VeraCrypt builds, pinned Go/CryFS builds and private Cryptomator JRE.
- Administrator-controlled executable resolution, identity-checked sessions, atomic publication, journal-before-filesystem initialization and busy-safe teardown.
- Exact-commit remote jobs, separate Cargo/native caches, concise JSON evidence and verified Unix/macOS staging.

## Development evidence (not one final release matrix)

The frozen candidate passes the following build/unit jobs. Ignored native tests
are not runtime qualification; packaged CLI evidence is tracked separately.

| Candidate build target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| macOS workspace, including desktop | 58 | 0 | 8 |
| Debian workspace, including desktop | 79 | 0 | 12 |
| Ubuntu / Fedora / openSUSE / Gentoo core, each | 55 | 0 | 12 |
| FreeBSD core | 44 | 0 | 5 |
| OpenBSD core | 42 | 0 | 9 |
| NetBSD core | 35 | 0 | 2 |
| DragonFly core | 33 | 0 | 1 |

These ten jobs total 511 passing tests and 85 ignored tests. Arch's final build
job waits for its exclusive package-test window. Exact run identities are in
`target/remote-test/final-matrix-result.json` (ignored).

| Target | Evidence |
| --- | --- |
| Arch Linux | Candidate cross-mode source exclusion passes eight checks. All-cipher, owned-loop device-create and final directory-crash package reruns remain in progress. Earlier native userspace suites and directory lifecycles pass. |
| FreeBSD | GELI, PEFS and gocryptfs native CLI checks pass; private VeraCrypt all cipher/credential combinations pass at adapter level. Complete package staged; packaged CLI qualification in progress. |
| NetBSD | CGD CLI checks and real acquired-mapping failure/recovery regression pass. |
| DragonFly | LUKS1 AES/Serpent/Twofish CLI suites pass. |
| OpenBSD | Native CRYPTO CLI checks pass; staged VeraCrypt package passes 39 explicit CLI cases. Unsupported read-only routes fail closed. |
| macOS | Packaged Apple CLI checks pass. Private VeraCrypt builds, but the tested FUSE-T runtime has creation timeouts; its runtime remains unqualified. Directory qualification needs macFUSE; experimental FUSE-T failed SQLite WAL. A matching final development DMG is being assembled. |
| Other Linux | Candidate build/unit results are above; they do not establish native cipher qualification on these distributions. |

Detailed evidence and exact source/artifact identities: `docs/backend-testing.md`, `docs/linux-userspace.md`, `docs/macos-backends.md`, and ignored remote job/result directories. Preserve failed fixtures and reports; do not combine passing results from different commits to claim a final matrix pass.

## Next gates

1. Finish the frozen-candidate Linux/BSD packaged CLI matrix, including Auto detection and alias guards. PTY transport uses harness `8bf9f0e`, which fixes a prompt/echo race independently of the runtime artifact.
2. Characterize abrupt poweroff using isolated owned VM copies and persistent guest fixtures. Ordinary tests use private `/tmp` directories; no original VM is stopped or repaired.
3. Reconcile every owned test mount/process and retain concise reports and artifact digests. Linux pre-mapper SIGKILL can retain a loop and fail closed; BSD private VeraCrypt daemon-death cleanup and full mounted CryFS storage-exhaustion recovery remain unqualified.
4. Keep macOS runtime limits explicit under the user's best-effort allowance. Public signing, clean-machine release qualification, full-home/session integration and untested fault behavior are separate gates.
