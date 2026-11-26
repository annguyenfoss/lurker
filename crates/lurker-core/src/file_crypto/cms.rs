//! OpenSSL CMS file encryption. The CLI handles certificate-only operations.
//! Shared credentials use the same CMS format through the installed libcrypto.

mod library;

use super::{FileOperation, FileRequest};
use crate::{AppError, AppResult};
use std::env;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn run(request: &FileRequest, staged: &Path) -> AppResult<()> {
    let openssl = Openssl::find()?;
    if request.passphrase.is_some() || request.key_file.is_some() {
        return library::run(&openssl, request, staged);
    }

    let mut command = Command::new(&openssl.executable);
    command
        .args(["cms", "-binary"])
        .arg("-in")
        .arg(&request.input);
    command.arg("-out").arg(staged);
    match request.operation {
        FileOperation::Encrypt => {
            command.args(["-encrypt", "-aes-256-gcm", "-stream", "-outform", "DER"]);
            for recipient in &request.public_keys {
                command.arg("-recip").arg(recipient);
            }
            command.stdin(Stdio::null());
        }
        FileOperation::Decrypt => {
            command.args(["-decrypt", "-inform", "DER"]);
            let key = request.private_key.as_ref().ok_or_else(|| {
                AppError::new(
                    "OpenSSL CMS decryption needs a private key, AES keyfile, or passphrase.",
                )
            })?;
            command.arg("-inkey").arg(key);
            if let Some(certificate) = &request.certificate {
                command.arg("-recip").arg(certificate);
            }
            // OpenSSL prompts on its terminal only if the private key needs a
            // passphrase. Do not put that passphrase in the command arguments.
            command.stdin(Stdio::inherit());
        }
    }
    let status = command
        .stdout(Stdio::null())
        .status()
        .map_err(|error| AppError::io("Run OpenSSL CMS", error))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(format!("OpenSSL CMS failed ({status}).")))
    }
}

struct Openssl {
    executable: PathBuf,
    version: String,
}

impl Openssl {
    fn find() -> AppResult<Self> {
        let path = env::var_os("PATH").unwrap_or_default();
        let candidates = env::split_paths(&path)
            .filter(|directory| directory.is_absolute())
            .map(|directory| directory.join("openssl"))
            .chain([
                PathBuf::from("/usr/local/bin/eopenssl"),
                PathBuf::from("/opt/homebrew/opt/openssl@3/bin/openssl"),
                PathBuf::from("/usr/local/opt/openssl@3/bin/openssl"),
                PathBuf::from("/usr/local/bin/openssl"),
                PathBuf::from("/usr/pkg/bin/openssl"),
                PathBuf::from("/usr/bin/openssl"),
            ]);
        for candidate in candidates {
            let Some(executable) = backend_path(&candidate) else {
                continue;
            };
            let Ok(probe) = Command::new(&executable)
                .arg("version")
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
            else {
                continue;
            };
            if probe.status.success() {
                if let Some(version) = openssl_version(&String::from_utf8_lossy(&probe.stdout)) {
                    return Ok(Self {
                        executable,
                        version,
                    });
                }
            }
        }
        Err(AppError::new(
            "OpenSSL 3 or newer is required for CMS. Install OpenSSL or add its executable to PATH.",
        ))
    }

    fn library_candidates(&self) -> impl Iterator<Item = PathBuf> {
        let mut directories = Vec::new();
        if let Ok(result) = Command::new(&self.executable)
            .args(["info", "-modulesdir"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        {
            if result.status.success() {
                let modules = PathBuf::from(String::from_utf8_lossy(&result.stdout).trim());
                if modules.is_absolute() {
                    directories.extend(modules.ancestors().skip(1).take(2).map(Path::to_path_buf));
                }
            }
        }
        if let Some(prefix) = self.executable.ancestors().nth(2) {
            directories.extend([prefix.join("lib"), prefix.join("lib64")]);
        }
        directories.extend([PathBuf::from("/lib"), PathBuf::from("/usr/lib")]);

        directories.into_iter().flat_map(|directory| {
            let mut paths: Vec<_> = fs::read_dir(directory)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| libcrypto_filename(&entry.file_name().to_string_lossy()))
                .map(|entry| entry.path())
                .collect();
            paths.sort_unstable();
            paths
        })
    }
}

fn openssl_version(description: &str) -> Option<String> {
    let mut fields = description.split_whitespace();
    if fields.next()? != "OpenSSL" {
        return None;
    }
    let version = fields.next()?;
    let major = version.split('.').next()?.parse::<u32>().ok()?;
    (major >= 3).then(|| version.to_owned())
}

fn libcrypto_filename(name: &str) -> bool {
    if let Some(suffix) = name.strip_prefix("libcrypto.so") {
        suffix.is_empty()
            || (suffix.starts_with('.') && suffix[1..].split('.').all(version_component))
    } else if let Some(version) = name
        .strip_prefix("libcrypto.")
        .and_then(|suffix| suffix.strip_suffix(".dylib"))
    {
        version.split('.').all(version_component)
    } else {
        false
    }
}

fn version_component(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())
}

fn backend_path(path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    if !canonical.is_file() {
        return None;
    }
    // A normal invocation may use Homebrew and its own PATH. If invoked as
    // root, never execute or dlopen something another user can replace.
    if unsafe { libc::geteuid() } == 0
        && !canonical.ancestors().all(|ancestor| {
            fs::symlink_metadata(ancestor).is_ok_and(|metadata| {
                !metadata.is_symlink() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0
            })
        })
    {
        return None;
    }
    Some(canonical)
}
