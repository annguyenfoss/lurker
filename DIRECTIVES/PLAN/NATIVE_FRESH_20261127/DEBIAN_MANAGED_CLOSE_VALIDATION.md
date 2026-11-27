# Debian shared managed-close source regressions

Both gates passed on `debian-core-01`, Linux `6.12.118+deb13-cloud-amd64`, as
UID/EUID 1300. They used existing native tools and private build directories on
the retained build-storage volume. No FUSE mount, device operation, installation,
elevation or reboot was performed. The earlier filesystem qualification and
installed 576/678/e927 artifact scopes are unchanged.

## Shared Java SDK

Inputs were base `e516ba81b2eff561374cbe572be59afa37977af1` plus the exact 14-file
`shared-inputs-2` overlay `0c2b29e74f1389dc497f06070f87d55cfb96c9cbfb86da3d58684a0bc53c2ae4`.
The untested Java interoperability fixture was absent. Native Temurin 25.0.5+8
and Maven 3.9.11 ran frozen `scripts/build-cryptomator.py --test` with a private
offline Maven wrapper/cache, one worker and a 768 MiB Maven heap.

`BackendTest` passed 9 tests and `ManagedCloseTest` passed 7: **16 passed,
0 failures/errors/skips**. The Linux patched modules, locked dependencies,
private `jdk.net` runtime, runtime verification, version and strict capability
probes passed. The legacy Linux provider/profile stayed unchanged; Linux reports
`managed_close_protocol: 0`. All **165 SDK inventory hashes** matched. Manifest:
`cab751096d4140dfaf2a223a3697aad97294ed53471751f6a0072d9ef4c6be97`.

The first invocation passed the same 16 tests, then stopped at a missing offline
CryptoFS source JAR. Both pinned source JARs were recovered from an existing cache
and checked against the frozen patch hashes. A guarded classifier-origin probe
aborted before copying; the final import preserved the locally untracked origin
without inventing remote metadata. No network fetch, source or pin change was
needed. The initial failure, reports and import provenance remain retained.

## Actual Rust parent workspace

Inputs were the same base plus the exact five-file Rust overlay
`99513921a30f638d8f3fd744f35094900a8419e1532999f3ca3aee1590f6dc23`.
All five hashes were retained and rechecked. Native stable Rust/Cargo 1.100 used
`cargo check --locked --offline --workspace` and
`cargo test --locked --offline --workspace`: **199 passed, 0 failed, 15 ignored**.
All **13 managed socket tests** passed against the real parent workspace types.
No ignored/native-storage test or FreeBSD-only Java interoperability case ran.

The existing Cargo target cache `remote-test/cargo/2333b62c66b4774e0e63/target`
was reused with jobs 1, dev/test debug 0 and incremental compilation disabled.
The Java and Rust gates ran sequentially with a continuous 2 GiB disk floor.

## Evidence and cleanup

Private evidence is under `target/goal-validation/native-fresh-20261127/hosts/debian/build/`:
`cryptomator-managed-close-linux-1/` retains the first failure;
`cryptomator-managed-close-linux-2/` contains the SDK, test reports, raw Maven/build
logs, tool/cache/source receipts and `coordinator-audit.json`;
`cryptomator-managed-core-linux-1/` contains workspace logs, all 13 named socket
results, five source snapshots and `cleanup.json`.

SDK evidence archive SHA-256:
`f2627eb7dc8ac0a8d703523c092ebe3c3e95d16db3e846983e4fa922c28a6f86`.
Rust evidence archive SHA-256:
`8811941693e01d14e967861f022b8862cbcb726594b1ee2b84ed468b4693581b`.
Archive CRCs, executed log hashes and retained artifact/source hashes passed
independent read-only audit. Final inspection found no test/compiler workers;
the empty owned Java fixture roots were removed. Source, caches, SDK and evidence
remain retained. These are source/build regressions; they do not admit the new
FreeBSD provider or qualify real mount/unmount behavior. No other ready deferred
Debian filesystem task was found in the recorded plan.
