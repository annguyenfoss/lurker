# NetBSD fresh qualification — 2026-11-27

Status: in progress. Prior-round receipts are not fresh passes.

The assigned host is `adjutant@netbsd-core-01`, running NetBSD 11.1 amd64,
GENERIC dated 2026-11-02. The current formal release and release-specific
security page were checked; the latter lists no advisories yet. `pkgin update`
refreshed the configured NetBSD 11.0 binary repository and both full-upgrade
and upgrade reported nothing to do. The initial root filesystem had 14 GiB free.
`adjutant` is not in sudoers; the existing root SSH inventory key supplies scoped
administration without changing authorization configuration.

- Frozen product source: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
- Fresh workspace run: `20261126T203531Z-27ee973f`.
- `cargo check --workspace`: passed.
- `cargo test --workspace`: 105 passed, 0 failed, 5 ignored.
- CLI/helper build: passed; reviewed binaries are installed in the isolated,
  root-owned `/opt/lurker-native-fresh-20261127-netbsd` prefix.
- Physical partition parser `c6fd18e`: native core check and five targeted tests
  passed. Shared product device lifecycle and final candidate checks remain pending.

Fresh CGD downloaded-payload results against the frozen baseline:

| Inner filesystem | Container outcome |
| --- | --- |
| FFSv1 | Passed |
| FFSv2 | Passed |
| ext4, restricted fuse-ext2 profile | Passed |
| exFAT | Passed |
| NTFS-3G | Failed: native PUFFS payload I/O stalled |
| HAMMER2, native matching external module | Passed after enabling upstream write option |
| ZFS, base kernel module | Passed |
| Btrfs | Unavailable: no identified native Create + Read/Write driver |
| XFS | Unavailable: formatter package exists, native mount driver not found |
| APFS | Unavailable: no native formatter and writable driver |

Each passing payload receipt covers create, real read/write mount, downloaded
Bitcoin/Ethereum/Transformer PDFs plus text, normal close, reopen and digest
verification, append/new file, another close/reopen and final digest verification.
Explicit and Auto CGD conformance each passed 12/12, including existing-target
protection, wrong password, duplicate/aliased source and busy close. CGD retains
its fixed AES-XTS/Argon2id password profile and mandatory parameter sidecar;
alternate ciphers and keyfiles are outside its implemented profile.

The latest upstream HAMMER2 v1.2.13, commit
`64095c3947f246f3eb6a8d7341ba856e057b6a6c`, was built against the running kernel.
NetBSD 11.1 requires explicit `-lcrypto` for the upstream tools' SHA-256 aliases.
The upstream `HAMMER2_WRITE=1` build option is necessary: the default module
returns EOPNOTSUPP from its write routine. The initial default-build failure is
retained separately; the enabled-write module passed the unchanged synced payload.
This is an ordinary matching external driver build, not a customized kernel.

The NTFS driver stall was recorded with exact owned worker/backend identities.
Scoped SIGTERM of the owned NTFS daemon released the blocked requests, after
which ordinary product cleanup completed. No forced unmount or pass credit was
used. Its fresh bounded reproduction and native control remain pending.

A new 2 GiB qcow2 was attached to `netbsd-core-01`, target `vdb`, serial
`lurker-fresh-netbsd`, at `/srv/build/lurker-native-fresh-20261127/netbsd/scratch.qcow2`.
The guest's unique 512-byte marker matched the preattachment host marker before
any GPT write. The new GPT wedge is `/dev/dk2`, parent `ld1`, offset 2048,
4190208 sectors; partition UUID `50da81c6-56b2-4c62-9251-4cc1481e0e46`.
A direct native CGD/FFSv2 control passed the complete downloaded-payload lifecycle
on this partition and cleaned up normally. This is a native control, not a Lurker
physical-device pass.

The first ordinary reboot activated the new disk, but exposed a qualification
setup error: the host's default `clear_tmp=YES` deleted the round's `/tmp` root,
including the armed persistence image and remote failed fixtures. That reboot
persistence attempt is invalid/incomplete. Collected receipts/logs and installed
artifacts survive, but the failed NTFS image was lost. Subsequent fixtures and
reproductions use persistent `/var/tmp/lurker-native-fresh-20261127-netbsd/`.
A new persistent UFS2 reboot fixture is being exercised; no reboot pass is yet claimed.

The 130-cell applicability ledger records every requested filesystem/backend pair.
Stable CryFS native source integration remains blocked/unqualified. Native driver
limitations and unavailable encryption engines are not counted as support.
Private evidence is under `target/goal-validation/native-fresh-20261127/hosts/netbsd/`;
its checkpoint identifies artifact hashes, receipts, resource identities and next work.
The common payload harness revision is recorded separately from the product source.

Eligibility references:

- https://www.netbsd.org/releases/formal.html
- https://www.netbsd.org/support/security/patches-11.1.html
- https://github.com/kusumi/netbsd_hammer2
- https://wiki.netbsd.org/zfs/
- https://man.netbsd.org/dm.4
- https://veracrypt.io/en/Supported%20Operating%20Systems.html
- https://github.com/rfjakob/gocryptfs
- https://github.com/cryfs/cryfs/tree/1.0.3
- https://github.com/cryptomator/fuse-nio-adapter
- https://github.com/sgan81/apfs-fuse
