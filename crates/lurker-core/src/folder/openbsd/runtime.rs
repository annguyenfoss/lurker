//! Administrator-owned CryFS and native loader prerequisites, before secrets.
use super::*;
use serde_json::Value;

fn invalid() -> AppError {
    AppError::new(
        "Install the complete administrator-owned OpenBSD CryFS package with native unveil protocol 1.",
    )
}

fn manifest_path(executable: &Path) -> AppResult<PathBuf> {
    let directory = executable.parent().ok_or_else(invalid)?;
    let adjacent = directory.join("cryfs.json");
    match fs::symlink_metadata(&adjacent) {
        Ok(_) => return Ok(adjacent),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(AppError::io("Inspect CryFS provenance", e)),
    }
    if !directory.ends_with("libexec/lurker/backends") {
        return Err(invalid());
    }
    let prefix = directory
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(invalid)?;
    Ok(prefix.join("share/lurker/provenance/cryfs/cryfs.json"))
}

fn tags(text: &str, tag: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.contains(&format!("({tag})")))
        .filter_map(|line| {
            line.split_once('[')
                .and_then(|(_, value)| value.split_once(']'))
                .map(|(value, _)| value.to_owned())
        })
        .collect()
}

pub(super) fn validate(executable: &Path) -> AppResult<PathBuf> {
    let executable = crate::trusted_privileged_executable(executable)?;
    let bytes = crate::trust::trusted_runtime_file(&executable, 64 * 1024 * 1024)?;
    let manifest = crate::trust::trusted_runtime_file(&manifest_path(&executable)?, 1024 * 1024)?;
    let value: Value = serde_json::from_slice(&manifest).map_err(|_| invalid())?;
    contract(&value, &bytes)?;
    let host = &value["host_fuse"];
    let library = host["library"].as_str().ok_or_else(invalid)?;
    let provider = PathBuf::from(host["path"].as_str().ok_or_else(invalid)?);
    if host["api"].as_str() != Some("fuse2")
        || provider.parent() != Some(Path::new("/usr/lib"))
        || provider.file_name().and_then(|v| v.to_str()) != Some(library)
        || !library.starts_with("libfuse.so.")
        || !library[11..]
            .split('.')
            .all(|n| !n.is_empty() && n.bytes().all(|v| v.is_ascii_digit()))
    {
        return Err(invalid());
    }
    let current = crate::trust::trusted_runtime_file(&provider, 16 * 1024 * 1024)?;
    if !current.starts_with(b"\x7fELF") {
        return Err(invalid());
    }
    let dynamic = crate::bsd::folder_runtime_probe(
        "/usr/bin/readelf",
        &["-d".into(), executable.as_os_str().to_owned()],
    )?;
    let host_dynamic = crate::bsd::folder_runtime_probe(
        "/usr/bin/readelf",
        &["-d".into(), provider.as_os_str().to_owned()],
    )?;
    if !tags(&dynamic, "RPATH").is_empty()
        || !tags(&dynamic, "RUNPATH").is_empty()
        || tags(&host_dynamic, "SONAME") != [library]
        || tags(&dynamic, "NEEDED")
            .iter()
            .filter(|name| name.starts_with("libfuse"))
            .collect::<Vec<_>>()
            != [&library.to_owned()]
    {
        return Err(invalid());
    }
    // Native loader trace with a cleared environment. Every resolved provider
    // must remain in administrator-controlled native library directories.
    let resolved =
        crate::bsd::folder_runtime_probe("/usr/bin/ldd", &[executable.as_os_str().to_owned()])?;
    if resolved.len() > 65536 || resolved.contains("not found") {
        return Err(invalid());
    }
    let mut fuse = Vec::new();
    let mut libraries = 0;
    for line in resolved.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(2) != Some(&"rlib") && fields.get(2) != Some(&"ld.so") {
            continue;
        }
        let path = Path::new(fields.last().ok_or_else(invalid)?);
        if !matches!(path.parent(), Some(p) if p == Path::new("/usr/lib") || p == Path::new("/usr/local/lib") || p == Path::new("/usr/libexec"))
        {
            return Err(invalid());
        }
        crate::trust::trusted_runtime_file(path, 64 * 1024 * 1024)?;
        libraries += 1;
        if path
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|name| name.starts_with("libfuse"))
        {
            fuse.push(path.to_owned());
        }
    }
    if libraries == 0 || fuse != [provider] {
        return Err(invalid());
    }
    Ok(executable)
}

fn contract(value: &Value, bytes: &[u8]) -> AppResult<()> {
    if !bytes.starts_with(b"\x7fELF")
        || value["schema"].as_u64() != Some(1)
        || value["backend"].as_str() != Some("cryfs")
        || value["revision"].as_str() != Some("5d0cc36785d93e12250696a3ffc928042f2e8d61")
        || value["version"].as_str() != Some("1.0.3")
        || value["sha256"].as_str() != Some(crate::sha256::hex_digest(bytes).as_str())
        || value["runtime"]["privilege"].as_str() != Some("root")
        || value["runtime"]["source_kind"].as_str() != Some("directory")
        || value["runtime"]["native_unveil_protocol"].as_u64() != Some(1)
        || value["patches"]["native/folder/cryfs/fsyncdir.patch"].as_str()
            != Some("407bde343553b88077eaec342213f4f0c5d735e148e504ed04351ed60e34a68f")
        || value["patches"]["native/folder/cryfs/openbsd-platform.patch"].as_str()
            != Some("18b6c821e121b5bcb52d433b6041587d1a414d42e94032d8405bb0fc94d35087")
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privileged_contract_binds_native_port_protocol_and_engine_bytes() {
        let bytes = b"\x7fELFtest";
        let mut value = serde_json::json!({"schema":1,"backend":"cryfs","version":"1.0.3",
            "revision":"5d0cc36785d93e12250696a3ffc928042f2e8d61","sha256":crate::sha256::hex_digest(bytes),
            "runtime":{"privilege":"root","source_kind":"directory","native_unveil_protocol":1},
            "patches":{"native/folder/cryfs/fsyncdir.patch":"407bde343553b88077eaec342213f4f0c5d735e148e504ed04351ed60e34a68f",
                "native/folder/cryfs/openbsd-platform.patch":"18b6c821e121b5bcb52d433b6041587d1a414d42e94032d8405bb0fc94d35087"}});
        assert!(contract(&value, bytes).is_ok());
        assert!(contract(&value, b"\x7fELFreplaced").is_err());
        value["runtime"]["native_unveil_protocol"] = 0.into();
        assert!(contract(&value, bytes).is_err());
    }
}
