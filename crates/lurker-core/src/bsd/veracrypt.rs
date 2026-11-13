//! Private upstream VeraCrypt engine with native BSD filesystem attachment.
//!
//! This adapter never searches PATH or an installed VeraCrypt application.
//! The shared lifecycle owns publication, mount identity and recovery records.
use super::{args, exec_utf8 as exec, Credential};
use crate::{AppError, AppResult, CreateCipher};
use std::ffi::OsString;
use std::fs;
#[cfg(target_os = "openbsd")]
use std::fs::OpenOptions;
#[cfg(target_os = "openbsd")]
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

const NAME: &str = "lurker-veracrypt-bsd";

fn candidates(current: &Path) -> Vec<PathBuf> {
    let Some(directory) = current.parent() else {
        return Vec::new();
    };
    vec![
        directory.join("../libexec/lurker/backends").join(NAME),
        directory.join("backends").join(NAME),
        Path::new("/usr/local/libexec/lurker/backends").join(NAME),
        Path::new("/usr/libexec/lurker/backends").join(NAME),
    ]
}

fn controlled(path: &Path) -> AppResult<PathBuf> {
    let path =
        fs::canonicalize(path).map_err(|e| AppError::io("Resolve private BSD runtime", e))?;
    for ancestor in path.ancestors() {
        let metadata =
            fs::metadata(ancestor).map_err(|e| AppError::io("Inspect private BSD runtime", e))?;
        if metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0 {
            return Err(AppError::new(
                "Private BSD runtime libraries must be administrator-controlled.",
            ));
        }
    }
    Ok(path)
}

pub(super) fn backend() -> AppResult<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("LURKER_BSD_VERACRYPT") {
        paths.push(PathBuf::from(path));
    }
    paths.extend(candidates(
        &std::env::current_exe().map_err(|e| AppError::io("Locate Lurker executable", e))?,
    ));
    let Some(path) = paths.into_iter().find(|path| path.is_file()) else {
        return Err(AppError::new("Private BSD VeraCrypt backend is missing; build scripts/build-bsd-backends.py and install the complete administrator-owned Lurker package."));
    };
    let path = crate::trusted_privileged_executable(&path)?;
    if path.file_name().is_none_or(|name| name != NAME) {
        return Err(AppError::new(
            "Unexpected private BSD VeraCrypt executable identity.",
        ));
    }
    let directory = path.parent().unwrap();
    #[cfg(target_os = "freebsd")]
    for library in ["libfuse.so.2", "libiconv.so.2"] {
        let library = controlled(&directory.join("lib").join(library))?;
        if !library.is_file() {
            return Err(AppError::new("Incomplete BSD private library closure."));
        }
    }
    if directory.join("lib").exists() {
        controlled(&directory.join("lib"))?;
        for entry in fs::read_dir(directory.join("lib"))
            .map_err(|e| AppError::io("Inspect private BSD libraries", e))?
        {
            controlled(
                &entry
                    .map_err(|e| AppError::io("Inspect private BSD library", e))?
                    .path(),
            )?;
        }
    }
    Ok(path)
}

fn invoke(argv: &[OsString], input: Option<&[u8]>) -> AppResult<String> {
    if crate::effective_uid() != 0 {
        return Err(AppError::new(
            "Native BSD VeraCrypt operations require root; implicit backend elevation is disabled.",
        ));
    }
    let path = backend()?;
    let executable = path
        .to_str()
        .ok_or_else(|| AppError::new("Private BSD backend path must be UTF-8."))?;
    exec(executable, argv, input)
}

fn regular_file(source: &Path) -> AppResult<()> {
    let metadata =
        fs::symlink_metadata(source).map_err(|e| AppError::io("Inspect BSD VeraCrypt file", e))?;
    if !metadata.is_file() {
        return Err(AppError::new("BSD VeraCrypt supports regular file containers only, never physical devices or symlinks."));
    }
    Ok(())
}

fn source_name(source: &Path) -> AppResult<&str> {
    let source = source
        .to_str()
        .ok_or_else(|| AppError::new("BSD VeraCrypt container path must be UTF-8."))?;
    if !source.starts_with('/') || source.contains(['\r', '\n', '\0']) {
        return Err(AppError::new(
            "BSD VeraCrypt requires an absolute container path without control characters.",
        ));
    }
    Ok(source)
}

fn authentication(credential: &Credential) -> AppResult<(OsString, Vec<u8>)> {
    match credential {
        Credential::Password(password) => {
            let mut line = password.clone();
            line.push(b'\n');
            Ok((OsString::new(), line))
        }
        Credential::Keyfile(path) => Ok((
            crate::credentials::veracrypt_keyfiles(Some(path))?,
            vec![b'\n'],
        )),
    }
}

pub(super) fn format(
    source: &Path,
    size_mib: u64,
    cipher: CreateCipher,
    credential: &Credential,
) -> AppResult<()> {
    source_name(source)?;
    regular_file(source)?;
    if attached(source)? {
        return Err(AppError::new(
            "Refusing to overwrite an attached VeraCrypt container.",
        ));
    }
    let keyfile = match credential {
        Credential::Keyfile(path) => Some(path.as_path()),
        _ => None,
    };
    let mut argv = crate::workflow::file_veracrypt_create_args(cipher, size_mib, keyfile)?;
    argv.push(source.as_os_str().to_owned());
    let (_, input) = authentication(credential)?;
    invoke(&argv, Some(&input))?;
    Ok(())
}

fn listing(source: &Path) -> AppResult<String> {
    source_name(source)?;
    let mut argv = args(&["--text", "--list", "--verbose"]);
    argv.push(source.as_os_str().to_owned());
    invoke(&argv, None)
}

fn not_mounted(error: &AppError) -> bool {
    [
        "Error: No volumes mounted.",
        "Error: No such volume is mounted.",
    ]
    .iter()
    .any(|message| error.message.ends_with(message))
}

pub(super) fn attached(source: &Path) -> AppResult<bool> {
    match listing(source) {
        Ok(information) => {
            let mapped = listed_device(&information, source)?;
            verify_backing(&information, &mapped)?;
            Ok(true)
        }
        Err(error) if not_mounted(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

fn listed_device(text: &str, source: &Path) -> AppResult<String> {
    let expected = source_name(source)?;
    let volumes: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Volume: "))
        .collect();
    let devices: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Virtual Device: "))
        .collect();
    if volumes.as_slice() != [expected] || devices.len() != 1 {
        return Err(AppError::new(
            "Private VeraCrypt listing does not identify exactly the recorded source.",
        ));
    }
    let device = devices[0];
    #[cfg(target_os = "freebsd")]
    {
        let unit = device.strip_prefix("/dev/md").unwrap_or("");
        if unit.is_empty() || !unit.bytes().all(|c| c.is_ascii_digit()) {
            return Err(AppError::new("Unexpected BSD VeraCrypt virtual device."));
        }
        Ok(device.to_owned())
    }
    #[cfg(target_os = "openbsd")]
    {
        let name = device.strip_prefix("/dev/").unwrap_or("");
        let unit = name
            .strip_prefix("vnd")
            .and_then(|s| s.strip_suffix('c'))
            .unwrap_or("");
        if unit.is_empty() || !unit.bytes().all(|c| c.is_ascii_digit()) {
            return Err(AppError::new("Unexpected BSD VeraCrypt virtual device."));
        }
        Ok(format!("/dev/vnd{unit}a"))
    }
}

fn verify_backing(text: &str, mapped: &str) -> AppResult<()> {
    let paths: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Lurker Auxiliary Directory: "))
        .collect();
    if paths.len() != 1 {
        return Err(AppError::new("The BSD private backend lacks the auxiliary identity contract; install the matching Lurker package."));
    }
    let auxiliary = Path::new(paths[0]);
    let number = auxiliary
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix(".veracrypt_aux_mnt"))
        .unwrap_or("");
    if auxiliary.parent() != Some(Path::new("/tmp"))
        || number.is_empty()
        || !number.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(AppError::new(
            "Unexpected VeraCrypt auxiliary mount identity.",
        ));
    }
    let image = auxiliary.join("volume");
    for (path, directory) in [(auxiliary, true), (image.as_path(), false)] {
        let metadata = fs::symlink_metadata(path)
            .map_err(|e| AppError::io("Inspect VeraCrypt auxiliary identity", e))?;
        if metadata.uid() != 0
            || metadata.mode() & 0o077 != 0
            || if directory {
                !metadata.is_dir()
            } else {
                !metadata.is_file()
            }
        {
            return Err(AppError::new(
                "VeraCrypt auxiliary resources are not private, root-owned native objects.",
            ));
        }
    }
    if !super::mounts()?.iter().any(|mount| {
        mount.target == auxiliary && matches!(mount.filesystem.as_str(), "fuse" | "fusefs")
    }) {
        return Err(AppError::new(
            "The VeraCrypt auxiliary FUSE mount is absent or replaced.",
        ));
    }
    #[cfg(target_os = "freebsd")]
    let backing = mapped.to_owned();
    #[cfg(target_os = "openbsd")]
    let backing = format!(
        "{}c",
        mapped
            .strip_suffix('a')
            .ok_or_else(|| AppError::new("Invalid OpenBSD VeraCrypt partition."))?
    );
    super::native::verify_backing(&backing, &image)
}

pub(super) fn unlock(source: &Path, readonly: bool, credential: &Credential) -> AppResult<String> {
    source_name(source)?;
    regular_file(source)?;
    #[cfg(target_os = "openbsd")]
    if readonly {
        return Err(AppError::new("OpenBSD VeraCrypt read-only crypto attachment has not been qualified; refusing a writable fallback."));
    }
    match listing(source) {
        Ok(_) => {
            return Err(AppError::new(
                "VeraCrypt container is already attached; refusing a duplicate mount.",
            ))
        }
        Err(error) if not_mounted(&error) => {}
        Err(error) => return Err(error),
    }
    let (keyfile, input) = authentication(credential)?;
    let mut argv = args(&[
        "--text",
        "--mount",
        "--non-interactive",
        "--stdin",
        "--pim=0",
        "--filesystem=none",
        "--protect-hidden=no",
        "--keyfiles",
    ]);
    argv.push(keyfile);
    if readonly {
        argv.push("--mount-options=ro".into());
    }
    argv.push(source.as_os_str().to_owned());
    invoke(&argv, Some(&input))?;
    // If discovery fails, do not blindly close: the shared state remains
    // recoverable and source-specific inspection can reconcile the backend.
    let information = listing(source)?;
    let mapped = listed_device(&information, source)?;
    verify_backing(&information, &mapped)?;
    Ok(mapped)
}

pub(super) fn verify_mapping(source: &Path, mapped: &str) -> AppResult<()> {
    let information = listing(source)?;
    if listed_device(&information, source)? != mapped {
        return Err(AppError::new(
            "The VeraCrypt virtual device changed; refusing cleanup.",
        ));
    }
    verify_backing(&information, mapped)
}

pub(super) fn make_filesystem(mapped: &str) -> AppResult<()> {
    #[cfg(target_os = "freebsd")]
    {
        let unit = mapped.strip_prefix("/dev/md").unwrap_or("");
        if unit.is_empty() || !unit.bytes().all(|c| c.is_ascii_digit()) {
            return Err(AppError::new(
                "Refusing to format a non-VeraCrypt md device.",
            ));
        }
        exec("/sbin/newfs", &args(&["-U", mapped]), None)?;
    }
    #[cfg(target_os = "openbsd")]
    {
        let unit = mapped
            .strip_prefix("/dev/vnd")
            .and_then(|s| s.strip_suffix('a'))
            .unwrap_or("");
        if unit.is_empty() || !unit.bytes().all(|c| c.is_ascii_digit()) {
            return Err(AppError::new(
                "Refusing to format a non-VeraCrypt vnode device.",
            ));
        }
        let name = format!("vnd{unit}");
        let mut disk = OpenOptions::new()
            .write(true)
            .open(format!("/dev/r{name}c"))
            .map_err(|e| AppError::io("Open newly created VeraCrypt virtual disk", e))?;
        disk.write_all(&vec![0u8; 1024 * 1024])
            .map_err(|e| AppError::io("Initialize new VeraCrypt disk label area", e))?;
        disk.sync_all()
            .map_err(|e| AppError::io("Sync new VeraCrypt disk label area", e))?;
        drop(disk);
        exec("/sbin/disklabel", &args(&["-wAT-", &name]), Some(b"/ *\n"))?;
        exec(
            "/sbin/newfs",
            &args(&["-q", "-O", "2", &format!("/dev/r{name}a")]),
            None,
        )?;
    }
    Ok(())
}

pub(super) fn close(source: &Path) -> AppResult<()> {
    // The shared lifecycle must verify the mapping and drain/unmount its
    // filesystem first. Upstream NoFilesystem does not track OpenBSD's inner
    // partition mount; never ask its broad --dismount-all or force options.
    source_name(source)?;
    let information = listing(source)?;
    let mapped = listed_device(&information, source)?;
    verify_backing(&information, &mapped)?;
    let mut argv = args(&["--text", "--non-interactive", "--dismount"]);
    argv.push(source.as_os_str().to_owned());
    invoke(&argv, None)?;
    match listing(source) {
        Err(error) if not_mounted(&error) => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(AppError::new(
            "VeraCrypt close did not remove its recorded attachment; retain recovery state.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn source_scoped_listing_rejects_ambiguous_or_different_attachments() {
        let source = Path::new("/tmp/vault with spaces.hc");
        #[cfg(target_os = "freebsd")]
        let device = "/dev/md42";
        #[cfg(target_os = "openbsd")]
        let device = "/dev/vnd2c";
        let listing = format!(
            "Slot: 1\nVolume: {}\nVirtual Device: {device}\n",
            source.display()
        );
        assert!(listed_device(&listing, source).is_ok());
        assert!(listed_device(&listing.replace("spaces", "other"), source).is_err());
        assert!(listed_device(&(listing.clone() + &listing), source).is_err());
        assert!(listed_device(&listing.replace(device, "/dev/sd0a"), source).is_err());
        assert!(source_name(Path::new("/tmp/name\nVolume: /dev/sd0a")).is_err());
        assert!(verify_backing("", device).is_err());
        for path in [
            "/tmp/not-veracrypt",
            "/tmp/.veracrypt_aux_mnt1/../other",
            "/root/.veracrypt_aux_mnt1",
        ] {
            assert!(
                verify_backing(&format!("Lurker Auxiliary Directory: {path}\n"), device).is_err()
            );
        }
    }
    #[test]
    fn password_transport_and_private_candidates_do_not_use_path_lookup() {
        let (keyfile, password) =
            authentication(&Credential::Password(b"  password  ".to_vec())).unwrap();
        assert!(keyfile.is_empty());
        assert_eq!(password, b"  password  \n");
        let choices = candidates(Path::new("/opt/lurker/bin/lurker"));
        assert_eq!(
            choices[0],
            Path::new("/opt/lurker/bin/../libexec/lurker/backends").join(NAME)
        );
        assert!(choices
            .iter()
            .all(|p| p.is_absolute() && p.file_name().unwrap() == NAME));
        assert!(not_mounted(&AppError::new(
            "backend failed: Error: No such volume is mounted."
        )));
        assert!(!not_mounted(&AppError::new(
            "Error: No such volume is mounted.\nUnexpected failure"
        )));
    }

    #[test]
    #[ignore = "root on a designated BSD guest with the private backend installed; creates /tmp file containers"]
    fn native_private_veracrypt_lifecycle() {
        assert_eq!(
            crate::effective_uid(),
            0,
            "native qualification requires root"
        );
        backend().unwrap();
        let root = std::env::temp_dir().join(format!("lurker-bsd-vc-rust-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        eprintln!("native private VeraCrypt fixture: {}", root.display());
        let key = root.join("binary, key.bin");
        let mut key_file = test_file(&key);
        key_file
            .write_all(&[0, 255, 10, 13, 128, 66, 99, 201])
            .unwrap();
        key_file.sync_all().unwrap();
        drop(key_file);
        for cipher in [
            CreateCipher::Aes,
            CreateCipher::Serpent,
            CreateCipher::Twofish,
        ] {
            for keyfile in [false, true] {
                let source = root.join(format!("Vault 空間 {cipher}-{keyfile}.hc"));
                let file = test_file(&source);
                file.set_len(96 * 1024 * 1024).unwrap();
                let credential = if keyfile {
                    Credential::Keyfile(key.clone())
                } else {
                    Credential::Password("  Lurker 密碼 testing only  ".as_bytes().to_vec())
                };
                format(&source, 96, cipher, &credential).unwrap();
                let mapped = unlock(&source, false, &credential).unwrap();
                verify_mapping(&source, &mapped).unwrap();
                make_filesystem(&mapped).unwrap();
                let target = root.join("mount");
                fs::create_dir(&target).unwrap();
                let filesystem = if cfg!(target_os = "freebsd") {
                    "ufs"
                } else {
                    "ffs"
                };
                exec(
                    "/sbin/mount",
                    &[
                        "-t".into(),
                        filesystem.into(),
                        mapped.into(),
                        target.as_os_str().into(),
                    ],
                    None,
                )
                .unwrap();
                let marker = target.join("durable marker");
                let mut data = test_file(&marker);
                data.write_all(b"durable private backend marker").unwrap();
                data.sync_all().unwrap();
                drop(data);
                exec("/sbin/umount", &[target.as_os_str().into()], None).unwrap();
                close(&source).unwrap();
                assert!(unlock(
                    &source,
                    false,
                    &Credential::Password(b"wrong password".to_vec())
                )
                .is_err());
                let mapped = unlock(&source, false, &credential).unwrap();
                verify_mapping(&source, &mapped).unwrap();
                exec(
                    "/sbin/mount",
                    &[
                        "-t".into(),
                        filesystem.into(),
                        mapped.into(),
                        target.as_os_str().into(),
                    ],
                    None,
                )
                .unwrap();
                assert_eq!(
                    fs::read(&marker).unwrap(),
                    b"durable private backend marker"
                );
                exec("/sbin/umount", &[target.as_os_str().into()], None).unwrap();
                close(&source).unwrap();
                #[cfg(target_os = "freebsd")]
                {
                    let mapped = unlock(&source, true, &credential).unwrap();
                    verify_mapping(&source, &mapped).unwrap();
                    exec(
                        "/sbin/mount",
                        &[
                            "-t".into(),
                            filesystem.into(),
                            "-o".into(),
                            "ro".into(),
                            mapped.into(),
                            target.as_os_str().into(),
                        ],
                        None,
                    )
                    .unwrap();
                    assert!(fs::write(target.join("must-fail"), b"forbidden").is_err());
                    exec("/sbin/umount", &[target.as_os_str().into()], None).unwrap();
                    close(&source).unwrap();
                }
                fs::remove_dir(target).unwrap();
                fs::remove_file(source).unwrap();
            }
        }
        fs::remove_file(key).unwrap();
        fs::remove_dir(root).unwrap();
    }

    fn test_file(path: &Path) -> std::fs::File {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .unwrap()
    }
}
