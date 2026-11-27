# Linux follow-up qualification

Frozen product source: `67894f68525c55d5cbb48c743e1a98a308bd970f`.

The change isolates the non-console sudo loop-lease helper into its own process
group while preserving its session and cached TTY authorization. The direct-root
branch, crypto profiles, filesystem adapters and private engines are unchanged.
Arch GNU-sudo and Ubuntu sudo-rs password/keyfile controls passed the exact source
preview. Ubuntu also recorded 168 process samples: same session as caller,
separate background group, no stopped tasks, and normal lease EOF/reap.

## Product gates

Finish live independent `576505d` batches before scheduling competing builds or
PBKDF workloads. Do not restart a live case or relabel its artifact identity.

Build/check/test the workspace from the exact Git archive, then build, install
and verify each native CLI/helper/desktop package. Keep package and component
hashes, engine identities and source/harness revisions in the private evidence.
On the new installed artifacts run:

- Normal-user password and binary-keyfile CLI file lifecycles for LUKS1,
  LUKS2 and VeraCrypt, including reopen, normal close and terminal survival.
- Strict helper JSON and full payload file/device samples for all three disk
  methods; these also cover the unchanged root branch in the new artifact.
- The native desktop window and default helper callbacks. Identify the runtime
  source separately from any test-only fixture overlay.
- The planned ordinary reboot cohort on the installed candidate after its
  required host prerequisites and resource admission checks pass.

Earlier fresh full filesystem matrices remain evidence for their named
`576505d` artifacts and unchanged branches. Carry forward only explicitly
identified unchanged coverage; do not present those receipts as executions of
this new package. New failures determine any broader rerun scope.

## Host FUSE packaging gate

The older Linux creator closure copied its FUSE library into Lurker. That
contradicts this round's host-only FUSE requirement; historical package
installation/runtime successes do not establish final package compliance.
The newer audit correctly rejects that payload, but its closure validator must
also recognize the dependency as a qualified host provider.

Ubuntu owns the Linux producer and its tests; FreeBSD owns the matching Linux
stager consumer and tests. They coordinate a validated host-library inventory,
native architecture/SONAME, administrator-controlled provider and actual loader
resolution. Use their named packaging revision separately from Rust source
`67894f6`; do not remove files without updating/verifying provenance, weaken the
ban, or bypass it with an older stager. Build hashes describe provenance rather
than prohibiting compatible administrator updates.

The creator's exact ABI must exist on its host. In particular, openSUSE provides
SONAME 3 rather than the old donor's SONAME 4; rebuild the pinned creator against
the ordinary native provider instead of substituting an incompatible library.
The final package inventory must contain no FUSE runtime or filesystem driver.

## UFS2 host prerequisite

Debian reproduced a missing directory entry after successful create/fsync/stat
on a clone of the retained failed physical fixture. Fedora identified an early
EOF for every nonzero readdir offset in the pinned host FUSE-UFS driver.
This is a separate prerequisite correction, not an encryption or profile change.

Fedora owns the minimal pagination fix and native unit/multi-page/new-entry
controls. Distribute the verified source/patch/build recipe and hashes to all six
Linux owners. Install only after owned sessions using the prior driver close;
never bundle the filesystem driver in Lurker.

After the corrected prerequisite is qualified on each host, rerun the affected
UFS2 container and physical-device cases: three disk methods and three directory
engines on UFS2 backing, with independent version/driver evidence and the full
PDF write/close/reopen/append/new-file/close/reopen hashes. Prefer the new installed
product artifact when scheduling these cases. Retain original failure receipts
and raw snapshots. Reconcile source kind, engine identity and prerequisite
revision independently; unrelated filesystem rows need no repetition solely
because the driver or product source hash changed.
