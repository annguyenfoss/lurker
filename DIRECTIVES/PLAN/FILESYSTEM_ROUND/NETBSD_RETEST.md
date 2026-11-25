# NetBSD filesystem retest audit — 2026-11-25

No unachieved promised NetBSD filesystem cell remains. No native test, build,
remote command, package operation, or host mutation was performed in this
round, honoring the instruction not to retest already achieved routes.

Audited repository head: `b474181c1e410cd44c3351a374ef47c5117bb0c6`.
Scope: README filesystem planning cells, `NETBSD_RESULTS.md`, selected prior
receipts and their independent review, and the current implementation. The
README describes filesystem capability research; this audit does not expand
its claims into complete metadata or feature qualification.

| README NetBSD cell | Existing CGD evidence | Retest decision |
| --- | --- | --- |
| ext4 — ☑️ | Passed; restricted experimental e2fsprogs/fuse-ext2 profile | Already achieved; omit rerun |
| Btrfs — unsupported | No established route; explicitly unsupported | No promised capability to test |
| XFS — unsupported | No established route; explicitly unsupported | No promised capability to test |
| APFS — unsupported | No established route; explicitly unsupported | No promised capability to test |
| UFS1 / FFSv1 — ✅ | Passed; base newfs/mount_ffs with WAPBL | Already achieved; omit rerun |
| UFS2 / FFSv2 — ✅ | Passed; base newfs/mount_ffs with WAPBL | Already achieved; omit rerun |
| HAMMER2 — ☑️ | Passed; unchanged upstream write-enabled module/tools | Already achieved; omit rerun |
| NTFS — ☑️ | Passed; ntfsprogs/NTFS-3G | Already achieved; omit rerun |
| exFAT — ✅ | Passed; fuse-exfat | Already achieved; omit rerun |
| ZFS / OpenZFS — ☑️ | Passed; base ZFS scoped import/export | Already achieved; omit rerun |

The seven selected receipts retain exact payload-qualified source
`58c97b0188ee2d604a094acb9ffe0e10ce84de23` and CLI SHA-256
`1de769160ae8ce9ad15580613b966f687914940c23d3d7077140b1c5022195e3`.
All 49 recorded CLI operations succeeded. Each receipt records PDF/text write,
normal close/reopen, original-content verification, text append and second-file
creation, another normal close/reopen, three matching final SHA-256 hashes,
and no remaining mount. Receipt identities and hashes were reconciled directly,
without rerunning those operations. The prior independent native source review
at `9b1bc0d816564b721c730f6e8c43039acc14a201` remains identified separately.

The current NetBSD adapter and filesystem module, shared BSD dispatch and
filesystem module, API/model/credential definitions, library entry point and CLI
sources are byte-identical to the payload-qualified revision. Other OS-specific
changes and the added executable search fallback in `system.rs` do not add or
remove a NetBSD filesystem route; NetBSD filesystem commands use absolute
native paths. No product fix was warranted by this audit.

CGD is the only existing NetBSD encryption route. No new engine or directory
backend is required. None of the promised NetBSD rows is read-only-only;
separately unexercised RO modes do not supply a missing cell for this scoped
round. The evidence establishes ordinary content persistence, retaining prior
ext4 experimental/restricted-feature and HAMMER2 installation/size limits.
Full ACL, snapshot, journal and other metadata parity remains outside these
payload receipts. No GUI, physical device, SQLite, fault or reboot case ran.

Private evidence:
`target/goal-validation/filesystem-retest/netbsd/summary.json` records all ten
cells, the seven selected receipt SHA-256 identities, payload hashes, source
comparisons and explicit zero new tests/host mutations. Its sibling `audit.py`
is a read-only receipt/source consistency audit. Existing native fixture and
cleanup evidence remains under
`target/goal-validation/filesystem-round/netbsd/`; remote state was not freshly
inspected, so this audit does not claim fresh installed-artifact or cleanup
verification. There is no remaining NetBSD test action for this round.
