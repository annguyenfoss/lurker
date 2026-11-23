# Cross-platform implementation plan

Status: implementation plan, recorded 2026-11-14. This document records the approved scope and acceptance gates. Current implementation and qualification results are tracked in `DIRECTIVES/PLAN/CROSS_PLATFORMS_STATUS.md`; the baseline below is historical. `DIRECTIVES/LLM/ENCRYPTION.md` remains authoritative for backend selection and crypto profiles.

## 1. Required outcome

Build Lurker so users do **not** need a separate VeraCrypt installation on Linux or macOS. Bundle the necessary VeraCrypt-derived backend, supporting libraries and notices with Lurker. Aim for the same arrangement on the selected BSD platforms, FreeBSD and OpenBSD, where native builds and filesystem integration prove feasible.

**macOS FUSE is an accepted external dependency.** Use **FUSE-T** for VeraCrypt, gocryptfs, CryFS and Cryptomator. Use the qualified 1.2.9 helper for gocryptfs. Users install the required runtime separately; bundling encryption engines does not remove that requirement. Do not make a new FUSE implementation, automatic FUSE installation or an in-house FSKit bridge a prerequisite for delivering this plan.

The broader distribution policy is:

- Wrap facilities supplied by the OS when that is the practical native interface: Apple disk images, BSD disk-encryption facilities, host mounts and device discovery.
- Bundle selected third-party encryption backends aggressively when users would otherwise need to install another application. A private executable is a valid bundled backend; standalone distribution does not require linking all encryption code into Rust.
- Deliver the CLI and Slint desktop over the same core and backend implementations. The desktop continues to invoke the internal helper, never the CLI.
- Support transparent, writable encrypted storage during a user session. Applications should use the mounted filesystem directly, without extracting and repacking archives.
- Close storage cleanly during normal session/shutdown handling. Do not promise that sudden power loss flushes pending writes, that shutdown hooks always run, or that mounted encryption protects plaintext from the logged-in account.

This changes the future dependency policy, not the approved cipher profiles. Before code changes adopt bundled discovery, update the current system-binary rules in `AGENTS.md` and `DIRECTIVES/LLM/CODEBASE.md` to describe the new architecture accurately. Preserve current runtime behavior until each replacement passes its acceptance gates.

## 2. Approved backend and physical-device scope

This planning snapshot records the starting integration state: ✅ existing workflow; 🟦 selected for implementation; — outside approved scope. Current packaged CLI qualification is recorded in `DIRECTIVES/LLM/ENCRYPTION.md`, not this historical table.

| Backend / format | macOS | Linux | FreeBSD | DragonFly BSD | NetBSD | OpenBSD |
| --- | --- | --- | --- | --- | --- | --- |
| Apple encrypted disk images | ✅ | — | — | — | — | — |
| LUKS2 | — | ✅ | — | — | — | — |
| VeraCrypt | ✅ | ✅ | 🟦 | — | — | 🟦 |
| CryFS stable 1.x | 🟦 | 🟦 | — | — | — | — |
| gocryptfs | 🟦 | 🟦 | 🟦 | — | — | — |
| GELI | — | — | 🟦 | — | — | — |
| CGD | — | — | — | — | 🟦 | — |
| softraid CRYPTO / RAID1C | — | — | — | — | — | 🟦 |
| PEFS | — | — | 🟦 | — | — | — |
| Cryptomator | 🟦 | 🟦 | — | — | — | — |
| LUKS1 | — | 🟦 | — | 🟦 | — | — |

Do not expand this matrix implicitly while solving packaging problems. Windows, VeraCrypt on NetBSD/DragonFly and other exploratory ports are outside this plan. EncFS, securefs, fscrypt, OpenZFS, plain dm-crypt, Loop-AES, TCG Opal, commercial-only backends, age/rage, encrypted archives, backup tools and rclone crypt remain excluded. LUKS may use dm-crypt internally.

Physical-device encryption setup is in scope wherever feasible on Linux, macOS, FreeBSD, DragonFly BSD, NetBSD and OpenBSD. The current product implements device creation on Linux. Qualify native device setup separately for each applicable backend and OS before exposing it.

On macOS, the scope includes native APFS data-volume setup via `diskutil` and supported startup-volume FileVault management via `fdesetup` and native authorization. The existing `hdiutil` encrypted-image workflow remains a separate backend. See the current scope in `DIRECTIVES/LLM/ENCRYPTION.md`.

## 3. Baseline before this implementation

| Area | Current behavior | Required change |
| --- | --- | --- |
| Linux privileged VeraCrypt create | Calls installed VeraCrypt; formats through the existing kernel workflow | Ship a narrowly scoped private creator and remove the external application requirement |
| Linux privileged VeraCrypt open/close | Uses cryptsetup TCRYPT support and host filesystem tools | Preserve this route; it already avoids the installed VeraCrypt application |
| Linux unprivileged VeraCrypt | Bundled modified VeraCrypt exposes a decrypted FUSE image; bundled LKL mounts its filesystem | Preserve root refusal, protocols, recovery and file-only restrictions; maintain and package dependencies |
| macOS VeraCrypt | Discovers `VeraCrypt.app` or an executable on PATH | Prefer the bundled native backend and private libraries; require only the separately installed compatible FUSE runtime |
| macOS Apple images | Uses OS `hdiutil`/`diskutil` | Retain native wrappers |
| New directory backends | Not integrated | Add shared backend capabilities, lifecycle handling and packaged runtimes |
| BSDs | Selected scope exists in the encryption directive; application integration remains planned | Qualify native core/CLI builds, OS adapters, dependencies and desktop separately |
| Release builds | Linux native-helper builder and macOS DMG builder cover different parts of the product | Introduce a complete per-platform build/staging workflow; a bare Cargo build is not a complete bundled release |

Relevant existing files:

- Shared behavior: `crates/lurker-core/src/{api,model,system,workflow,linux_workflow,userspace,macos,keyslots}.rs`.
- Internal protocol/escalation: `crates/lurker-helper/src/main.rs`; desktop launch and validation: `apps/lurker-desktop/src/{helper,logic}.rs`.
- Existing native build: `scripts/build-linux-userspace.py`, patches under `native/linux-userspace/patches/`, findings in `docs/linux-userspace.md`.
- Existing macOS packaging: `scripts/build-macos-dmg.py`. It currently rejects non-system dylibs rather than collecting a private dependency closure; it does not yet package a VeraCrypt backend.

## 4. Distribution contracts

### Linux

Provide native packages first, with the CLI, desktop, internal helper and selected private backends installed together. Declare required OS packages for kernel-backed crypto/filesystem operations and FUSE support. Do not assume cryptsetup or filesystem formatters are present in every default Linux installation.

A portable archive may carry additional private userspace tools where licensing and compatibility permit, but must state its host requirements. It cannot provide kernel device-mapper/FUSE support, authorize mounts, or replace an installed privileged `fusermount3` simply by shipping an ordinary executable. Do not add a new setuid binary or silently change host permissions to make the archive appear self-contained.

Both native and portable releases must operate without a separately installed VeraCrypt executable. Keep privileged and unprivileged capabilities distinct.

### macOS

The DMG contains a complete Lurker app, private encryption backends, redistributable non-system libraries, required private language runtimes, and notices. No VeraCrypt.app, Homebrew VeraCrypt, Java installation or Go installation is required at runtime.

Users install FUSE separately. Qualify VeraCrypt with FUSE-T SMB loopback and gocryptfs, CryFS and Cryptomator with FUSE-T FSKit. Do not substitute one runtime when the required one is missing. Apple disk-image operations must remain available without FUSE. Missing FUSE disables only the operations that require it and produces an actionable diagnosis.

Build native arm64 and x86-64 artifacts where upstream dependencies support them. Decide on a universal app only after every bundled executable and library has compatible slices. Derive the minimum OS version from the complete dependency set. Sign all nested code before signing the app; notarize and staple public Developer ID releases. Keep ad-hoc developer builds clearly distinct from notarized releases.

### BSDs

Produce native packages for each supported OS release/architecture. Use base-system crypto commands and matching kernel facilities for native formats. Include private third-party userspace backends when feasible; do not require a separate VeraCrypt application on a platform where the bundled build qualifies.

FreeBSD/OpenBSD VeraCrypt bundling is a feasibility and release gate, not an assertion that the current Linux helper patch ports unchanged. If it fails qualification, report that backend as unavailable and record the concrete blocker; do not silently substitute an installed application or advertise completed support.

PEFS requires a compatible FreeBSD kernel module. A package dependency and administrator installation are acceptable; an arbitrary portable executable is not a substitute for that module. Qualify the BSD CLI/core first. Publish desktop artifacts only after Slint, winit, rendering and privilege handling work natively on that OS.

### Building from source

Provide one documented build/stage entry point per platform, backed by shared build metadata. It invokes Cargo and the required native/backend builds, then produces the same layout as the release artifact. Plain `cargo build` remains useful for Rust development but must not be described as building the complete distribution.

Build dependencies may include C/C++, Go, a JDK, platform SDKs, wxWidgets base and FUSE development headers. End users should not need those toolchains. Use the user's managed Python/Rust/Node infrastructure for applicable project tooling. Build scripts must not install system packages, elevate privileges or modify the host silently.

## 5. Backend implementation choices

| Backend | Delivery and integration approach | Host requirements / qualification |
| --- | --- | --- |
| Apple images | Wrap OS `hdiutil` and `diskutil`; no bundled replacement | macOS; existing file-only and busy-unmount rules |
| Linux LUKS2/LUKS1 | Keep cryptsetup orchestration; declare package dependencies, consider private portable tooling separately | Host kernel/device-mapper, filesystem tools and explicit privilege boundary; prefer LUKS2 for new Linux storage |
| VeraCrypt | Build a pinned console/backend executable into Lurker; retain separate Linux creator and unprivileged contracts | Linux kernel route or existing FUSE/LKL route; macOS external FUSE; BSD native qualification |
| gocryptfs | Bundle a pinned native executable; use its documented CLI interface | Linux FUSE; qualify macOS beta and FreeBSD basic support, permissions and application semantics independently |
| CryFS | Bundle stable 1.x executable and dependency closure; prefer supported static dependency builds where practical | Linux/macOS FUSE; do not adopt the Rust 2.0 alpha through this plan |
| Cryptomator | Bundle a small private Java helper using CryptoFS and a reduced private Java runtime | Linux/macOS mount provider; complete create/credential/lifecycle API required, not just CLI unlock |
| GELI | Wrap FreeBSD base tools and file-backed device attachment | Native kernel support, authorization and ordered teardown |
| CGD | Wrap NetBSD base tools and file-backed device attachment | Native kernel support, authorization and ordered teardown |
| softraid CRYPTO / RAID1C | Wrap OpenBSD `bioctl` and native file-backed device facilities | Native kernel support, exact layout/credential qualification and root-authorized mounts |
| PEFS | Wrap utility with a matching kernel-module package | Kernel/OS version compatibility; qualify directory and optional login integration |
| DragonFly LUKS1 | Wrap the OS's native cryptsetup implementation after qualification | Verify actual version, local patches, profiles, file attachment and filesystem behavior; no copied Linux binary |

The implementation must not assume every backend supports password changes, keyfiles, keyslots, tags, every cipher family, read-only mode or the same filesystems. Advertise only the operations demonstrated for that backend/OS combination. Never pass Linux LUKS2/Argon2id options to LUKS1 or map LUKS/VeraCrypt profiles onto folder formats.

Keep upstream formats interoperable. Do not invent Lurker-specific cryptography, modify format primitives, or make another implementation unable to open the storage merely to simplify integration. Retain normal-volume VeraCrypt AES/Serpent/Twofish, SHA-512, PIM 0 and password-or-keyfile policy.

## 6. Shared architecture

### Reuse boundaries for platform implementers

The architecture is **one shared core, reusable backend integrations, and small OS-specific adapters**. Five KVM OS implementation targets do not mean five encryption implementations. These are implementation rules for future agents, not a description of a completed refactor.

| Layer | Shared responsibility | Platform-specific responsibility |
| --- | --- | --- |
| Product and core | CLI/desktop command model, validation, credential policy, capability contracts, lifecycle state machine, error categories and recovery rules | Presentation of capabilities actually available on that host |
| Encryption backend | Upstream format/crypto implementation, backend-specific argument/protocol construction, credential transport and profile validation where semantics match | Necessary upstream build options and narrowly scoped integration patches |
| OS adapter | Common attach/open/mount/close contract and resource-ownership rules | Device attachment, filesystem mounting, authorization, mount/device discovery, process identity and native session hooks |
| Build and packaging | Source pins, checksums, patch provenance, backend build recipes and dependency inventory | Target compilation, package dependencies/layout, ABI/library resolution, signing and kernel/FUSE prerequisites |

For example, VeraCrypt's upstream encryption engine is reusable across selected Linux, macOS, FreeBSD and OpenBSD integrations. Build it for each target and adapt attachment/mounting; do not reimplement AES or duplicate an entire Lurker workflow per OS. Linux's kernel cryptsetup route, Linux's unprivileged FUSE/LKL route and macOS's FUSE/disk-image route remain distinct execution adapters beneath that shared backend policy. Reusable source does not mean the same executable runs on every OS.

Native facilities remain OS-specific: GELI on FreeBSD, CGD on NetBSD and softraid crypto on OpenBSD need separate adapters. They should still use the common command, credential, capability, session and error contracts. Similar lifecycle steps do not make their formats, KDFs or credential operations interchangeable.

All six Linux distributions use the **same Linux integration code**. Handle dependency names, package layouts and library/tool versions in build/packaging metadata and capability probes. Do not fork crypto behavior into Arch/Ubuntu/openSUSE/Fedora/Debian/Gentoo implementations. A demonstrated tool incompatibility may need a narrow, tested version/capability adaptation; distribution name alone is not a reason for a separate workflow.

When adding another platform:

1. Read the existing shared and backend-specific implementations first. Extract genuinely common behavior with regression coverage rather than copying a platform workflow into a new file wholesale.
2. Keep domain types and backend policy in `lurker-core`; put native operations behind explicit adapter boundaries. Keep OS-specific imports and conditional compilation at those boundaries instead of scattering platform branches through frontend and credential code.
3. Share each backend's profile/command builders where its native interface has the same semantics. Keep OS-specific flags and filesystem decisions explicit; never hide meaningful differences behind an abstraction that silently ignores unsupported options.
4. Reuse the backend source pin and common patches across targets where compatible. Keep unavoidable target patches small and documented; avoid maintaining independent platform forks without a demonstrated need.
5. Run shared contract/conformance tests against each adapter, plus native lifecycle tests and packaging checks. A successful adapter on one OS is not proof that another OS's mounts, privileges or recovery behave identically.
6. Expose unsupported operations through capabilities. Do not manufacture feature parity by weakening safety rules, adding an unapproved backend/OS pair, or changing an existing format/profile.

Container interoperability also depends on the inner filesystem: sharing VeraCrypt support does not make a Linux-created Btrfs container mountable on macOS. Report filesystem support separately from encryption-format support. Directory backends likewise require their own cross-platform filename, permission and locking qualification.

### Backend discovery and capabilities

Introduce a backend registry in `lurker-core`, with per-platform adapters. Each entry describes supported source types, operations, credentials, filesystem choices, privilege route, runtime prerequisites and implementation version. Keep supported profile choices hardcoded; build manifests are provenance/dependency data, not user-editable crypto policy.

Resolve tools per requested operation. Refactor the current global `ToolPaths` assumptions so a missing cryptsetup does not prevent an unrelated directory backend or Apple image from working. Distinguish: unsupported on this OS; selected but not implemented; bundled component missing/incompatible; external FUSE/kernel prerequisite missing; operation needs authorization.

Extend helper capability negotiation before introducing new requests. Old helpers must reject unsupported fields/operations rather than ignore them or fall back to a more privileged route. Keep CLI and desktop capability results consistent. The UI should expose usable operations and concise remedies, without presenting internal build details as ordinary user choices.

### Private component layout

Use a platform-specific private directory, for example `libexec/lurker/backends/` on Unix packages and `Lurker.app/Contents/Helpers/` on macOS, with app-private libraries/frameworks and runtime assets alongside it. Preserve a compatibility resolver for the existing Linux userspace helper layout while migrating.

Invoke exact resolved paths, never construct a shell command. Prefer packaged backends over PATH and separately installed applications. Development overrides must be explicit and unprivileged. Elevated operations may execute only components from an administrator-controlled installation with safe path ownership; never elevate a user-writable helper, runtime, library search path or environment override.

Version and verify the native helper contract. Record source revision, source archive digest, patch digests, build options, target, linked dependencies and licenses for each shipped backend. Verify downloaded bytes as well as Git revisions and patch cache state. Keep caches in ignored build/cache directories, outside tracked source paths, and reject stale/inconsistent artifacts.

### Credentials and privilege boundaries

Keep passphrases on private stdin/pipes or an equally narrow supported native API. Do not expose them in argv, environment variables, logs, session JSON or temporary files. Preserve backend-specific newline/NUL validation and binary keyfile handling. Bound and redact subprocess diagnostics.

Preserve the Linux unprivileged backend's refusal to run as root. Do not remove that guard to reuse it for privileged creation. Build a separate narrow creator contract where elevated device access is actually necessary. Run regular-file work without elevation where possible, and grant privileged helpers only the operations they need.

Keep existing Linux pkexec/TTY-sudo behavior while developing platform-specific BSD authorization. macOS file-container operations remain in the login user's session; bundling VeraCrypt does not justify elevating the entire desktop. Do not add passwordless sudo rules or broad device permissions.

### Session lifecycle and recovery

Generalize the existing session model without weakening its checks: source identity, owner uid, mount identity, backing device identity, backend version, process identity/start time and current lifecycle stage. Persist only non-secret metadata atomically in a private per-user directory. Lock against duplicate opens and cross-mode attachment.

For layered storage, drain/unmount the application filesystem before closing the decrypted image or detaching the backing device. Apply read-only at every applicable layer. Preserve busy or partially failed sessions for a safe retry; no routine forced/lazy unmount and no blanket process killing. Track resources acquired by this operation so cleanup cannot close an unrelated pre-existing mount.

Retain existing same-directory staged file creation, private permissions and atomic publication. Directory backends need equivalent failure handling for their configuration/data directories; do not overwrite a partially initialized or existing vault blindly. Never publish a create as successful before its storage is durably initialized and closed.

## 7. Implementation sequence and exit criteria

### Phase 0 — capability and build foundation

1. Inventory actual platform code gates and current external calls; capture the existing Linux/macOS operation matrix as regression coverage.
2. Introduce backend-specific discovery/capabilities and versioned helper requests incrementally, retaining existing defaults.
3. Define a common backend build manifest and private component resolver. Migrate the existing pinned Linux helper build without changing crypto behavior.
4. Add complete release staging and dependency inspection; preserve separate developer Cargo builds.
5. Update the agent/codebase directives to distinguish system facilities from bundled third-party backends before replacing discovery behavior.
6. Implement the scripted remote-job workflow in section 8 before expanding manual SSH testing across the VM matrix. Qualify job deployment, result collection and reconnection with a small existing check first.

Exit: current workflows still pass; missing unrelated tools do not block independent backends; stale helpers cannot execute new operations; release artifacts contain identified, reproducibly sourced private components.

### Phase 1 — Linux without installed VeraCrypt

1. Reuse the upstream source/build machinery already maintained by `scripts/build-linux-userspace.py`; keep the existing unprivileged executable and protocol intact.
2. Add the separate private creator build/entry point. Restrict its command surface and preserve current format/profile arguments, full file initialization, keyfile semantics and block-device force confirmation.
3. Run accessible regular-file header creation as the user where possible, then use the existing privileged mapping/formatting route. For device creation, validate target identity and invoke only the trusted privileged creator operations required.
4. Change privileged creation discovery to the packaged creator. Leave privileged open/close on cryptsetup. Remove the external VeraCrypt runtime requirement from package metadata and documentation only after end-to-end qualification.
5. Test complete persistence across privileged/unprivileged mounts in both directions, not just header password acceptance.

Exit: on a clean Linux VM with required OS tools and FUSE but no VeraCrypt package/PATH executable, both supported file workflows create, write, close and reopen correctly. Device create passes only on dedicated disposable test devices. Unprivileged requests still cannot elevate or accept raw devices.

### Phase 2 — macOS without installed VeraCrypt

1. Build a pinned native console/backend from upstream sources, with the minimum necessary integration patch. Building requires toolchains/libraries, not a preinstalled VeraCrypt.app.
2. Select and test the supported external FUSE ABI/runtime. Stock macOS VeraCrypt code contains runtime detection and library-path assumptions; adapt those deliberately. Do not assume changing a build flag makes FSKit compatible.
3. Bundle non-system dependencies recursively. Rewrite install names/rpaths to private locations and reject references to a developer's Homebrew prefix or checkout. Keep the separately installed FUSE boundary explicit rather than copying its runtime into the app.
4. Replace installed-app discovery with trusted bundled discovery for create, mount, list and unmount. Decide how to discover externally mounted volumes safely; never detach unrelated images to preserve a broad listing feature.
5. Extend `scripts/build-macos-dmg.py` for backend/runtime staging, nested signing, license/provenance payloads, architecture/deployment checks and release notarization.
6. Verify the qualified FUSE-T transport for each packaged backend. Report missing or unsupported prerequisites with a useful explanation.

Exit: a clean Mac with the qualified FUSE runtime, no VeraCrypt.app and no developer package-manager dependencies can create, mount, use and close a VeraCrypt file from the packaged app. Apple images work without FUSE. Busy-unmount protection remains intact. Native release signing and dependency checks pass for each published architecture.

### Phase 3 — bundled directory backends

1. Integrate gocryptfs first through its documented CLI ABI. Use the supported Go-only build on Linux where appropriate; qualify native macOS/FreeBSD builds separately instead of assuming Linux static-build settings apply everywhere.
2. Integrate CryFS stable 1.x with a pinned C++/CMake/Conan build and private dependency closure. Test filesystem semantics and known crash/full-storage behavior before recommending it for live application data.
3. Implement a shared folder-vault lifecycle: create, unlock, active discovery, read-only where supported, busy-safe close and recovery after frontend exit.
4. Expose format-specific capabilities and failures through the existing CLI/helper/desktop flow. Do not pass generic `--cipher` options to a format that does not implement them.

Exit: each released OS/backend pair works without separately installed gocryptfs/CryFS, using only its documented host FUSE prerequisites. Application-file semantics, mount recovery and clean shutdown pass. macOS beta/FreeBSD basic status remains visible in release qualification until evidence supports stronger claims.

### Phase 4 — Cryptomator without a separate Java installation

1. Prototype the upstream CLI packaged image to validate the mount provider and private-runtime build. Its unlock/mount surface alone is insufficient for the full Lurker lifecycle.
2. Implement a narrow private Java helper using supported CryptoFS APIs for initialization, unlock, mount, close and supported credential changes. Use a structured, versioned protocol with private secret transport.
3. Package a reduced Java runtime using the upstream jlink/jpackage approach. No system Java, Cryptomator GUI installation, or runtime dependency downloads.
4. Qualify FUSE mounting and live application behavior. Do not silently substitute a WebDAV mount with different locking/performance semantics.
5. Review AGPL distribution/source obligations and compatibility before shipping the helper; do not assume a process boundary alone settles licensing obligations.

Exit: a clean Linux/macOS machine without Java or Cryptomator can use the complete supported vault lifecycle with the Lurker package and required host FUSE. Packaged-image integration tests, credential handling and runtime dependency audits pass.

### Phase 5 — BSD adapters and bundled components

1. Separate Linux-only APIs/imports and platform dispatch sufficiently for native core/CLI compilation. Add BSD-specific mount/device/session discovery rather than parsing Linux `/proc` structures everywhere.
2. Implement GELI on FreeBSD, CGD on NetBSD and softraid crypto on OpenBSD using base tools and file-backed test devices. Qualify layout, authentication, attach/open/mount and reverse-order close/detach; define each format's hardcoded create profile before exposing create.
3. Qualify DragonFly's installed cryptsetup, local patches and LUKS1 support. Check effective algorithms/KDFs, file-backed devices, mount behavior and maintenance/security evidence. An old vendor baseline alone proves neither safety nor unsafety; unresolved qualification blocks release.
4. Build the private VeraCrypt console/backend on FreeBSD and OpenBSD. Start with approved upstream targets (FreeBSD x86-64 14+, OpenBSD x86-64 7.8+, subject to confirmation for the pinned revision). Test actual inner-filesystem support and authorization; do not transplant Linux LKL or macOS attachment logic blindly.
5. Bundle gocryptfs for FreeBSD after native FUSE/permissions testing. Package PEFS utility/module integration against the matching kernel, with clear missing/mismatched-module errors.
6. Qualify the Slint desktop independently per BSD. If core/CLI are ready first, label those artifacts accurately; do not advertise desktop support based only on Rust compilation.

Exit: each advertised pair has native operation tests and a complete package. FreeBSD/OpenBSD VeraCrypt needs no separately installed application when published as bundled support. Infeasible pairs remain unavailable with a documented blocker; the selected matrix is not a claim that all pairs ship simultaneously.

### Phase 6 — lifecycle integration and release hardening

1. Add optional session/login automation only after manual lifecycle handling is reliable. Start application/profile consumers after unlock and stop them before unmount.
2. Treat an encrypted whole home as a separate login/PAM integration problem. A desktop launched from inside that home cannot reliably unlock its own prerequisites; do not claim generic full-home support from a successful folder mount.
3. Add shutdown/session hooks with bounded waits and busy-state reporting. Avoid disconnecting live storage merely to report a completed lock operation.
4. Complete dependency/license inventories, source publication, update/rebuild policy and artifact signing. Measure package/runtime size and I/O behavior rather than promising negligible overhead.

Exit: published support matches tested behavior, all bundled code has provenance and an update owner, and shutdown integration preserves data rather than forcing success.

## 8. Validation and release gates

### KVM development and validation topology

Use **five primary OS development VMs and ten total KVM build/functional-test VMs**. The five development VMs are a subset of the ten, not five additional machines. These are guest environments, not a requirement for ten physical hosts.

| OS implementation target | Primary development | Native build, packaging and functional tests |
| --- | --- | --- |
| Linux | One of the six Linux VMs | Arch Linux, Ubuntu, openSUSE, Fedora, Debian, Gentoo |
| FreeBSD | FreeBSD VM | FreeBSD VM |
| DragonFly BSD | DragonFly BSD VM | DragonFly BSD VM |
| NetBSD | NetBSD VM | NetBSD VM |
| OpenBSD | OpenBSD VM | OpenBSD VM |
| Total | 5 VMs | 10 VMs |

Develop shared features and the Linux adapter primarily on one Linux distribution, and native adapters on their respective BSDs. Then build and run functional tests on all ten guests for each applicable selected backend. Testing the other Linux distributions means exercising the installed artifacts and operations, not compilation alone. Maintain the same source revision and record each guest's toolchain, dependency and kernel/FUSE versions so failures can be attributed correctly.

The ten guests cover Linux and the four BSDs. macOS remains a separate native Mac build/test target under the approved matrix. BSD CLI/core and desktop qualification remain separate; no guest may be marked fully supported merely because its CLI compiles.

### Scripted remote build and test workflow

Future agents must use repeatable remote jobs with saved results for matrix testing. Avoid reconstructing a test session through many interactive SSH commands or streaming complete compiler/test logs into agent context. Scripts and artifacts are the durable record; the agent normally reads the summary and only the evidence needed to diagnose failures. This section specifies tooling to implement, not tooling already available.

#### Source deployment and script responsibilities

- Use one feature/integration branch and deploy the **same exact Git commit** to every host in a matrix run. Keep OS-specific scripts in that shared revision; do not maintain separate permanent OS branches. Platform fixes return to the shared branch. A changed revision starts a new run; never combine old passing results with a new revision to claim a complete matrix pass.
- Deploy into isolated remote worktrees/checkouts identified by run ID and commit. Preserve existing remote development trees and uncommitted changes. Fetch the commit or transfer a verified Git bundle/source payload when the guest cannot fetch it; do not copy private SSH credentials into guests for deployment.
- Add one local coordinator, a common remote runner and small platform adapters. Suggested locations are `scripts/remote-test.py` and `scripts/remote-test/`; final names may follow repository conventions. Share stage execution, timeouts, log capture and result serialization. Adapters provide native build/test commands and OS integration, while Linux distribution metadata supplies package/dependency differences.
- Keep a host inventory containing SSH alias, OS/distribution, architecture and expected capabilities. Resolve actual host aliases from the environment when implementing it; do not invent hostnames. Keep credentials and mutable run state out of tracked inventory.
- Separate explicit bootstrap scripts from ordinary test runs. Bootstrap prepares prerequisites according to project toolchain policies. Test jobs check prerequisite versions and report missing components; they do not repeatedly refresh repositories, install packages, alter sudo policy or change the host configuration.
- Resolve required toolchains explicitly for noninteractive SSH execution. Record the selected executables/versions; do not depend on an interactive shell's incidental PATH or switch to another language installation silently.

#### Test selection and execution

Provide named stages: prerequisite check, source verification, backend build, Rust checks, packaging, installed-artifact tests and cleanup. Each stage has a timeout, exit status and log file. Respect dependencies: a failed build blocks tests requiring its artifact, but does not erase completed evidence.

Use two testing levels:

1. Development checks run the affected backend/adapter tests and relevant regressions on the primary OS VM. Shared core, protocol or build-system changes broaden checks to all affected primary OS targets.
2. Milestone/release checks build, package and exercise applicable selected backends across all ten guests at one revision. A successful compile alone never substitutes for installed-artifact functional testing. Keep macOS on its separate native runner using the same reporting contract where practical.

Derive backend applicability from the approved matrix and implemented capabilities. Record unsupported combinations as skipped with a reason. A missing prerequisite, unimplemented required operation or unavailable host is blocked/incomplete, not a successful skip. Required tests must actually execute before their target can pass.

Run remote jobs independently of the SSH connection and track them by job ID. Launch once, query status and collect results; reconnecting must not launch a duplicate build. Use a native supervisor or a verified detached runner per OS. Persist stage state and publish final results atomically. A disconnected or crashed runner remains incomplete/unknown until reconciled, never implicitly successful.

Bound concurrency by the KVM host's available CPU, RAM and storage, with per-guest build/test-space checks and serialization of conflicting storage tests. Reuse compiler/dependency caches keyed by target, toolchain, backend revision/patches and relevant build flags. Keep job artifacts separate from reusable caches. Do not rebuild unchanged native dependencies or download them repeatedly without cause.

Timeout/cancellation cleanup targets only the job's verified processes and disposable resources. An ordinary build run must not kill a VM, close unrelated mounts, force unmount or repair filesystems. Abrupt-shutdown/fault-injection suites are explicitly selected runs on disposable test environments, separate from the normal fast suite.

#### Concise results and retained evidence

Each job writes a versioned `result.json`, stage logs and artifact references under its run directory. The coordinator collects the small result files and produces one aggregate JSON report plus a short table. Keep full logs and artifacts in ignored run directories; do not commit generated reports or dump them into chat.

The result contract must include:

- Run/job ID, exact source commit, host alias, OS/release/kernel, architecture and runner schema version.
- Toolchain, backend and FUSE versions; selected suite, privilege route and relevant inner filesystem.
- Stage status, duration, exit code, test pass/fail/skip counts and explicit skip/block reasons.
- Failing stage, a bounded diagnostic excerpt, and locations/digests for logs and built/tested artifacts.

Remote runner stdout should contain a small machine-readable status/result reference. Send verbose tool output to files. The coordinator's default output is one row per host with commit, build/test status, counts, duration and failure/artifact reference. Successful jobs need no log excerpt. For failures, return only the first actionable diagnostic with a configurable cap, initially 40 lines / 8 KiB per failed job; fetch a specific additional stage log only when needed. Disable shell tracing around credentials and never persist secrets in logs or results.

Provide coordinator operations to start, inspect, resume collection and cancel a run by ID, plus retrieve a named stage's diagnostic excerpt. Use bounded/backoff status queries with no repeated unchanged output. Reruns have new job IDs and preserve previous failures; cached results are reusable only for an identical verified run specification and are labeled as reused. The aggregate command exits nonzero for required failures, timeouts, blocked or incomplete targets.

Retain the run manifest, result summary and artifact locations so a later agent can resume from the run ID instead of repeating SSH discovery or rereading whole logs. Apply explicit retention/cleanup to owned old run artifacts, without deleting active worktrees, valuable failure evidence or unrelated files. A release still requires the clean-machine and absent-external-backend checks below; a successful build on a prepared development VM alone does not prove runtime dependency independence.

### Required checks

Run Rust checks for implementation changes: `cargo check --workspace`, `cargo test --workspace`, and desktop tests when desktop behavior changes. Preserve and extend the real Linux userspace suite (`scripts/test-linux-userspace.sh`, normal user) and macOS disposable native tests. Unit tests alone cannot qualify a filesystem backend or packaged runtime.

For every backend/OS pair that will be advertised:

- Test the **packaged artifact** in a clean VM or native test machine. Remove/uninstall separate VeraCrypt for its tests; exclude other separately installed bundled backends and language runtimes. Use execution/dependency inspection as well as PATH manipulation.
- Exercise create, unlock, write, fsync, close, reopen, incorrect credentials, supported keyfiles, Unicode/spaces, read-only, duplicate/aliased sources, existing destination protection and frontend restart.
- Exercise open files, busy directories, mount replacement, stale session records, PID reuse protection, cancellation, lost frontend/helper and backend exit. Cleanup must touch only resources owned by the identified session.
- Test SQLite WAL/locking, atomic rename, truncation, permissions and relevant application profiles. Include browser-profile workloads; a simple text-file test is insufficient. Verify unsupported concurrent writers are rejected or explicitly documented.
- Test full storage, interrupted create, SIGKILL of individual backend layers and abrupt VM shutdown using dedicated disposable storage. Verify readable data that was durably committed, characterize incomplete writes and retain recovery metadata. Do not promise immunity to corruption or run filesystem repairs on user data automatically.
- Check private and system dependency resolution with appropriate ELF/Mach-O tools. Reject accidental developer paths, missing dylibs/shared objects and runtime downloads. Test offline runtime operation after installation.
- Verify no-secret diagnostics, helper protocol mismatch handling, trust/ownership checks, unprivileged no-elevation behavior and minimal privileged command scope.
- Record OS/kernel, architecture, FUSE implementation/version, backend source/build, inner filesystem, privilege route, operations exercised and remaining limitations. Do not infer another platform's support from a successful Linux build.

Additional macOS gates: verify the final signed app and DMG, nested library signatures, minimum deployment target, architecture slices, Gatekeeper/notarization for public releases, and failure with missing/incompatible FUSE. FSKit's flush/fsync/close behavior must be tested against the selected runtime and OS version, especially for SQLite/browser profiles; the existence of an FSKit backend is not evidence of matching durability semantics.

Use only dedicated disposable devices or test images for destructive tests, including qualification of physical-device setup. Preserve unrelated host and guest storage, including its mounts and filesystem state.

## 9. Security, licensing and maintenance gates

- Bundling transfers dependency-update responsibility to Lurker. Maintain pinned revisions, artifact hashes, patch review, a dependency inventory and an upstream security-update/rebuild process. Pins establish provenance, not proof of security.
- Preserve the distinction between an ordinary user process and a sandbox. Evaluate process confinement, dump/debugger access, memory locking and swap/hibernation exposure separately; do not claim the existing LKL route implements those protections.
- Review each component's actual license and notices, including VeraCrypt's combined licensing/branding terms, Java modules, CryFS dependencies and Cryptomator/CryptoFS. Publish required corresponding source/build changes. Keep attribution separate from the product's own GPL license text.
- External macOS FUSE is intentional. Do not redistribute or automatically download a FUSE runtime under assumptions that its user-install license also permits commercial embedding. Verify ABI and redistribution boundaries for linked components.
- CryFS crash/full-storage limitations, gocryptfs macOS/FreeBSD maturity, PEFS kernel coupling and DragonFly's native crypto implementation are explicit qualification work, not solved by successful compilation.
- Preserve normal-volume interoperability and credential policy. Never present encryption as a guarantee of authenticated data integrity, safe concurrent access, power-loss recovery or protection from malware using an unlocked account.

## 10. Documentation and completion checklist

Update implementation and documentation together in small coherent commits. Keep `DIRECTIVES/LLM/ENCRYPTION.md` authoritative for selected scope/profiles, `DIRECTIVES/LLM/CODEBASE.md` accurate for shipped architecture, and `README.md` accurate for runtime/build/install requirements. Where profile behavior changes, update `workflow.rs`, relevant platform adapters, archival CLI profile references where applicable, and user-facing documentation coherently.

The first required milestone is complete only when Linux and macOS releases run their supported VeraCrypt operations without any separately installed VeraCrypt, while macOS FUSE remains an explicit separately installed dependency. Finishing unrelated future backends is not a prerequisite for delivering that milestone.

The full plan is complete only when every advertised matrix entry has packaged and native-tested support, external prerequisites are explicit, bundled dependencies are maintainable, and unsupported operations fail safely. Selected but unfinished entries must continue to be labeled planned.

## 11. Primary references and implementation evidence

Research basis: upstream material reviewed for the cross-platform packaging discussion. Recheck the pinned release's requirements during implementation; moving upstream branches are references, not immutable build inputs.

- Existing Lurker implementation and limitations: `docs/linux-userspace.md`, `scripts/build-linux-userspace.py`, `scripts/build-macos-dmg.py`.
- VeraCrypt build requirements: https://github.com/veracrypt/VeraCrypt/blob/master/src/Readme.txt
- VeraCrypt supported targets: https://veracrypt.io/en/Supported%20Operating%20Systems.html
- VeraCrypt macOS attachment/runtime detection: https://github.com/veracrypt/VeraCrypt/blob/master/src/Core/Unix/MacOSX/CoreMacOSX.cpp
- VeraCrypt licensing: https://github.com/veracrypt/VeraCrypt/blob/master/License.txt
- gocryptfs platform status and CLI ABI: https://github.com/rfjakob/gocryptfs and https://github.com/rfjakob/gocryptfs/blob/master/Documentation/CLI_ABI.md
- gocryptfs Go-only build: https://github.com/rfjakob/gocryptfs/blob/master/build-without-openssl.bash
- CryFS stable branch/build instructions: https://github.com/cryfs/cryfs/tree/release/1.0
- Cryptomator CLI and packaged runtime builds: https://github.com/cryptomator/cli and https://github.com/cryptomator/cli/blob/develop/build_mac.sh and https://github.com/cryptomator/cli/blob/develop/build_linux.sh
- CryptoFS initialization API: https://github.com/cryptomator/cryptofs/blob/develop/src/main/java/org/cryptomator/cryptofs/CryptoFileSystemProvider.java
- Linux FUSE architecture: https://docs.kernel.org/filesystems/fuse/fuse.html
- macFUSE runtime/FSKit behavior: https://macfuse.github.io/ and https://macfuse.github.io/2026/10/17/macfuse-5.5.0.html
- macFUSE binary redistribution terms: https://github.com/macfuse/macfuse/blob/release/macfuse/LICENSE.txt
- FUSE-T implementation and licensing: https://github.com/macos-fuse-t/fuse-t and https://www.fuse-t.org/#licensing
- Apple FSKit architecture, if later needed: https://developer.apple.com/documentation/fskit/building-a-passthrough-file-system
- FreeBSD GELI: https://man.freebsd.org/cgi/man.cgi?query=geli&sektion=8&format=html
- NetBSD CGD: https://man.netbsd.org/cgdconfig.8
- OpenBSD crypto/device management: https://man.openbsd.org/bioctl.8
- FreeBSD PEFS kernel-module packaging: https://github.com/freebsd/freebsd-ports/blob/main/sysutils/pefs-kmod/Makefile
- DragonFly native cryptsetup baseline: https://github.com/DragonFlyBSD/DragonFlyBSD/blob/master/contrib/cryptsetup/README.DRAGONFLY
