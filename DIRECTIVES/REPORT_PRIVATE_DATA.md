# Private Data Location Report

Date: 2026-04-25

Purpose: document where common applications store configuration, secrets, keys, and session data on Linux and macOS so Lurker can later detect and migrate them into an encrypted container.

Scope notes:

- Paths below are default locations. Flatpak, Snap, App Store, Homebrew, portable, and vendor-custom builds may differ.
- For browser-family applications, the base directory is not enough. The sensitive material is inside profile subdirectories.
- Some applications split "profile data" from "stored secrets". In particular, Chrome-family browsers, Safari, Docker, and many macOS apps may rely on OS credential stores in addition to files under the home directory.
- Where upstream does not provide a strong first-party storage-path document, the finding is marked as inferred or operational rather than fully vendor-documented.

## Firefox

Linux:

- `~/.mozilla/firefox`

macOS:

- `~/Library/Application Support/Firefox`

Sensitive files and directories:

- `Profiles/<name>/logins.json`
- `Profiles/<name>/key4.db`
- `Profiles/<name>/cookies.sqlite`
- `Profiles/<name>/places.sqlite`
- `Profiles/<name>/storage/`
- `Profiles/<name>/sessionstore*`
- `Profiles/<name>/extensions/`
- `installs.ini`
- `profiles.ini`

Notes:

- `logins.json` and `key4.db` must move together for saved logins.
- Session, cookie, extension, and site storage data are profile-local.

## Floorp

Family: Firefox-derived browser

Likely Linux base path:

- `~/.floorp`

Likely macOS base path:

- `~/Library/Application Support/Floorp`

Sensitive files and directories:

- same profile internals as Firefox, especially `logins.json`, `key4.db`, `cookies.sqlite`, `storage/`, `sessionstore*`

Notes:

- Treat this as Firefox-compatible profile handling.
- Prefer runtime detection over a hardcoded path because upstream path documentation is weak compared with Firefox.

## Zen

Family: Firefox-derived browser

Likely Linux base path:

- `~/.zen`

Likely macOS base path:

- `~/Library/Application Support/Zen Browser`

Sensitive files and directories:

- same profile internals as Firefox, especially `logins.json`, `key4.db`, `cookies.sqlite`, `storage/`, `sessionstore*`

Notes:

- Upstream build discussions indicate profile/app naming around `zen` and `Zen Browser`, but runtime detection is safer than hardcoding.
- Treat this as Firefox-compatible profile handling.

## Chrome

Linux:

- `~/.config/google-chrome`

macOS:

- `~/Library/Application Support/Google/Chrome`

Sensitive files and directories:

- `Local State`
- `Default/`
- `Profile */`
- profile-local files and directories inside `Default/` or `Profile */`:
- `Login Data`
- `Cookies`
- `Web Data`
- `History`
- `Bookmarks`
- `Preferences`
- `Local Storage/`
- `IndexedDB/`
- `Session Storage/`
- `Extensions/`

Notes:

- Password material is not purely file-based. Chrome can bind credential encryption to macOS Keychain or Linux Secret Service or KWallet when available.
- Moving only the profile directory may not fully migrate saved-password usability across systems or OS user contexts.

## Chromium

Linux:

- `~/.config/chromium`

macOS:

- `~/Library/Application Support/Chromium`

Sensitive files and directories:

- same structure and files as Chrome

Notes:

- Same OS credential store caveats as Chrome.

## Safari

macOS only.

Notes:

- Safari stores sensitive data across multiple locations and also relies heavily on Keychain.
- Apple documents that Safari keeps website data, browsing history, bookmarks, extensions, passwords, and payment card data locally, but Apple does not publish one simple authoritative "all disk paths" guide suitable for safe automation.
- Because of that split, Safari should not be treated as a simple symlink target until verified on a live macOS installation.

Operational conclusion:

- Safari support likely needs a dedicated macOS-specific discovery and migration design rather than a generic "move one directory and symlink it" workflow.

## AWS

Linux and macOS:

- `~/.aws`

Sensitive files and directories:

- `credentials`
- `config`
- `cli/cache/`
- `sso/cache/` when AWS SSO or IAM Identity Center is used

Notes:

- `credentials` may contain long-lived access keys.
- `config` may contain profile endpoints, role assumptions, SSO config, and session settings.
- `cli/cache/` and `sso/cache/` can contain temporary credentials and tokens.

## SSH

Linux and macOS:

- `~/.ssh`

Sensitive files and directories:

- `config`
- `id_*`
- `id_*.pub`
- `known_hosts`
- `authorized_keys`
- `controlmasters/` if user configured it

Notes:

- Private keys are the highest priority material.
- Agent sockets are usually ephemeral and should not be migrated as persistent data.
- Some users also keep SSH certificates, yubikey-related config, and per-host secrets here.

## GnuPG

Linux and macOS:

- `~/.gnupg`

Sensitive files and directories:

- `private-keys-v1.d/`
- `pubring.kbx`
- `trustdb.gpg`
- `gpg.conf`
- `gpg-agent.conf`
- `openpgp-revocs.d/`
- `S.gpg-agent*`

Notes:

- `private-keys-v1.d/` is the critical secret-key store.
- Agent sockets are ephemeral but live inside the GnuPG home.
- Permissions are strict; migration logic must preserve mode and ownership expectations.

## Kubernetes

Linux and macOS:

- `~/.kube`

Sensitive files and directories:

- `config`
- `cache/`
- any client certificate or key files referenced by kubeconfig

Notes:

- kubeconfig files often embed bearer tokens, certificates, and keys inline, not only by path.
- Users may also override with `KUBECONFIG`, so a detector must inspect environment and not only default home paths.

## Docker

Linux and macOS:

- `~/.docker`

Sensitive files and directories:

- `config.json`
- `contexts/`
- `buildx/`
- `trust/`

Notes:

- Registry credentials may be stored in `config.json` if no credential helper is configured.
- On macOS, Docker commonly uses the `osxkeychain` credential helper.
- On Linux, helpers may use `pass`, Secret Service, or other backends.
- As with Chrome, moving the directory alone may not move all secrets.

## Hasura

Primary storage model:

- project-local rather than a single stable home-directory app folder

Common sensitive locations:

- project `config.yaml`
- project `metadata/`
- project `migrations/`
- project `.env`
- Docker Compose files
- Kubernetes manifests or Secrets
- shell history if users pass `--admin-secret`

Notes:

- The Hasura CLI and project layout can expose endpoint URLs and admin secrets in repository-local files.
- The server itself stores metadata in Postgres `hdb_catalog`.
- This is better treated as a "scan project/workspace for Hasura secrets" feature than a simple home-directory migration target.

## Telegram

Linux:

- `~/.local/share/TelegramDesktop`

Flatpak Linux:

- `~/.var/app/org.telegram.desktop/data/TelegramDesktop`

macOS:

- `~/Library/Application Support/Telegram Desktop`

Sensitive files and directories:

- `tdata/`
- application working directory contents generally

Notes:

- Local auth/session state lives under the application data directory.
- This is a good candidate for move-and-symlink support if the app is fully closed first.

## WhatsApp

Current product note:

- As of 2026-04-25, official desktop support is published for macOS and Windows. For other operating systems, WhatsApp directs users to WhatsApp Web.

Linux:

- no official native Linux desktop target to model as a stable app directory
- practical sensitive storage is usually the browser profile used for WhatsApp Web

macOS:

- native app exists
- a strong first-party storage-path document was not identified in this research set

Notes:

- Linux support should likely be addressed through browser-profile migration rather than a dedicated WhatsApp rule.
- macOS support needs live-system verification before any hardcoded path choice.

## Bitwarden

Linux:

- `~/.config/Bitwarden`

Linux Flatpak:

- `~/.var/app/com.bitwarden.desktop/`

Linux Snap:

- `~/snap/bitwarden/current/.config/Bitwarden`

macOS:

- `~/Library/Application Support/Bitwarden`

macOS App Store variant:

- `~/Library/Containers/com.bitwarden.desktop/Data/Library/Application Support/Bitwarden`

CLI:

- Linux: `~/.config/Bitwarden CLI`
- macOS: `~/Library/Application Support/Bitwarden CLI`

Sensitive files and directories:

- desktop application state and local encrypted vault cache
- CLI state and local session data

Notes:

- Browser-extension Bitwarden data is stored inside the browser profile, not the desktop app directory.
- This is a strong candidate for move-and-symlink support.

## Additional Researched Targets

## Git Credential Store

Linux and macOS:

- `~/.git-credentials`
- `$XDG_CONFIG_HOME/git/credentials` (usually `~/.config/git/credentials`)

Related configuration:

- `~/.gitconfig`
- `$XDG_CONFIG_HOME/git/config` (usually `~/.config/git/config`)
- repository-local `.git/config`

Sensitive files and directories:

- `~/.git-credentials`
- `~/.config/git/credentials`
- any remote URLs in Git config files that embed tokens or passwords

Notes:

- This is only applicable when users enable the `store` credential helper.
- `git-credential-store` keeps credentials unencrypted on disk.
- If users rely on `osxkeychain`, `libsecret`, Git Credential Manager, or another helper, the secret may not live in these files.

## GitHub CLI (`gh`)

Linux and macOS default config directory:

- `~/.config/gh`

Sensitive files and directories:

- `config.yml`
- `hosts.yml` or equivalent host credential file

Notes:

- `GH_CONFIG_DIR` overrides the default directory.
- By default, `gh auth login` stores tokens in the system credential store.
- If secure storage is unavailable, or if `--insecure-storage` is used, `gh` falls back to plain-text storage in its config directory.
- `GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`, and `GITHUB_ENTERPRISE_TOKEN` can override disk-stored credentials.

## GitLab CLI (`glab`)

Linux and macOS default global config file:

- `~/.config/glab-cli/config.yml`

Sensitive files and directories:

- `config.yml`

Notes:

- `GLAB_CONFIG_DIR` overrides the global configuration location.
- `glab auth login` stores credentials in the global config file by default.
- `--use-keyring` moves token storage into the operating system keyring instead.
- `GITLAB_TOKEN`, `GITLAB_ACCESS_TOKEN`, and `OAUTH_TOKEN` override stored credentials.

## Google Cloud CLI (`gcloud`)

Linux and macOS default config directory:

- `~/.config/gcloud`

Sensitive files and directories:

- `application_default_credentials.json`
- `configurations/config_*`
- `active_config`
- stored account credentials inside the gcloud config directory
- any credential files referenced by `GOOGLE_APPLICATION_CREDENTIALS`, `--cred-file`, or `--login-config`

Notes:

- `CLOUDSDK_CONFIG` overrides the default directory.
- The Google Cloud docs explicitly distinguish gcloud CLI credentials from Application Default Credentials.
- The ADC file in the well-known location is a strong migration target.
- Service-account key files are often stored outside `~/.config/gcloud` and may be more sensitive than the config directory itself.

## Azure CLI

Linux and macOS default config directory:

- `~/.azure`

Sensitive files and directories:

- `config`
- `azureProfile.json`
- MSAL token cache and service principal entry files under `~/.azure`
- legacy `accessTokens.json` on older pre-MSAL installations

Notes:

- `AZURE_CONFIG_DIR` overrides the default directory.
- Microsoft documents that current MSAL-based Azure CLI stores token cache and service principal entries as plain-text files on Linux and macOS.
- Current upstream operational behavior commonly uses files such as `msal_token_cache.json`; exact cache filenames should be treated as implementation details rather than a stable product contract.

## Oracle Cloud Infrastructure CLI (`oci`)

Linux and macOS:

- `~/.oci/config`
- private key files typically under `~/.oci/*.pem`, though `key_file` can point anywhere

Sensitive files and directories:

- `config`
- the API signing private key referenced by `key_file`
- any passphrase file used to protect the private key

Notes:

- The API signing private key is the main secret.
- The config file contains tenancy, user, region, fingerprint, and key path metadata and is usually not sufficient on its own.

## Terraform

Linux and macOS:

- `~/.terraformrc`
- `~/.terraform.d/credentials.tfrc.json`

Sensitive files and directories:

- `~/.terraform.d/credentials.tfrc.json`
- any `credentials` blocks in `~/.terraformrc`

Notes:

- `terraform login` stores HCP Terraform or Terraform Enterprise tokens in plain text by default.
- `TF_TOKEN_*` environment variables can keep credentials out of files.
- Credentials helpers can move storage into another system.

## OpenTofu

Linux and macOS CLI config locations:

- `~/.tofurc`
- `$XDG_CONFIG_HOME/opentofu/tofurc` (usually `~/.config/opentofu/tofurc`)

Default credentials file:

- `credentials.tfrc.json`
- by compatibility convention this is often found in Terraform-style locations such as `~/.terraform.d/credentials.tfrc.json`, but this should be verified on a live install before hardcoding migration logic

Sensitive files and directories:

- any `credentials` blocks in OpenTofu CLI config
- default `credentials.tfrc.json`
- any inline `oci_credentials` containing username/password, access token, or refresh token

Notes:

- OpenTofu supports Docker-style ambient registry credentials and explicit CLI config credentials.
- If users rely on Docker auth files or credential helpers, the real secret may live in `~/.docker/config.json`, containers auth files, or an OS keychain rather than the OpenTofu config itself.

## Pulumi

Linux and macOS default home:

- `~/.pulumi`

Sensitive files and directories:

- `~/.pulumi/credentials.json`
- project-local `Pulumi.<stack>.yaml`
- project-local `Pulumi.yaml`
- local or DIY backend state files and checkpoints

Notes:

- `PULUMI_HOME` overrides the default home directory.
- Pulumi Cloud login state is stored in `credentials.json`.
- Stack config files can contain either plain-text config or encrypted secret ciphertext.
- `PULUMI_ACCESS_TOKEN`, `PULUMI_CONFIG_PASSPHRASE`, and `PULUMI_CONFIG_PASSPHRASE_FILE` can move the most important secret material outside on-disk defaults.

## Python Publishing (`.pypirc`)

Linux and macOS:

- `~/.pypirc`

Sensitive files and directories:

- repository URLs
- usernames
- passwords

Notes:

- `.pypirc` is a plain-text credentials file used by tools such as twine and flit.

## RubyGems

Linux and macOS:

- `~/.gem/credentials`
- `~/.local/share/gem/credentials`

Sensitive files and directories:

- API keys stored in credentials files

Notes:

- `gem push` uses `~/.gem/credentials` by default.
- Modern RubyGems documentation also references `~/.local/share/gem/credentials`.
- `GEM_HOST_API_KEY` can keep credentials out of files.

## Cargo

Linux and macOS:

- `~/.cargo/config.toml`
- `~/.cargo/credentials.toml`
- legacy `~/.cargo/credentials`

Sensitive files and directories:

- registry tokens in `credentials.toml` or legacy `credentials`
- any token-related config in `config.toml`

Notes:

- `CARGO_HOME` overrides the default Cargo home.
- The default `cargo:token` credential provider stores credentials in plain text.
- Cargo also supports OS-backed credential providers and recommends configuring them.

## Product Guidance

Good first candidates for automated move-and-symlink support:

- AWS
- SSH
- GnuPG
- Kubernetes
- Telegram Desktop
- Firefox-family browser profiles
- Git credential store files
- GitHub CLI when `gh` is using plain-text fallback storage
- GitLab CLI when `glab` is not using keyring storage
- OCI CLI key and config files
- `.pypirc`
- RubyGems credentials files
- Cargo credentials files

Support that likely needs warnings or partial support:

- Chrome
- Chromium
- Docker
- Google Cloud CLI
- Azure CLI
- Terraform
- OpenTofu
- Pulumi

Reason:

- these often split sensitive material between app data, environment variables, credential helpers, keychains, cloud backends, or project-local files

Support that likely needs bespoke handling instead of one default home path:

- Safari
- Hasura
- WhatsApp on Linux

Reason:

- Safari is strongly integrated with macOS Keychain and scattered data locations
- Hasura is project and environment driven rather than home-directory driven
- WhatsApp on Linux is effectively browser-profile data

## Sources

Firefox:

- https://support.mozilla.org/en-US/kb/profiles-where-firefox-stores-user-data

Chromium and Chrome:

- https://chromium.googlesource.com/chromium/src/+/HEAD/docs/user_data_dir.md
- https://chromium.googlesource.com/playground/chromium-org-site/+/refs/heads/main/developers/design-documents/preferences.md
- https://chromium.googlesource.com/chromium/src/+/main/components/os_crypt/

AWS:

- https://docs.aws.amazon.com/cli/v1/userguide/cli-configure-files.html

OpenSSH:

- https://man.openbsd.org/ssh.1
- https://man.openbsd.org/ssh_config.5
- https://man.openbsd.org/ssh-agent.1

GnuPG:

- https://gnupg.org/documentation/manuals/gnupg/GPG-Configuration-Options.html
- https://www.gnupg.org/documentation/manuals/gnupg26/gpg-agent.1.html

Kubernetes:

- https://kubernetes.io/docs/concepts/configuration/organize-cluster-access-kubeconfig/
- https://kubernetes.io/docs/reference/kubectl/generated/kubectl_config/

Docker:

- https://docs.docker.com/reference/cli/docker/
- https://docs.docker.com/reference/cli/docker/login/

Hasura:

- https://hasura.io/learn/graphql/hasura-advanced/migrations-metadata/1-hasura-cli/
- https://hasura.io/learn/graphql/hasura-advanced/migrations-metadata/3-metadata/
- https://hasura.io/blog/hasura-authentication-explained

Bitwarden:

- https://bitwarden.com/help/data-storage/
- https://bitwarden.com/help/cli/

Git and forge tooling:

- https://git-scm.com/docs/git-credential-store.html
- https://cli.github.com/manual/gh_help_environment
- https://cli.github.com/manual/gh_auth_login
- https://docs.gitlab.com/cli/auth/login/
- https://docs.gitlab.com/cli/

Google Cloud CLI:

- https://docs.cloud.google.com/sdk/docs/configurations
- https://docs.cloud.google.com/sdk/docs/authorizing
- https://cloud.google.com/sdk/gcloud/reference/auth/application-default/login
- https://cloud.google.com/docs/authentication/application-default-credentials

Azure CLI:

- https://learn.microsoft.com/en-us/cli/azure/authenticate-azure-cli
- https://learn.microsoft.com/en-us/cli/azure/azure-cli-configuration?view=azure-cli-latest
- https://learn.microsoft.com/en-us/cli/azure/msal-based-azure-cli?view=azure-cli-latest

OCI CLI:

- https://docs.oracle.com/iaas/Content/API/Concepts/sdkconfig.htm
- https://docs.oracle.com/en-us/iaas/Content/API/SDKDocs/clienvironmentvariables.htm
- https://docs.oracle.com/en-us/iaas/tools/oci-cli/latest/oci_cli_docs/cmdref/setup/keys.html

Terraform and OpenTofu:

- https://developer.hashicorp.com/terraform/cli/config/config-file
- https://developer.hashicorp.com/terraform/cli/commands/login
- https://developer.hashicorp.com/terraform/cli/commands/logout
- https://opentofu.org/docs/v1.11/cli/config/config-file/
- https://opentofu.org/docs/v1.6/cli/commands/login/
- https://opentofu.org/docs/cli/oci_registries/credentials/

Pulumi:

- https://www.pulumi.com/docs/concepts/state/
- https://www.pulumi.com/docs/iac/cli/environment-variables/
- https://www.pulumi.com/docs/iac/concepts/secrets/

Publishing credentials:

- https://packaging.python.org/specifications/pypirc/
- https://guides.rubygems.org/api-key-scopes/
- https://guides.rubygems.org/command-reference/
- https://doc.rust-lang.org/cargo/guide/cargo-home.html
- https://doc.rust-lang.org/cargo/reference/config.html
- https://doc.rust-lang.org/cargo/reference/registry-authentication.html

Telegram:

- https://github.com/telegramdesktop/tdesktop
- https://github.com/telegramdesktop/tdesktop/issues/28460
- https://github.com/telegramdesktop/tdesktop/issues/26962

WhatsApp:

- https://www.whatsapp.com/download/desktop

Zen:

- https://github.com/zen-browser/desktop/issues/722

Floorp:

- upstream source and operational behavior were used as Firefox-family inference; explicit vendor path documentation is weaker than Firefox
