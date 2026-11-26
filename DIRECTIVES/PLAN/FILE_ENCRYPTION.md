# File encryption CLI implementation plan

Implement the credential and scope decisions in `FILE_ENCRYPTION_NOTES.md` for the `lurker` CLI. The native build matrix is Buran, six Linux distributions, and four BSD guests. The existing Windows CLI and native build host are roadmap work. Keep the file implementation portable within the existing CLI targets.

## 1. Commands and behavior

- Add `lurker encrypt --type age|gnupg|openssl [CREDENTIALS] INPUT OUTPUT` and `lurker decrypt --type age|gnupg|openssl [CREDENTIALS] INPUT OUTPUT`. Require a backend and explicit input and output paths for the initial surface. The OpenSSL type means CMS.
- Use `--passphrase`, `--key-file FILE`, repeatable `--public-key RECIPIENT`, and `--private-key FILE` according to the working notes. Use `--certificate FILE` for a CMS decryption certificate when needed. Prompt for an entered passphrase and confirm it on encryption. GnuPG resolves its own private key. Expose a read-only GnuPG key listing through `lurker keys --type gnupg` with usable public encryption recipients and full fingerprints.
- Validate the selected backend and credential combination before creating output. Accept regular file inputs, including existing archives as opaque bytes. Publish successful output with a private, same-directory staging file and atomic no-clobber behavior. Preserve existing destinations unless `--force` is supplied, reject input/output aliases, and discard failed staged output. A backend failure must leave the original input intact.
- Put file operations in `lurker-core` and route them from the CLI separately from the volume command and helper protocol. Run all file operations as the invoking user. Render CLI help for supported credential choices and missing dependencies. Keep the GUI, helper protocol, archive handling, filesystems, and power-loss testing outside this round.

## 2. Backend implementation

| Backend | Implementation | Supported credentials |
| --- | --- | --- |
| age | Use the pinned Rust `age` crate in core with its streaming APIs. Retain standard `.age` interoperability with both upstream CLIs. Enable supported existing SSH key files alongside native age keys. | One entered passphrase or one or more public recipients. Accept a matching private identity file on decryption and prompt when it is protected. |
| GnuPG | Invoke installed `gpg` with structured status and parse `--with-colons` output for listing. Use full key fingerprints and direct `--recipient-file` for existing public files. Disable automatic key retrieval and implicit encrypt-to recipients. Keep ordinary GnuPG keyring and agent integration. Suppress automatic output compression in Lurker-created files. | An entered passphrase or first line of `--key-file` counts as one shared option. Accept multiple public recipients, alone or with that shared option. Pass secrets through an inherited pipe or pinentry. |
| OpenSSL CMS | Use AES-256-GCM `AuthEnvelopedData`. Use installed OpenSSL CLI for certificate-only operations. Use a narrow adapter to installed OpenSSL `libcrypto` for operations accepting a shared AES key or recipient passphrase. Resolve a supported OpenSSL implementation explicitly on LibreSSL hosts. | One optional passphrase, one optional 32-byte AES keyfile, and any number of X.509 public certificates. Allow any nonempty combination when encrypting and one matching credential when decrypting. Accept 32 raw bytes or 64 hex digits with at most one trailing line ending for the AES keyfile. |

For the CMS adapter, use public OpenSSL APIs to add the selected certificate, KEK, and password recipient entries to a partial authenticated AES-GCM envelope, and public CMS APIs to open it. Pass secrets in memory and use a fixed, documented password-recipient PBKDF2 iteration count with CMS-compatible parameter encoding. Keep certificate-only CLI passphrases on stdin. Confirm interoperability in both directions between CLI-created and library-created CMS envelopes. Do not route secret values through argv, environment variables, or log output. Scope installed-tool and library paths to unprivileged file operations. Keep age available without an installed `age` command and keep missing GnuPG or OpenSSL local to the selected backend.

## 3. Ownership and host prerequisites

Read the tracked SSH inventory in `scripts/remote-test/hosts.json`. The coordinator owns shared CLI/core contracts, Cargo dependency reconciliation, integration, and the common PDF harness. The Arch owner implements age, the Debian owner implements GnuPG, and the Ubuntu owner implements CMS in separate backend modules. All 11 agents own their named host's preflight, needed package setup, and validation of all three backends at the frozen integrated revision. Keep one agent per host and report only changed prerequisites, blockers, or a final result row.

These are candidates from the initial SSH probe. Confirm version, package ownership, executable and library resolution, and space before changing packages. Install or update only the missing or incompatible requirements. Use the supplied VM sudo credential through stdin without recording it. Build and run the file tests as `adjutant`. On Buran use `ssh adjutant@buran` and passwordless sudo only if needed for approved package setup. Use user-managed Homebrew for brew packages. Keep Buran running and preserve its installed Lurker and personal keys.

| Owner and host | Initial finding and package action |
| --- | --- |
| Arch `archlinux-core-01` | GnuPG 2.4.9 and OpenSSL 4.1.0 present. Use installed `gnupg` and `openssl` packages. |
| Debian `debian-core-01` | OpenSSL 3.5.9 present. Install `gnupg` with APT if `gpg` remains missing. |
| Ubuntu `ubuntu-core-01` | GnuPG 2.4.9 and OpenSSL 4.0.3 present. Use installed packages. |
| Fedora `fedora-core-01` | GnuPG 2.4.9 and OpenSSL 4.0.3 present. Use installed `gnupg2` and `openssl` packages. |
| openSUSE `opensuse-core-01` | GnuPG 2.5.5 and OpenSSL 3.5.0 present. Check vendor updates with Zypper if the installed build fails the required capability checks. |
| Gentoo `gentoo-core-01` | GnuPG 2.5.24 and OpenSSL 3.5.9 present. Use installed `app-crypt/gnupg` and `dev-libs/openssl`. |
| FreeBSD `freebsd-core-01` | Base OpenSSL 3.5.9 present. Install `gnupg` via `pkg` if missing. Space is low on `/tmp`; use a measured existing build target with room. |
| NetBSD `netbsd-core-01` | Base OpenSSL 3.5.7 and pkgsrc tool paths present. Install `gnupg2` via the configured pkgsrc package repository if missing. |
| DragonFly `dragonflybsd-core-01` | LibreSSL in base and OpenSSL 3.5.9 under `/usr/local`. Select packaged OpenSSL. Check the DPorts GnuPG version and upstream support or documented backports before choosing a package. Obtain a supported build if the available port is stale. |
| OpenBSD `openbsd-core-01` | LibreSSL in base and OpenSSL 3.5.9 as `/usr/local/bin/eopenssl`. Select packaged OpenSSL and its matching library. Install `gnupg` using `pkg_add` if missing. |
| macOS `buran` | LibreSSL in base and Homebrew OpenSSL 3.5.9 under `/opt/homebrew/opt/openssl@3`. Select the brew executable and matching library. Install Homebrew `gnupg` if missing. Use the configured Rust toolchain and Homebrew paths on SSH. |

### Prerequisite changes made

| Host | Package action |
| --- | --- |
| Arch, Ubuntu, Fedora, openSUSE, Gentoo | Existing GnuPG and OpenSSL installations met the preflight requirements. No package changes. |
| Debian | Installed GnuPG 2.4.7, its agent and `pinentry-curses` through APT. Retained OpenSSL 3.5.9. |
| FreeBSD | Installed quarterly `gnupg` 2.5.24 and dependencies via `pkg`. Retained base OpenSSL 3.5.9. |
| NetBSD | Staged official pkgsrc `gnupg2` 2.5.22 and dependencies in a private prefix because the guest denied sudo. Used base OpenSSL 3.5.7. |
| DragonFly | Staged official `gnupg` 2.5.24 and dependencies in a private prefix because the guest has no sudo or doas. Used OpenSSL 3.5.9 from `/usr/local`. |
| OpenBSD | Staged signed `gnupg` 2.5.24p0 and dependencies in a private prefix because the guest denied sudo. Used `eopenssl` 3.5.9. |
| Buran | Installed Homebrew `gnupg` 2.5.24_1 and dependencies. Used Homebrew `openssl@3` 3.5.9. |

## 4. Validation and delivery

1. Finish the shared implementation and review the atomic commits before freezing the test revision. Use private native run directories and the existing remote runner. Build `lurker-cli` and run the scoped core/CLI checks without compiling desktop targets.
2. Use one deterministic valid PDF per host, reusing the existing test fixture generator. Generate disposable recipient keys and credentials in the job directory as fixture setup. Use a private GnuPG home and never read or modify personal keys during tests.
3. Encrypt and restore the PDF through the actual built `lurker` CLI. Test age passphrase and multiple recipients, GnuPG passphrase, passphrase file, public-key files, keyring selection, and combined recipients, and CMS certificates, shared keyfile, passphrase, and combined recipients. In combined cases decrypt independently using each kind of credential. Compare each restored PDF to the original byte-for-byte. Exercise one wrong credential or corrupted input per backend to confirm failed decryption leaves no output. Use the same PDF for every case.
4. Record one compact result row per host with the commit, build and test result, PDF SHA-256, tool versions and paths, package changes, binary hash, and evidence path. Keep full logs in job directories. Rerun only failed or affected cases after fixes and identify the exact product revision used.
5. Reconcile the encryption directive and CLI usage documentation with the implemented file backend scope. Record actual native passes and any unresolved host prerequisites before declaring coverage.
