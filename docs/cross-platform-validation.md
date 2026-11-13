# Frozen-source native build validation

Application source: `76f05fbd2167313273c53201a070252719e4f4ea` (2026-11-14). Every recorded job verified the exact deployed Git snapshot. Subsequent documentation and test-harness changes do not replace the application source identity.

Status: ten hosts passed; Arch Linux is waiting for the Linux package runtime owner to release its exclusive test window.

Core checks compile and test `lurker-core`, CLI and helper. Workspace checks also cover the Slint desktop. Ignored native encryption tests were not executed by these jobs; packaged cipher lifecycle and durability suites have separate evidence in `docs/backend-testing.md`, `docs/linux-package-testing.md` and `docs/macos-backends.md`.

| Host | Scope | Passed | Failed | Ignored | Run |
| --- | --- | ---: | ---: | ---: | --- |
| archlinux-core-01 | Pending exclusive-window release | — | — | — | — |
| macOS (buran) | Workspace | 58 | 0 | 8 | A |
| freebsd-core-01 | Core | 44 | 0 | 5 | B |
| dragonflybsd-core-01 | Core | 33 | 0 | 1 | C |
| netbsd-core-01 | Core | 35 | 0 | 2 | C |
| openbsd-core-01 | Core | 42 | 0 | 9 | B |
| debian-core-01 | Workspace | 79 | 0 | 12 | D |
| ubuntu-core-01 | Core | 55 | 0 | 12 | E |
| fedora-core-01 | Core | 55 | 0 | 12 | E |
| opensuse-core-01 | Core | 55 | 0 | 12 | E |
| gentoo-core-01 | Core | 55 | 0 | 12 | E |

Recorded total: **511 passed, 0 failed, 85 ignored** across ten completed hosts. Debian used debug information disabled and incremental compilation disabled to reuse its qualified workspace cache within the VM disk budget. Cargo dependency caches are separate from native-backend source/patch identities.

Run identifiers:

- A: `20261113T224831Z-cf46ce61`.
- B: `20261113T224832Z-4792ea51`.
- C: `20261113T225017Z-6bdc66c4`.
- D: `20261113T225202Z-d0a9de45`.
- E: `20261113T225203Z-1c05422c`.

The local aggregate is `target/remote-test/final-matrix-result.json`; each run retains its manifest, result JSON, exact source hash, toolchains, stage exits and log references. Generated artifacts remain ignored by Git. `docs/remote-testing.md` describes reconnecting and bounded log retrieval.

BSD core results do not claim BSD desktop qualification. macOS runtime/FUSE limitations remain documented separately. No runtime encryption tests were performed on endeavour.
