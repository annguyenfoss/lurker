# DragonFly BSD VeraCrypt implementation plan

## Scope and baseline

Implement privileged create, mount, list, and close for normal VeraCrypt file containers
on DragonFly BSD, using `tcplay` for header creation and authentication and DragonFly's
device mapper for sector I/O. The CLI and the desktop use the same `lurker-core` route
through their existing entry points. Start qualification on DragonFly 6.4.2 x86-64, the
current project test baseline.

- Create with AES by default, with Serpent and Twofish as choices. Fix the profile to
  XTS, VeraCrypt PBKDF2-HMAC-SHA-512 (`SHA512-VC`, 500,000 iterations, PIM 0), and full
  initialization. Accept either a password of up to 64 UTF-8 bytes or one non-empty
  regular binary keyfile. Reject an overlong password before invoking the engine.
- Use FAT32 as the proposed first create and import filesystem. It gives the containers
  Lurker creates a writable filesystem shared with the VeraCrypt targets in the
  interoperability tests. Qualify it before fixing the default. Cover its 4 GiB minus
  one byte per-file limit and permission semantics. A container with a different inner
  filesystem needs separate filesystem qualification even when its VeraCrypt header
  opens.
- Qualify imports of the create profile first. Treat other formats `tcplay` opened in
  the exploratory tests as additional test inputs, and list only qualified profiles in
  user documentation. Create selects LUKS1 by default and VeraCrypt explicitly.
- The exploratory run used stock `tcplay` 3.3 and disposable FAT volumes. It verified
  both directions with Linux and macOS for all three ciphers, and with FreeBSD and
  OpenBSD for AES. Linux-created SHA-256, Whirlpool, and AES–Twofish fixtures also
  mounted and accepted writes. Argon2id, BLAKE2s, Camellia, PIM 501, and an 80-byte
  password failed to unlock. Windows evidence comes from format compatibility. These
  engine results precede the Lurker lifecycle and release package qualification. Local
  logs are under `target/goal-validation/dragonfly-veracrypt-interop/`.

## 1. Qualify the tcplay wrapper

1. Record the kernel release, stock `tcplay` and library identity, device-mapper
   interface, and available base binaries. Repeat one password and one binary-keyfile
   create, write, close, and reopen as the stock-engine control. Confirm header
   metadata, the authenticated payload offset and length, byte preservation, and the
   resources the engine creates.
2. Prototype a narrow wrapper around the `libtcplay` task API for noninteractive
   creation and authentication. Test bounded credential delivery from the Rust adapter
   through an inherited pipe and a structured result. On 6.4.2 the base CLI requires
   interactive create confirmation, uses `/dev/crypto` for header processing, and can
   change the system-wide `kern.cryptodevallowsoft` setting even during a root version
   probe. The base library shares that crypto path. Prefer the OS-provided
   implementation on releases where it meets this contract. For 6.4.2, evaluate a
   private build from pinned `tcplay` source with DragonFly's upstream userspace
   header-crypto change backported. Use the upstream VeraCrypt format and `tcplay`
   cryptographic primitives. Prove the packaged wrapper works with the setting untouched
   and that capability checks carry no side effects.
3. Verify that the authenticated result can identify the normal VeraCrypt profile and
   its geometry before any writable mapping is created. If the public task API lacks a
   required field or operation, use a minimal reviewed patch at the private wrapper
   boundary. Validate size, offsets, alignment, range and supported sector size against
   the open backing device. Let the engine own header decoding and key handling. Load
   mappings through the trusted native interface. Keep passwords, keyfile bytes, and
   volume keys out of arguments, environment, session journals, command output, and
   logs.
4. Prove read-only at the mapper and at the filesystem independently, including rejected
   writes and an unchanged ciphertext digest. If the native interface cannot enforce
   both, reject read-only before attachment and report the limitation explicitly. Verify
   that unsupported KDFs, unsupported ciphers and wrong credentials leave the source
   unchanged. Where the engine cannot distinguish an incompatible encrypted header from
   a wrong credential, present both possibilities in the diagnostic.

- DragonFly command reference. https://man.dragonflybsd.org/?command=tcplay&section=8
- DragonFly 6.4.2 header crypto source. https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/lib/libtcplay/crypto-dev.c
- Userspace header crypto change. https://github.com/DragonFlyBSD/DragonFlyBSD/commit/c06d1031abbee2577e05da83882039fa6d19302a
- Userspace header crypto follow-up. https://github.com/DragonFlyBSD/DragonFlyBSD/commit/ede102cd94449fe52fa9da25631d9f15af6d62ef

## 2. Integrate the native lifecycle

Record the DragonFly file-container route as selected for implementation in the internal
scope matrix when coding begins. Public support follows package qualification.

1. In `crates/lurker-core/src/bsd.rs` and `crates/lurker-core/src/bsd/dragonfly.rs`,
   reuse the shared BSD staging and session registry, DragonFly's vnode reservation and
   backing identity checks, and its device-mapper identity checks. Add a
   DragonFly-specific VeraCrypt adapter for the engine calls and format-specific state.
   Apply the [encryption profile](../LLM/ENCRYPTION.md) through the existing create
   dispatch and record any DragonFly-specific behavior there when it is implemented.
2. For create, reserve a same-directory staging file, fill it with random data, and
   sync it before vnode attachment. Create the normal VeraCrypt header with the fixed
   profile, authenticate and map the image, and format the verified decrypted provider
   with the qualified native FAT32 formatter. Flush and close the owned resources
   before atomically publishing the image. Preserve the
   existing destination and no-clobber semantics, including `--force` handling and
   invoking-user ownership. Record recoverable state before each operation that may
   leave an attached resource.
3. For open, support explicit VeraCrypt and Auto. Keep an identified LUKS1 source on its
   existing route, including failures. Use the supplied credential to identify VeraCrypt
   during unlock. Probe the filesystem on the authenticated mapping and mount only a
   qualified filesystem. Verify mount identity, mode, and login-user access before
   reporting success. Test permission and ownership behavior through the actual desktop
   helper as well as the CLI.
4. Store the source, vnode, mapper, mount, and staging identities needed for close and
   recovery. Reuse the shared source locking and reject overlapping opens, path aliases,
   or replaced resources as appropriate. Close in order by unmounting the filesystem,
   removing the mapping, and detaching the vnode. Preserve recoverable state on busy or
   failed close. Recovery may remove only resources with a verified ownership chain.
   Support retry after partial close and keep older LUKS1 journals readable.
5. Connect native capability discovery, active-volume listing, and the installed desktop
   helper. Expose operations the qualified backend can actually perform. Route close by
   the recorded backend for requests by source or mountpoint. Add the selected route to
   native packaging in `scripts/stage-unix.py`. If the 6.4.2 wrapper needs a private
   build, give it pinned sources, patch and binary hashes, license notices, and trusted
   executable and library discovery. Verify the package contains the wrapper and can run
   offline on a clean target.

## 3. Build and run the reliability matrix

Follow the [development runbook](../LLM/DEVELOPMENT.md) and the
[remote runner guide](../../docs/remote-testing.md). Freeze the source revision, wrapper
inputs, harness, and native package before qualification. Use `scripts/remote-test.py`
with `dragonflybsd` for native builds and the installed package tests. Run
`cargo check --workspace --locked` and `cargo test --workspace --locked` on the target.
Start with a disposable-file smoke test on the primary guest. Use isolated KVM clones
for package installation, destructive fault tests, and reboot tests. Serialize storage
jobs per guest. Keep receipts with the package and wrapper hashes, guest release and
boot identities, source and payload digests, test commands, cleanup inventory, and
applicable skipped cases.

| Test group | Cases and required result |
| --- | --- |
| Core lifecycle | Run all six cipher and credential pairs through the installed CLI with explicit and Auto open. Write the [standard mounted payload](../../scripts/test-mounted-payload.py), close and reopen, append, then reopen and check all five expected hashes. Exercise close by source and mountpoint, repeated cycles, wrong and missing credentials, binary keyfiles with NUL and newline bytes, and UTF-8 passwords at the 64-byte boundary. Verify the 65-byte case fails before attachment and the correct credential still opens the same image. |
| Cross-platform exchange | Use the same qualified FAT filesystem in both directions. For Linux, FreeBSD, OpenBSD, and macOS, create with VeraCrypt, open in Lurker on DragonFly, write a marker, and reopen in the creator to verify it. Create with the installed Lurker package and repeat the exchange in reverse. Cover all three ciphers on DragonFly and Linux and at least AES on each additional OS. Use Windows when a native test environment is available, otherwise record Windows as unverified. Keep upstream and Lurker console builds identified in the receipts. |
| Unsupported imports and read-only | Test Argon2id, BLAKE2s, Camellia, changed PBKDF2 iteration counts and an 80-byte VeraCrypt password as incompatible fixtures. Test a compatible header with an unqualified inner filesystem separately. Confirm an actionable error, unchanged ciphertext and no resources left behind. For read-only, test rejected writes at both layers and unchanged ciphertext, or explicit early refusal when that mode is unavailable. |
| Ownership, busy close and concurrency | Exercise concurrent independent vaults, duplicate open, symlink and hardlink aliases, source replacement, stale name or vnode reuse, and a foreign mapper present. Hold an open file or working directory across a close attempt and verify it fails without detaching. Release it and retry normally. Compare inventories and unrelated image digests before and after. |
| Application behavior and capacity | Reuse the [backend conformance harness](../../scripts/test-backends.py). Verify directory and file renames, fsync, and SQLite WAL with `synchronous=FULL`, including the exact committed rows and `integrity_check` after reopen. Exhaust inner FAT space and outer backing space independently, using isolated fixtures. Exercise full allocation, an append with little space, the FAT32 per-file size boundary, and recovery after space is freed. Require a visible write or sync failure when appropriate and preservation of previously committed bytes. |
| Interruption and recovery | Interrupt after staging, vnode attach, mapper activation, format, mount, unmount, and mapper removal. Exercise both an orderly cancellation and abrupt exit where applicable. Verify atomic publication and existing-destination preservation, correct ownership, retryable journals, safe close of verified resources, and refusal to detach a reused device. Run the DragonFly LUKS1 regression suite after changes to shared discovery or recovery. |
| Ordinary reboot | Leave qualified test vaults mounted on a dedicated KVM guest, record boot and mount identities, and perform an ordinary reboot. Reopen through the installed product and verify payload hashes and committed rows. Record any manual reconciliation or filesystem repair separately. A busy or interrupted shutdown gets its own case. |
| Abrupt power loss | Follow the [power-loss runbook](../../docs/vm-powerloss-testing.md). Use a disposable clone with a previously published vault and cut power after the host observes acknowledged file and directory fsync and a SQLite FULL/WAL commit. Check sparse and fully allocated images on representative HAMMER2 and UFS backing stores. Retain the cut disk and test a hash-verified copy. Compare with stock `tcplay` using the same workload and filesystem, and record read-only recovery, writable reopen, repair and missing bytes separately. Judge normal application durability from the ordinary workload and use extra flushes only to diagnose failures. |
| Installed package and desktop | Install the exact candidate on a clean DragonFly guest and confirm offline operation, trusted wrapper and library resolution, accurate package requirements, and unrelated backends still working. Run the packaged Slint desktop and actual login-user helper callback through create, open, list, write, close and reopen. Check credentials, busy-close feedback, read-only availability, uninstall behavior, and clean inventories. |

Reconcile every result before promotion. A write or sync acknowledged before a
controlled cut and later lost is a failed durability check. DragonFly's LUKS1 power-loss
results make the shared storage path an explicit test target. Use a matched stock
`tcplay` control to locate a shared failure, and keep the VeraCrypt route planned if a
required lifecycle or preservation gate remains unresolved. Record the blocker and
reproducer in [cross-platform status](CROSS_PLATFORMS_STATUS.md).

## 4. Publish the qualified support and limits

1. Keep the README entry planned until the native package passes the required gates.
   Once qualified, update the DragonFly VeraCrypt cell and its short footnote in
   `README.md`. Identify the supported file-container profile, inner filesystem, 64-byte
   password limit, import limits, read-only behavior, and any verified reliability
   limitation that applies to the shipped route. Preserve the single OS dependency
   table. List only actual installable dependencies and built-in binaries, and represent
   the engine as bundled if Lurker ships the private wrapper.
2. Update `docs/usage.md` with the DragonFly create, open, and close workflow,
   compatible VeraCrypt settings for exchange, applicable FAT32 limits, and recovery
   guidance. Update `DIRECTIVES/LLM/ENCRYPTION.md` with the approved backend state,
   fixed create profile, and tested import scope. Record any port pin, build policy,
   offline runtime requirements, and maintenance instructions with the backend. Update
   `docs/backend-testing.md` with the reproducible native matrix and
   `DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md` with package and test receipts.
3. Review the documentation against the exact shipped artifact and tested filesystems.
   Commit the adapter, packaging and verification changes in coherent increments, then
   commit the matching release documentation when results are available.
