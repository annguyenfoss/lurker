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
- CLI/helper artifact build, native filesystem cases, scratch-device cases and
  final candidate qualification remain pending; compilation is not a lifecycle pass.

Seven implemented CGD inner-filesystem routes are queued: FFSv1, FFSv2,
restricted ext4 through fuse-ext2, exFAT, NTFS-3G, native HAMMER2 and base ZFS.
CGD keeps its fixed AES-XTS/Argon2id password profile and mandatory parameter
sidecar. Alternate ciphers and keyfiles are outside its implemented profile.
The current source rejects physical-device creation; native controls will precede
any adapter change. No device acceptance is claimed.

The ordinary HAMMER2 external module and ZFS stock module load on this kernel.
A fresh build of upstream HAMMER2 v1.2.13, commit
`64095c3947f246f3eb6a8d7341ba856e057b6a6c`, is queued with matching installed
kernel headers. This is an external driver build; the GENERIC kernel is unchanged.
Btrfs, XFS and APFS lack an identified native Create + Read/Write route. The
130-cell applicability ledger records every requested filesystem/backend pair;
stable CryFS native source integration remains blocked/unqualified, not a pass.

Private evidence is under `target/goal-validation/native-fresh-20261127/hosts/netbsd/`.
Remote evidence is under `/tmp/lurker-native-fresh-20261127-netbsd/`.
The checkpoint records live process handles and exact next steps. Common payload
uses the downloaded Bitcoin, Ethereum and Transformer PDFs plus plain text;
the shared payload harness is from `c5feaec` and is identified separately.

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
