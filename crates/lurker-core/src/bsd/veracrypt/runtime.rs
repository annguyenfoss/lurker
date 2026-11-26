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
    host_fuse: HostFuse,
}

struct HostFuse {
    library: String,
    path: PathBuf,
}

fn host_fuse(value: &Value, platform: &str) -> AppResult<HostFuse> {
    let value = &value["host_fuse"];
    let library = value["library"].as_str().ok_or_else(invalid)?;
    let path = PathBuf::from(value["path"].as_str().ok_or_else(invalid)?);
    let sha256 = value["sha256"].as_str().ok_or_else(invalid)?;
    let expected = match platform {
        "FreeBSD" => Path::new("/usr/local/lib"),
        "OpenBSD" => Path::new("/usr/lib"),
        _ => return Err(invalid()),
    };
    let numbers = |name: &str| {
        name.strip_prefix("libfuse.so.").is_some_and(|tail| {
            !tail.is_empty()
                && tail
                    .split('.')
                    .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
    };
    if value["api"].as_str() != Some("fuse2")
        || !digest(sha256)
        || !normal_name(library)
        || !numbers(library)
        || (platform == "FreeBSD" && library != "libfuse.so.2")
        || (platform == "OpenBSD"
            && library
                .strip_prefix("libfuse.so.")
                .unwrap()
                .split('.')
                .count()
                != 2)
        || path.parent() != Some(expected)
        || path.file_name().and_then(|n| n.to_str()) != Some(library)
        || path.components().any(|c| {
            !matches!(
                c,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err(invalid());
    }
    Ok(HostFuse {
        library: library.to_owned(),
        path,
    })
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
        "FreeBSD" => ["libiconv.so.2"].into_iter().map(String::from).collect(),
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
        host_fuse: host_fuse(value, platform)?,
    })
}

fn validate_assets(program: &Path) -> AppResult<(Layout, Inventory)> {
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
    for (logical, expected) in &inventory.files {
        let limit = if logical == NAME || logical.starts_with("lib/") {
            64 * 1024 * 1024
        } else {
            4 * 1024 * 1024
        };
        let bytes = crate::trust::trusted_runtime_file(&layout.file(&logical), limit)?;
        if crate::sha256::hex_digest(&bytes) != *expected {
            return Err(AppError::new("BSD VeraCrypt runtime hash mismatch; reinstall the complete administrator-owned package."));
        }
    }
    verify_host_fuse(&inventory.host_fuse)?;
    Ok((layout, inventory))
}

/// Host packages may receive compatible administrator-controlled updates.
/// The manifest hash is build provenance; ownership, ABI and live resolution
/// authorize runtime use. Version aliases may change only within the host libdir.
fn host_provider(path: &Path) -> AppResult<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let directory = path.parent().ok_or_else(invalid)?;
    crate::trust::trusted_runtime_directory(directory)?;
    let mut current = path.to_owned();
    for _ in 0..16 {
        let metadata = fs::symlink_metadata(&current)
            .map_err(|e| AppError::io("Inspect host FUSE prerequisite", e))?;
        if metadata.uid() != 0 {
            return Err(invalid());
        }
        if !metadata.file_type().is_symlink() {
            crate::trust::trusted_runtime_file(&current, 64 * 1024 * 1024)?;
            return Ok(current);
        }
        let target =
            fs::read_link(&current).map_err(|e| AppError::io("Read host FUSE ABI alias", e))?;
        current = if target.is_absolute() {
            target
        } else {
            if target.components().count() != 1 {
                return Err(invalid());
            }
            directory.join(target)
        };
        if current.parent() != Some(directory)
            || current.components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
        {
            return Err(invalid());
        }
    }
    Err(AppError::new(
        "Host FUSE ABI alias chain is cyclic or too long.",
    ))
}

fn verify_host_fuse(host: &HostFuse) -> AppResult<PathBuf> {
    host_provider(&host.path)
}

fn dynamic_tags(text: &str, tag: &str) -> AppResult<Vec<String>> {
    if text.len() > 65536 {
        return Err(invalid());
    }
    let marker = format!("({tag})");
    text.lines()
        .filter(|line| line.contains(&marker))
        .map(|line| {
            let (_, value) = line.split_once('[').ok_or_else(invalid)?;
            let (value, rest) = value.split_once(']').ok_or_else(invalid)?;
            if !normal_name(value) || !rest.trim().is_empty() {
                return Err(invalid());
            }
            Ok(value.to_owned())
        })
        .collect()
}

fn inspect_dynamic(path: &Path, tag: &str) -> AppResult<Vec<String>> {
    let text = super::super::exec_in_locale(
        "/usr/bin/readelf",
        &["-d".into(), path.as_os_str().to_owned()],
        None,
        "C",
        None,
        std::time::Duration::from_secs(30),
        None,
    )?;
    dynamic_tags(&text, tag)
}

fn resolved_fuse_paths(text: &str) -> AppResult<Vec<PathBuf>> {
    if text.len() > 65536 || text.contains("not found") {
        return Err(invalid());
    }
    let paths: Vec<_> = text
        .split_whitespace()
        .filter(|value| value.starts_with('/'))
        .map(PathBuf::from)
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("libfuse.so."))
        })
        .collect();
    if paths.len() != 1 {
        return Err(invalid());
    }
    Ok(paths)
}

fn verify_fuse_resolution(resolved: &Path, host: &HostFuse) -> AppResult<()> {
    if resolved.parent() != host.path.parent()
        || host_provider(resolved)? != verify_host_fuse(host)?
    {
        return Err(AppError::new(
            "BSD loader resolves FUSE outside the qualified host prerequisite.",
        ));
    }
    Ok(())
}

pub(super) fn validate(program: &Path) -> AppResult<()> {
    let (layout, inventory) = validate_assets(program)?;
    let bytes = crate::trust::trusted_runtime_file(program, 64 * 1024 * 1024)?;
    if !bytes.starts_with(b"\x7fELF") {
        return Err(AppError::new(
            "Private BSD VeraCrypt backend must be a native ELF executable.",
        ));
    }
    let expected = std::slice::from_ref(&inventory.host_fuse.library);
    let provider = verify_host_fuse(&inventory.host_fuse)?;
    if inspect_dynamic(&provider, "SONAME")? != expected {
        return Err(AppError::new(
            "Host FUSE SONAME differs from the qualified prerequisite.",
        ));
    }
    let needed: Vec<_> = inspect_dynamic(program, "NEEDED")?
        .into_iter()
        .filter(|n| n.starts_with("libfuse"))
        .collect();
    if needed != expected {
        return Err(AppError::new(
            "Backend requires a different FUSE ABI than its host prerequisite.",
        ));
    }
    for library in &inventory.libraries {
        if inspect_dynamic(&layout.directory.join("lib").join(library), "SONAME")?
            != [library.clone()]
        {
            return Err(AppError::new(
                "Unexpected private BSD library ABI; FUSE must remain host-owned.",
            ));
        }
    }
    // The trusted OS loader trace runs with a cleared environment after every
    // declared provider and private asset has passed ownership and hash checks.
    let text = super::super::exec_in_locale(
        "/usr/bin/ldd",
        &[program.as_os_str().to_owned()],
        None,
        "C",
        None,
        std::time::Duration::from_secs(30),
        None,
    )?;
    verify_fuse_resolution(&resolved_fuse_paths(&text)?.remove(0), &inventory.host_fuse)
}

/// Older OpenBSD engines abort while enumerating physical disks. Require the
/// source producer's explicit native-device contract before any credential I/O.
pub(super) fn validate_physical(program: &Path) -> AppResult<()> {
    validate(program)?;
    let layout = Layout::locate(program)?;
    let bytes = crate::trust::trusted_runtime_file(&layout.manifest, MAX_MANIFEST)?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    physical_contract(&manifest)
}

fn physical_contract(manifest: &Value) -> AppResult<()> {
    if manifest["runtime"]["physical_source_protocol"].as_u64() != Some(1) {
        return Err(AppError::new("The private BSD VeraCrypt package lacks qualified physical-device discovery; install the matching current Lurker package."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_contract_is_explicit_and_versioned() {
        let mut value = manifest("OpenBSD");
        assert!(physical_contract(&value).is_err());
        value["runtime"]["physical_source_protocol"] = 1.into();
        assert!(physical_contract(&value).is_ok());
        value["runtime"]["physical_source_protocol"] = 2.into();
        assert!(physical_contract(&value).is_err());
    }

    fn manifest(platform: &str) -> Value {
        let hash = "a".repeat(64);
        let mut value = serde_json::json!({"schema":1,"backend":"veracrypt","platform":platform,"binary":NAME,
            "binary_sha256":hash,"runtime":{"privilege":"root","source_kind":"regular_file"},
            "private_libraries":[],"files":{NAME:hash,"licenses/VeraCrypt.txt":hash,"licenses/wxWidgets.txt":hash}});
        if platform == "FreeBSD" {
            for name in ["libiconv.so.2"] {
                value["private_libraries"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"name":name,"sha256":hash}));
                value["files"][format!("lib/{name}")] = hash.clone().into();
            }
        }
        let (library, path) = if platform == "FreeBSD" {
            ("libfuse.so.2", "/usr/local/lib/libfuse.so.2")
        } else {
            ("libfuse.so.4.0", "/usr/lib/libfuse.so.4.0")
        };
        value["host_fuse"] =
            serde_json::json!({"api":"fuse2","library":library,"path":path,"sha256":hash});
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
        let duplicate = value["private_libraries"][0].clone();
        value["private_libraries"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(inventory(&value, "FreeBSD").is_err());
        assert!(inventory(&manifest("OpenBSD"), "FreeBSD").is_err());
    }

    #[test]
    fn host_fuse_contract_is_external_and_build_hash_is_provenance() {
        let mut value = manifest("OpenBSD");
        for path in ["/tmp/libfuse.so.4.0", "/usr/lib/../lib/libfuse.so.4.0"] {
            value["host_fuse"]["path"] = path.into();
            assert!(inventory(&value, "OpenBSD").is_err());
        }
        value = manifest("OpenBSD");
        value["host_fuse"]["sha256"] = "b".repeat(64).into();
        assert!(inventory(&value, "OpenBSD").is_ok());
        value["host_fuse"]["api"] = "fuse3".into();
        assert!(inventory(&value, "OpenBSD").is_err());
        let mut value = manifest("FreeBSD");
        value["private_libraries"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"name":"libfuse.so.2","sha256":"a".repeat(64)}));
        value["files"]["lib/libfuse.so.2"] = "a".repeat(64).into();
        assert!(inventory(&value, "FreeBSD").is_err());
    }

    #[test]
    fn native_loader_audits_require_a_single_matching_fuse_provider() {
        let freebsd = "libfuse.so.2 => /usr/local/lib/libfuse.so.2 (0x10)\nlibc.so.7 => /lib/libc.so.7 (0x20)\n";
        let openbsd = "0000 0001 rlib 0 1 0 /usr/lib/libfuse.so.4.0\n";
        assert_eq!(
            resolved_fuse_paths(freebsd).unwrap(),
            vec![PathBuf::from("/usr/local/lib/libfuse.so.2")]
        );
        assert_eq!(
            resolved_fuse_paths(openbsd).unwrap(),
            vec![PathBuf::from("/usr/lib/libfuse.so.4.0")]
        );
        assert!(resolved_fuse_paths("libfuse.so.2 => not found").is_err());
        assert!(resolved_fuse_paths(&(freebsd.to_owned() + openbsd)).is_err());
        assert_eq!(
            dynamic_tags(" 0x0 (NEEDED) Shared library: [libfuse.so.2]\n", "NEEDED").unwrap(),
            vec!["libfuse.so.2"]
        );
        assert!(dynamic_tags("(NEEDED) [../libfuse.so.2]", "NEEDED").is_err());
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
    #[ignore = "root BSD; metadata-only host-library ownership, alias and compatible-update fixtures"]
    fn host_library_updates_require_administrator_control_through_every_alias() {
        use std::os::unix::fs::PermissionsExt;
        crate::bsd::require_root().unwrap();
        let root = PathBuf::from(format!(
            "/var/run/lurker-host-fuse-audit-{}",
            crate::bsd::unique_id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let first = root.join("libfuse.so.2.1");
        let second = root.join("libfuse.so.2.2");
        let alias = root.join("libfuse.so.2");
        fs::write(&first, b"old compatible provider fixture").unwrap();
        fs::write(&second, b"updated compatible provider fixture").unwrap();
        std::os::unix::fs::symlink(first.file_name().unwrap(), &alias).unwrap();
        assert_eq!(host_provider(&alias).unwrap(), first);
        let host = HostFuse {
            library: "libfuse.so.2".into(),
            path: alias.clone(),
        };
        let trace = format!("libfuse.so.2 => {} (0x10)\n", first.display());
        verify_fuse_resolution(&resolved_fuse_paths(&trace).unwrap()[0], &host).unwrap();
        assert!(verify_fuse_resolution(&second, &host).is_err());
        fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(second.file_name().unwrap(), &alias).unwrap();
        assert_eq!(host_provider(&alias).unwrap(), second);
        verify_fuse_resolution(&second, &host).unwrap();
        assert!(verify_fuse_resolution(&first, &host).is_err());
        fs::set_permissions(&second, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(host_provider(&alias).is_err());
        fs::set_permissions(&second, fs::Permissions::from_mode(0o644)).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(host_provider(&alias).is_err());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink("../outside-provider", &alias).unwrap();
        assert!(host_provider(&alias).is_err());
        fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(alias.file_name().unwrap(), &alias).unwrap();
        assert!(host_provider(&alias).is_err());
        fs::remove_dir_all(root).unwrap();
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
            assert!(validate_assets(&program).is_ok());
            // A shell stand-in is not a native ELF backend. Actual qualified
            // engine execution is covered by the separate native version probe.
            assert!(version().is_err());
            assert!(!marker.exists());
            let reject = || {
                assert!(validate_assets(&program).is_err());
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
            assert!(validate_assets(&program).is_ok());
            assert!(version().is_err());
            assert!(!marker.exists());
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
