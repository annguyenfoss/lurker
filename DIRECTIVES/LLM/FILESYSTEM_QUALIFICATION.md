# Filesystem support qualification benchmark

Use this functional acceptance benchmark in every filesystem testing round.
The end goal is that a user can install Lurker and its documented host
prerequisites, create encrypted storage with the selected filesystem, use it
read/write, close it normally and reopen it with the data intact.

## Scope and prerequisites

- Assess each applicable OS, filesystem, encryption backend and source kind.
  Record whether the filesystem is an inner filesystem or directory-vault
  backing storage. One passing backend does not qualify every combination.
- Use eligible OS-native facilities and established external tools/drivers.
  Filesystem drivers remain host prerequisites; independent encryption engines
  are packaged privately according to the product architecture.
- Document required packages, versions, configuration and established patches.
  Reproducible configured kernel builds are allowed under DEVELOPMENT.md's
  kernel integration boundary; Lurker-invented kernel code is excluded.
- Use the approved profiles and variations in ENCRYPTION.md. A round's brief
  defines its host inventory, device samples and resource limits; it does not
  weaken this acceptance benchmark.

## Functional acceptance

1. **Create:** Encrypt the container/device and format its filesystem using the
   eligible native route. For directory encryption, qualify the backing
   filesystem separately and initialize the encrypted vault normally.
2. **Open:** Authenticate and prove that the expected filesystem is genuinely
   mounted read/write, with the correct source and native mount identity.
3. **Use:** Exercise directory and file creation, writes, reads, rename and
   deletion. Use a pinned payload and compare its hashes; verify actual caller
   identity where caller permissions are part of the route.
4. **Persist:** Check file/directory synchronization, close normally, reopen and
   verify original hashes. Append data, create another file, close/reopen again
   and verify the updated hashes. Respect the backend's checked drain/exit
   contract before detaching outer encryption.
5. **Permissions and features:** Check caller access, ownership and unrelated-user
   denial where supported. Briefly check journaling, CoW and ACL capabilities
   where relevant; record supported, unsupported, unknown or not applicable
   with the observation. This is not exhaustive filesystem certification.
6. **Lifecycle:** Exercise ordinary VM reboot persistence and relevant busy-close,
   wrong-credential, duplicate/aliased-source, overwrite and failure/recovery
   cases. Scope SQLite WAL, full-storage, injected I/O errors and crash tests to
   the affected behavior. Do not equate an orderly reboot with abrupt failure.

Test containers before device routes. Use the round's verified disposable VM
disks and authorized physical samples. Follow DEVELOPMENT.md's persistence and
payload procedures. Buran must not reboot; use additional close/reopen checks
and the authorized limited USB sample instead, recording reboot as unexercised.

Run the actual CLI and desktop/helper paths required by the round. Keep native
tool controls, development binaries, staged artifacts and installed-package
qualification distinct. If a failure may be upstream, compare the unmodified
pinned upstream workflow under equivalent conditions before assigning cause.

## Evidence and availability labels

A qualification result must identify the source/artifact, host, prerequisite,
filesystem/backend/source-kind roles, expected/observed outcome, commands/raw
logs, payload hashes and cleanup. Retain failed and partial attempts concisely;
use DEVELOPMENT.md's remote execution and evidence rules.

| Availability | Meaning |
| --- | --- |
| Qualified | The actual Lurker workflow passed within its recorded scope. |
| Conditional | The workflow passed with the stated packages, drivers, configuration or established patches. |
| Incomplete | An eligible route exists, but implementation or qualification remains. |
| Unavailable | No eligible complete native Create + Read/Write route was identified in the recorded assessment. |
| Not applicable | The backend/source-kind combination does not apply; state why. |

An available formatter, compiled driver, successful mount, readable image or
imported filesystem seed alone does not qualify the full workflow. Read-only
access and inability to create/format remain capability limitations, not product
modes or full passes. Synthetic controls and primitive tests do not establish
real mounted-storage support. Manual repair, force, custom reconciliation and
test-only cleanup must not become ordinary-close or automatic-recovery passes.

Availability and remaining-work rank are separate. Rank only retained in-scope
implementation/qualification work; count deferred work and exclude cut work.
When no actionable in-scope route remains, use **N/A — native capability
unavailable**, not a low progress score or an indefinite deferred kernel task.
Keep the capability limitation visible and reopen it only when a suitable
established solution or new concrete evidence warrants reassessment. Do not
claim that a bounded unsuccessful search proves support is impossible forever.
