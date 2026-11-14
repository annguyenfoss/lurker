//! Validate the bounded private loader closure before executing even --version.
use crate::{AppError, AppResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const NAME: &str = "lurker-veracrypt-bsd";
const MANIFEST: &str = "bsd-veracrypt-source.json";
const MAX_MANIFEST: u64 = 1024 * 1024;

struct Layout {
    directory: PathBuf,
    manifest: PathBuf,
    licenses: PathBuf,
}

impl Layout {
    fn locate(program: &Path) -> AppResult<Self> {
        let directory = program.parent().ok_or_else(invalid)?.to_owned();
        let adjacent = directory.join(MANIFEST);
        // An invalid adjacent entry must fail, not fall through to another
        // inventory. Development producers retain their original layout.
        match fs::symlink_metadata(&adjacent) {
            Ok(_) => {
                return Ok(Self {
                    manifest: adjacent,
                    licenses: directory.join("licenses"),
                    directory,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::io("Inspect BSD runtime manifest", e)),
        }
        let lurker = directory.parent().ok_or_else(invalid)?;
        let libexec = lurker.parent().ok_or_else(invalid)?;
        let prefix = libexec.parent().ok_or_else(invalid)?;
        if directory.file_name().is_none_or(|n| n != "backends")
            || lurker.file_name().is_none_or(|n| n != "lurker")
            || libexec.file_name().is_none_or(|n| n != "libexec")
        {
            return Err(invalid());
        }
        // stage-unix intentionally moves provenance and notices out of the
        // executable directory. No ancestor search or host-library fallback.
        let share = prefix.join("share/lurker");
        Ok(Self {
            manifest: share.join("provenance/veracrypt-bsd").join(MANIFEST),
            licenses: share.join("licenses/veracrypt-bsd"),
            directory,
        })
    }

    fn file(&self, logical: &str) -> PathBuf {
        match logical.strip_prefix("licenses/") {
            Some(name) => self.licenses.join(name),
            None => self.directory.join(logical),
        }
    }
}

fn invalid() -> AppError {
    AppError::new("BSD VeraCrypt runtime inventory is missing or invalid; install the complete administrator-owned Lurker package.")
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn normal_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
}

struct Inventory {
    files: BTreeMap<String, String>,
    libraries: BTreeSet<String>,
}

fn inventory(value: &Value, platform: &str) -> AppResult<Inventory> {
    if value["schema"].as_u64() != Some(1)
        || value["backend"].as_str() != Some("veracrypt")
        || value["platform"].as_str() != Some(platform)
        || value["binary"].as_str() != Some(NAME)
        || value["runtime"]["privilege"].as_str() != Some("root")
        || value["runtime"]["source_kind"].as_str() != Some("regular_file")
    {
        return Err(invalid());
    }
    let entries = value["files"].as_object().ok_or_else(invalid)?;
    if entries.len() < 3 || entries.len() > 64 {
        return Err(invalid());
    }
    let mut files = BTreeMap::new();
    for (name, hash) in entries {
        let hash = hash.as_str().ok_or_else(invalid)?;
        if name.len() > 512
            || !digest(hash)
            || !name.split('/').all(normal_name)
            || !(name == NAME || name.starts_with("lib/") || name.starts_with("licenses/"))
        {
            return Err(invalid());
        }
        files.insert(name.clone(), hash.to_owned());
    }
    let binary = value["binary_sha256"].as_str().ok_or_else(invalid)?;
    if !digest(binary)
        || files.get(NAME).is_none_or(|hash| hash != binary)
        || !files.contains_key("licenses/VeraCrypt.txt")
        || !files.contains_key("licenses/wxWidgets.txt")
    {
        return Err(invalid());
    }
    let required: BTreeSet<String> = match platform {
        "FreeBSD" => ["libfuse.so.2", "libiconv.so.2"]
            .into_iter()
            .map(String::from)
            .collect(),
        "OpenBSD" => BTreeSet::new(),
        _ => return Err(invalid()),
    };
    let libraries = value["private_libraries"].as_array().ok_or_else(invalid)?;
    if libraries.len() != required.len() {
        return Err(invalid());
    }
    let mut seen = BTreeSet::new();
    for library in libraries {
        let name = library["name"].as_str().ok_or_else(invalid)?;
        let hash = library["sha256"].as_str().ok_or_else(invalid)?;
        if !required.contains(name)
            || !seen.insert(name.to_owned())
            || !digest(hash)
            || files
                .get(&format!("lib/{name}"))
                .is_none_or(|expected| expected != hash)
        {
            return Err(invalid());
        }
    }
    let declared: BTreeSet<_> = files
        .keys()
        .filter_map(|key| key.strip_prefix("lib/"))
        .map(String::from)
        .collect();
    if declared != required || seen != required {
        return Err(invalid());
    }
    Ok(Inventory {
        files,
        libraries: required,
    })
}

pub(super) fn validate(program: &Path) -> AppResult<()> {
    let layout = Layout::locate(program)?;
    let bytes = crate::trust::trusted_runtime_file(&layout.manifest, MAX_MANIFEST)?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    #[cfg(target_os = "freebsd")]
    let platform = "FreeBSD";
    #[cfg(target_os = "openbsd")]
    let platform = "OpenBSD";
    let inventory = inventory(&manifest, platform)?;
    let libraries = layout.directory.join("lib");
    match fs::symlink_metadata(&libraries) {
        Ok(_) => {
            crate::trust::trusted_runtime_directory(&libraries)?;
            let mut seen = BTreeSet::new();
            for entry in fs::read_dir(&libraries)
                .map_err(|e| AppError::io("List BSD private libraries", e))?
            {
                let entry = entry.map_err(|e| AppError::io("Inspect BSD private library", e))?;
                let name = entry.file_name().into_string().map_err(|_| invalid())?;
                if !inventory.libraries.contains(&name) || !seen.insert(name) {
                    return Err(invalid());
                }
            }
            if seen != inventory.libraries {
                return Err(invalid());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && inventory.libraries.is_empty() => {}
        Err(e) => return Err(AppError::io("Inspect BSD private library closure", e)),
    }
    for (logical, expected) in inventory.files {
        let limit = if logical == NAME || logical.starts_with("lib/") {
            64 * 1024 * 1024
        } else {
            4 * 1024 * 1024
        };
        let bytes = crate::trust::trusted_runtime_file(&layout.file(&logical), limit)?;
        if crate::sha256::hex_digest(&bytes) != expected {
            return Err(AppError::new("BSD VeraCrypt runtime hash mismatch; reinstall the complete administrator-owned package."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(platform: &str) -> Value {
        let hash = "a".repeat(64);
        let mut value = serde_json::json!({"schema":1,"backend":"veracrypt","platform":platform,"binary":NAME,
            "binary_sha256":hash,"runtime":{"privilege":"root","source_kind":"regular_file"},
            "private_libraries":[],"files":{NAME:hash,"licenses/VeraCrypt.txt":hash,"licenses/wxWidgets.txt":hash}});
        if platform == "FreeBSD" {
            for name in ["libfuse.so.2", "libiconv.so.2"] {
                value["private_libraries"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"name":name,"sha256":hash}));
                value["files"][format!("lib/{name}")] = hash.clone().into();
            }
        }
        value
    }

    #[test]
    fn inventories_bind_only_the_native_bounded_library_closure() {
        for platform in ["FreeBSD", "OpenBSD"] {
            assert!(inventory(&manifest(platform), platform).is_ok());
            for name in [
                "../libevil.so",
                "lib/../libevil.so",
                "lib//libevil.so",
                "/libevil.so",
                "lib/libevil.so",
            ] {
                let mut value = manifest(platform);
                value["files"][name] = "a".repeat(64).into();
                assert!(inventory(&value, platform).is_err(), "{name}");
            }
            let mut value = manifest(platform);
            value["files"][NAME] = "b".repeat(64).into();
            assert!(inventory(&value, platform).is_err());
            let mut value = manifest(platform);
            value["files"]["licenses/VeraCrypt.txt"] = Value::Null;
            assert!(inventory(&value, platform).is_err());
        }
        let mut value = manifest("FreeBSD");
        value["private_libraries"][0]["sha256"] = "b".repeat(64).into();
        assert!(inventory(&value, "FreeBSD").is_err());
        value = manifest("FreeBSD");
        value["private_libraries"][1] = value["private_libraries"][0].clone();
        assert!(inventory(&value, "FreeBSD").is_err());
        assert!(inventory(&manifest("OpenBSD"), "FreeBSD").is_err());
    }

    #[test]
    #[ignore = "requires root BSD and a complete administrator-owned backend; version only, no storage operations"]
    fn native_backend_version_requires_the_complete_trusted_inventory() {
        crate::bsd::require_root().unwrap();
        let version =
            crate::bsd::veracrypt::invoke(&crate::bsd::args(&["--text", "--version"]), None)
                .unwrap();
        assert!(version.trim().starts_with("VeraCrypt 1.26.31"));
    }

    #[test]
    #[ignore = "requires root BSD metadata fixtures under a trusted /var/run ancestor; no storage operations"]
    fn private_manifest_and_loader_assets_are_checked_before_version_execution() {
        use std::os::unix::fs::PermissionsExt;
        crate::bsd::require_root().unwrap();
        #[cfg(target_os = "freebsd")]
        let platform = "FreeBSD";
        #[cfg(target_os = "openbsd")]
        let platform = "OpenBSD";
        let root = PathBuf::from(format!(
            "/var/run/lurker-bsd-runtime-audit-{}",
            crate::bsd::unique_id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        for installed in [false, true] {
            let base = root.join(if installed {
                "installed"
            } else {
                "development"
            });
            let directory = if installed {
                base.join("libexec/lurker/backends")
            } else {
                base.join("backends")
            };
            fs::create_dir_all(&directory).unwrap();
            let program = directory.join(NAME);
            let marker = base.join("executed");
            let script = format!(
                "#!/bin/sh\n: > '{}'\nprintf 'VeraCrypt 1.26.31\\n'\n",
                marker.display()
            );
            fs::write(&program, script.as_bytes()).unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
            let manifest_path = if installed {
                base.join("share/lurker/provenance/veracrypt-bsd")
                    .join(MANIFEST)
            } else {
                directory.join(MANIFEST)
            };
            fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
            let licenses = if installed {
                base.join("share/lurker/licenses/veracrypt-bsd")
            } else {
                directory.join("licenses")
            };
            fs::create_dir_all(&licenses).unwrap();
            let mut value = manifest(platform);
            let names = value["files"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            for name in names {
                let path = if let Some(relative) = name.strip_prefix("licenses/") {
                    licenses.join(relative)
                } else {
                    directory.join(&name)
                };
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                if name != NAME {
                    fs::write(&path, b"trusted fixture").unwrap();
                }
                let hash = crate::sha256::hex_digest(&fs::read(path).unwrap());
                value["files"][&name] = hash.clone().into();
                if name == NAME {
                    value["binary_sha256"] = hash.into();
                } else if let Some(library) = name.strip_prefix("lib/") {
                    for item in value["private_libraries"].as_array_mut().unwrap() {
                        if item["name"].as_str() == Some(library) {
                            item["sha256"] = hash.clone().into();
                        }
                    }
                }
            }
            let manifest_bytes = serde_json::to_vec(&value).unwrap();
            fs::write(&manifest_path, &manifest_bytes).unwrap();
            let version = || -> AppResult<String> {
                let program = crate::trusted_privileged_executable(&program)?;
                validate(&program)?;
                crate::bsd::exec_utf8(
                    program.to_str().unwrap(),
                    &crate::bsd::args(&["--version"]),
                    None,
                )
            };
            assert!(version().unwrap().contains("VeraCrypt"));
            assert!(marker.exists());
            fs::remove_file(&marker).unwrap();
            let reject = || {
                assert!(version().is_err());
                assert!(
                    !marker.exists(),
                    "untrusted backend executed before validation"
                );
            };
            fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o666)).unwrap();
            reject();
            fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o644)).unwrap();
            let file = fs::File::open(&manifest_path).unwrap();
            use std::os::fd::AsRawFd;
            assert_eq!(unsafe { libc::fchown(file.as_raw_fd(), 65534, !0) }, 0);
            reject();
            assert_eq!(unsafe { libc::fchown(file.as_raw_fd(), 0, !0) }, 0);
            drop(file);
            let held = manifest_path.with_extension("held");
            fs::rename(&manifest_path, &held).unwrap();
            std::os::unix::fs::symlink(&held, &manifest_path).unwrap();
            reject();
            fs::remove_file(&manifest_path).unwrap();
            fs::rename(&held, &manifest_path).unwrap();
            fs::remove_file(&manifest_path).unwrap();
            reject();
            fs::write(&manifest_path, &manifest_bytes).unwrap();
            let libdir = directory.join("lib");
            fs::create_dir_all(&libdir).unwrap();
            fs::set_permissions(&libdir, fs::Permissions::from_mode(0o777)).unwrap();
            reject();
            fs::set_permissions(&libdir, fs::Permissions::from_mode(0o755)).unwrap();
            let extra = libdir.join("libunmanifested.so");
            fs::write(&extra, b"unknown").unwrap();
            reject();
            fs::remove_file(&extra).unwrap();
            if platform == "FreeBSD" {
                let library = libdir.join("libiconv.so.2");
                let original = fs::read(&library).unwrap();
                let file = fs::File::open(&library).unwrap();
                assert_eq!(unsafe { libc::fchown(file.as_raw_fd(), 65534, !0) }, 0);
                reject();
                assert_eq!(unsafe { libc::fchown(file.as_raw_fd(), 0, !0) }, 0);
                drop(file);
                fs::write(&library, b"modified loader payload").unwrap();
                reject();
                fs::write(&library, &original).unwrap();
                let held = base.join("held-library");
                fs::rename(&library, &held).unwrap();
                std::os::unix::fs::symlink(&held, &library).unwrap();
                reject();
                fs::remove_file(&library).unwrap();
                fs::rename(&held, &library).unwrap();
            }
            fs::write(&program, b"#!/bin/sh\nexit 0\n").unwrap();
            reject();
            fs::write(&program, script.as_bytes()).unwrap();
            assert!(version().is_ok());
            fs::remove_file(&marker).unwrap();
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
