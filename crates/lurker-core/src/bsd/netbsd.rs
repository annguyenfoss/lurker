//! NetBSD CGD adapter. Parameters are mandatory recovery metadata, not a keyfile.
//!
//! Only the generated AES-XTS/Argon2id profile is accepted. In particular, an
//! arbitrary cgdconfig file may contain shell_cmd and must never be passed to an
//! elevated cgdconfig. Device allocation uses the kernel's exclusive configure
//! operation and never clears a device it did not successfully configure.
use super::{args, exec, Credential, Registry, Session};
use crate::{AppError, AppResult, CreateCipher, CreateFilesystem};
mod filesystems;
pub(super) use filesystems::{
    close_filesystem, filesystem_has_posix_ownership, make_selected_filesystem,
    matches_filesystem_mount, mount_filesystem, validate_filesystem,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const CGDCONFIG: &str = "/sbin/cgdconfig";
const VNDCONFIG: &str = "/usr/sbin/vndconfig";

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DetachIntent {
    Cgd,
    Vnd,
}

pub(super) fn attach_file(path: &Path, readonly: bool) -> AppResult<String> {
    let mut last_error = None;
    for n in 0..4 {
        // Full raw device paths avoid opendisk's current-directory lookup.
        let raw = format!("/dev/rvnd{n}d");
        let mut argv = args(&["-c"]);
        if readonly {
            argv.push("-r".into());
        }
        argv.extend([raw.into(), path.as_os_str().into()]);
        match exec(VNDCONFIG, &argv, None) {
            Ok(_) => return Ok(format!("/dev/vnd{n}d")),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::new("No free NetBSD vnode device.")))
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    let n = device_number(device, "vnd")?;
    exec(VNDCONFIG, &args(&["-u", &format!("/dev/rvnd{n}d")]), None)?;
    Ok(())
}

/// Generate and sync recovery metadata only; do not allocate a CGD mapping.
/// CGD has no embedded header: the caller must publish params with the image.
/// The shared lifecycle calls unlock_initial, journals the returned mapping,
/// then initializes its filesystem so interruption leaves a recoverable session.
pub(super) fn format(
    device: &str,
    cipher: CreateCipher,
    credential: &Credential,
    params: &Path,
    filesystem: Option<CreateFilesystem>,
) -> AppResult<()> {
    if cipher != CreateCipher::Aes {
        return Err(AppError::new("CGD creation supports AES-XTS only."));
    }
    device_number(device, "vnd")?;
    password_input(credential, false)?;
    let generated = exec(
        CGDCONFIG,
        &args(&["-g", "-k", "argon2id", "-V", "ffs", "aes-xts", "512"]),
        None,
    )?;
    // Salt generation stays in the native tool; KDF costs are fixed policy.
    let normalized = normalize_generated(&generated)?;
    let normalized = if matches!(
        filesystem,
        None | Some(CreateFilesystem::Ufs1 | CreateFilesystem::Ufs2)
    ) {
        normalized
    } else {
        normalized.replace("verify_method ffs;", "verify_method disklabel;")
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(params)
        .map_err(|e| AppError::io("Failed to create CGD recovery parameters", e))?;
    file.write_all(normalized.as_bytes())
        .map_err(|e| AppError::io("Failed to save CGD parameters", e))?;
    file.sync_all()
        .map_err(|e| AppError::io("Failed to sync CGD parameters", e))
}

/// First attach of a new image uses re-entry verification because its FFS
/// superblocks do not exist yet. The caller must journal this mapping before
/// running newfs, and close it through the normal verified cleanup path.
pub(super) fn unlock_initial(
    device: &str,
    credential: &Credential,
    params: &Path,
) -> AppResult<String> {
    device_number(device, "vnd")?;
    let input = password_input(credential, true)?;
    configure(device, &input, params, true)
}

pub(super) fn unlock(
    device: &str,
    _readonly: bool,
    credential: &Credential,
    params: &Path,
) -> AppResult<String> {
    device_number(device, "vnd")?;
    let input = password_input(credential, false)?;
    configure(device, &input, params, false)
}

fn configure(device: &str, input: &[u8], params: &Path, initial: bool) -> AppResult<String> {
    // Snapshot exactly the validated bytes to a private, owned directory. An
    // attacker replacing an external params file cannot insert a shell_cmd
    // between our validation and the elevated native tool's second open.
    let snapshot = ParametersSnapshot::new(params)?;
    let listing = exec(CGDCONFIG, &args(&["-lv"]), None)?;
    let name = free_cgd(&listing).ok_or_else(|| AppError::new("No free NetBSD CGD device."))?;
    let raw = format!("/dev/r{name}d");
    let mut argv = args(&["-p"]);
    if initial {
        argv.extend(args(&["-V", "re-enter"]));
    }
    argv.extend([raw.into(), device.into(), snapshot.path.as_os_str().into()]);
    // A concurrently claimed CGD fails atomically. Never unconfigure on error:
    // that could destroy another process's mapping.
    exec(CGDCONFIG, &argv, Some(input))?;
    Ok(format!("/dev/{name}d"))
}

pub(super) fn close(device: &str) -> AppResult<()> {
    let n = device_number(device, "cgd")?;
    exec(CGDCONFIG, &args(&["-u", &format!("/dev/rcgd{n}d")]), None)?;
    Ok(())
}

pub(super) fn make_filesystem(device: &str) -> AppResult<()> {
    let n = device_number(device, "cgd")?;
    // -I permits a filesystem spanning the whole raw device without a disklabel;
    // it does not disable kernel bounds or force a busy mapping closed.
    exec(
        "/sbin/newfs",
        &args(&["-I", "-O", "2", &format!("/dev/rcgd{n}d")]),
        None,
    )?;
    Ok(())
}

// NetBSD <dev/vndvar.h>: struct vnd_user and VNDIOCGET. Kernel returns the
// backing vnode's va_fsid/va_fileid, which match stat's st_dev/st_ino even on
// tmpfs or a FUSE filesystem without a corresponding /dev mount-source node.
// This is an ABI binding only; encryption remains in the native CGD stack.
#[repr(C)]
struct VndUser {
    unit: libc::c_int,
    device: libc::dev_t,
    inode: libc::ino_t,
}

// <sys/ioccom.h> _IOWR('F', 3, struct vnd_user).
const VNDIOCGET: libc::c_ulong = 0xc000_0000
    | ((std::mem::size_of::<VndUser>() as libc::c_ulong & 0x1fff) << 16)
    | ((b'F' as libc::c_ulong) << 8)
    | 3;

fn backing_identity(n: u32) -> AppResult<(u64, u64)> {
    let raw = format!("/dev/rvnd{n}d");
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&raw)
        .map_err(|e| AppError::io("Failed to open NetBSD vnode for identity check", e))?;
    let unit = libc::c_int::try_from(n).map_err(|_| AppError::new("Invalid NetBSD vnode unit."))?;
    let mut info = VndUser {
        unit,
        device: 0,
        inode: 0,
    };
    // SAFETY: VNDIOCGET is the native read/write ioctl for exactly VndUser's
    // repr(C) layout; file is a live raw vnode descriptor, and info is fully
    // initialized and valid for the whole synchronous kernel call.
    if unsafe { libc::ioctl(file.as_raw_fd(), VNDIOCGET, &mut info) } == -1 {
        return Err(AppError::io(
            "Failed to query NetBSD vnode backing identity",
            std::io::Error::last_os_error(),
        ));
    }
    if info.unit != unit || (info.device == 0 && info.inode == 0) {
        return Err(AppError::new(
            "NetBSD vnode is unconfigured or its unit changed.",
        ));
    }
    Ok((info.device as u64, info.inode as u64))
}

pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let n = device_number(device, "vnd")?;
    let identity =
        fs::metadata(path).map_err(|e| AppError::io("Failed to inspect CGD image", e))?;
    if backing_identity(n)? != (identity.dev(), identity.ino()) {
        return Err(AppError::new(
            "NetBSD vnode backing identity changed; refusing detach.",
        ));
    }
    Ok(())
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    let n = device_number(mapped, "cgd")?;
    device_number(backing, "vnd")?;
    let listing = exec(CGDCONFIG, &args(&["-lv", &format!("/dev/rcgd{n}d")]), None)?;
    if !listing
        .split_whitespace()
        .any(|part| part.trim_end_matches(',') == backing.trim_start_matches("/dev/"))
    {
        return Err(AppError::new(
            "CGD backing mapping changed; refusing close.",
        ));
    }
    Ok(())
}

pub(super) fn reconcile_detach(registry: &Registry, session: &mut Session) -> AppResult<()> {
    let Some(intent) = session.netbsd_detach else {
        return Ok(());
    };
    let conflict = || {
        AppError::new("NetBSD detach intent conflicts with its session; preserving the journal.")
    };
    if session.mounted.is_some() || !session.identity.matches(&session.source) {
        return Err(conflict());
    }
    let backing = session.backing.as_deref().ok_or_else(conflict)?;
    match intent {
        DetachIntent::Cgd => {
            let mapped = session.mapped.as_deref().ok_or_else(conflict)?;
            verify_backing(backing, &session.source)?;
            let n = device_number(mapped, "cgd")?;
            let raw = format!("/dev/rcgd{n}d");
            let listing = exec(CGDCONFIG, &args(&["-lv", &raw]), None)?;
            if listing.trim() != format!("{raw}: not in use") {
                return Ok(());
            }
            if super::mounts()?
                .iter()
                .any(|mount| mount.source == Path::new(mapped))
            {
                return Err(conflict());
            }
            session.mapped = None;
        }
        DetachIntent::Vnd => {
            if session.mapped.is_some() {
                return Err(conflict());
            }
            let n = device_number(backing, "vnd")?;
            let listing = exec(VNDCONFIG, &args(&["-l", &format!("/dev/rvnd{n}d")]), None)?;
            if listing.trim() != format!("vnd{n}: not in use") {
                return Ok(());
            }
            session.backing = None;
        }
    }
    session.netbsd_detach = None;
    registry.save(session)
}

struct ParametersSnapshot {
    directory: PathBuf,
    path: PathBuf,
}
impl ParametersSnapshot {
    fn new(source: &Path) -> AppResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(source)
            .map_err(|e| AppError::io("Failed to open CGD parameters", e))?;
        let metadata = file
            .metadata()
            .map_err(|e| AppError::io("Failed to inspect CGD parameters", e))?;
        if !metadata.is_file() || metadata.len() > 4096 {
            return Err(AppError::new(
                "CGD parameters must be a small regular file.",
            ));
        }
        let mut value = String::new();
        file.take(4097)
            .read_to_string(&mut value)
            .map_err(|e| AppError::io("Failed to read CGD parameters", e))?;
        if value.len() > 4096 {
            return Err(AppError::new("CGD parameters exceed the size limit."));
        }
        validate_params(&value)?;
        // Reconstruct syntax rather than passing arbitrary punctuation accepted
        // by our tokenizer through to another parser.
        let value = normalize_generated(&value)?;
        let mut random = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|e| AppError::io("Failed to create private CGD parameter snapshot", e))?;
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let directory = PathBuf::from(format!(
            "/tmp/lurker-cgd-params-{}-{suffix}",
            std::process::id()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| AppError::io("Failed to create private CGD parameter directory", e))?;
        let snapshot = Self {
            path: directory.join("params"),
            directory,
        };
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&snapshot.path)
            .map_err(|e| AppError::io("Failed to create CGD parameter snapshot", e))?;
        out.write_all(value.as_bytes())
            .map_err(|e| AppError::io("Failed to write CGD parameter snapshot", e))?;
        Ok(snapshot)
    }
}
impl Drop for ParametersSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

fn device_number(device: &str, kind: &str) -> AppResult<u32> {
    let prefix = format!("/dev/{kind}");
    let digits = device
        .strip_prefix(&prefix)
        .and_then(|s| s.strip_suffix('d'))
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| AppError::new("Invalid NetBSD device identity."))?;
    digits
        .parse()
        .map_err(|_| AppError::new("Invalid NetBSD device number."))
}

fn password_input(credential: &Credential, repeat: bool) -> AppResult<Vec<u8>> {
    let Credential::Password(value) = credential else {
        return Err(AppError::new("CGD currently accepts a password only; raw-key and shell-command key generators are excluded."));
    };
    if value.is_empty() || value.len() > 1023 || value.iter().any(|b| matches!(b, 0 | 10 | 13)) {
        return Err(AppError::new(
            "CGD password must contain 1–1023 bytes without NUL/newlines.",
        ));
    }
    let mut input = value.clone();
    input.push(b'\n');
    if repeat {
        input.extend(value);
        input.push(b'\n');
    }
    Ok(input)
}

fn free_cgd(listing: &str) -> Option<String> {
    listing
        .lines()
        .filter_map(|line| line.strip_suffix(": not in use"))
        .filter(|name| {
            name.strip_prefix("cgd")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .min()
        .map(String::from)
}

fn normalize_generated(value: &str) -> AppResult<String> {
    let tokens = tokens(value);
    if tokens.len() != 20
        || tokens[0] != "algorithm"
        || tokens[1] != "aes-xts"
        || tokens[8] != "keygen"
        || tokens[9] != "argon2id"
        || tokens[18] != "salt"
    {
        return Err(AppError::new("Unexpected native CGD parameter output."));
    }
    let verifier = match tokens.get(7).copied() {
        Some("ffs") => "ffs",
        Some("disklabel") => "disklabel",
        _ => return Err(AppError::new("Unsupported CGD verification method.")),
    };
    let result = format!("algorithm aes-xts;\niv-method encblkno1;\nkeylength 512;\nverify_method {verifier};\nkeygen argon2id {{\n iterations 3;\n memory 262144;\n parallelism 2;\n version 19;\n salt {};\n}};\n", tokens[19]);
    validate_params(&result)?;
    Ok(result)
}

fn tokens(value: &str) -> Vec<&str> {
    value
        .split(|c: char| c.is_ascii_whitespace() || matches!(c, '{' | '}' | ';'))
        .filter(|s| !s.is_empty())
        .collect()
}

fn validate_params(value: &str) -> AppResult<()> {
    let parts = tokens(value);
    let verifier = match parts.get(7).copied() {
        Some("ffs") => "ffs",
        Some("disklabel") => "disklabel",
        _ => return Err(AppError::new("Unsupported CGD verification method.")),
    };
    let expected = [
        "algorithm",
        "aes-xts",
        "iv-method",
        "encblkno1",
        "keylength",
        "512",
        "verify_method",
        verifier,
        "keygen",
        "argon2id",
        "iterations",
        "3",
        "memory",
        "262144",
        "parallelism",
        "2",
        "version",
        "19",
        "salt",
    ];
    if parts.len() != 20
        || parts[..19] != expected
        || parts[19].len() != 28
        || !parts[19].starts_with("AAAAg")
        || !parts[19].ends_with('=')
        || !parts[19]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
    {
        return Err(AppError::new(
            "CGD parameters do not match Lurker's safe AES-XTS/Argon2id profile.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires root and real disposable NetBSD CGD and vnode attachments"]
    fn cleanup_reconciles_completed_native_detaches_with_saved_intent() {
        use super::super::{cleanup, require_root, unique_id, Identity};

        require_root().unwrap();
        let work = std::env::temp_dir().join(format!("lurker-netbsd-detach-{}", unique_id()));
        fs::DirBuilder::new().mode(0o700).create(&work).unwrap();
        let source = work.join("owned.img");
        let file = File::create(&source).unwrap();
        file.set_len(128 * 1024 * 1024).unwrap();
        let registry = Registry {
            directory: work.clone(),
            _lock: File::create(work.join("lock")).unwrap(),
        };
        let backing = attach_file(&source, false).unwrap();
        let params = work.join("owned.img.cgdparams");
        let credential = Credential::Password(b"disposable NetBSD detach fixture".to_vec());
        format(&backing, CreateCipher::Aes, &credential, &params, None).unwrap();
        let mapped = unlock_initial(&backing, &credential, &params).unwrap();
        let mut session = Session {
            version: 1,
            id: unique_id(),
            backend: crate::VolumeType::Cgd,
            identity: Identity::read(&source).unwrap(),
            source: source.clone(),
            create_target: None,
            peer_sources: Vec::new(),
            backing: Some(backing.clone()),
            netbsd_detach: Some(DetachIntent::Cgd),
            mapped: Some(mapped.clone()),
            mounted: None,
            intended_mountpoint: None,
            initialization_directory: None,
            readonly: false,
        };
        registry.save(&session).unwrap();
        reconcile_detach(&registry, &mut session).unwrap();
        assert_eq!(session.mapped.as_deref(), Some(mapped.as_str()));
        verify_mapping(&mapped, &backing).unwrap();
        close(&mapped).unwrap();

        let mut saved = registry.records().unwrap().pop().unwrap();
        let mut without_intent = saved.clone();
        without_intent.netbsd_detach = None;
        assert!(cleanup(&registry, &mut without_intent).is_err());
        reconcile_detach(&registry, &mut saved).unwrap();
        assert!(saved.mapped.is_none() && saved.netbsd_detach.is_none());
        verify_backing(&backing, &source).unwrap();

        saved.netbsd_detach = Some(DetachIntent::Vnd);
        registry.save(&saved).unwrap();
        reconcile_detach(&registry, &mut saved).unwrap();
        assert_eq!(saved.backing.as_deref(), Some(backing.as_str()));
        detach_file(&backing).unwrap();

        let mut saved = registry.records().unwrap().pop().unwrap();
        let mut without_intent = saved.clone();
        without_intent.netbsd_detach = None;
        assert!(cleanup(&registry, &mut without_intent).is_err());
        cleanup(&registry, &mut saved).unwrap();
        assert!(registry.records().unwrap().is_empty());
        assert!(saved.identity.matches(&source));
        assert!(params.is_file());
        fs::remove_dir_all(work).unwrap();
    }
    const PARAMS: &str = "algorithm aes-xts;\niv-method encblkno1;\nkeylength 512;\nverify_method ffs;\nkeygen argon2id {iterations 64; memory 42943; parallelism 2; version 19; salt AAAAgOI5SzGcFVKg64hWP3CRq2E=;};";
    #[test]
    fn native_output_is_normalized_and_executable_keygens_rejected() {
        let params = normalize_generated(PARAMS).unwrap();
        assert!(validate_params(&params).is_ok());
        assert!(validate_params(&format!("{params} keygen shell_cmd {{cmd \"id\";}};")).is_err());
        assert!(validate_params(&params.replace("memory 262144", "memory 1")).is_err());
        assert!(normalize_generated(&PARAMS.replace("argon2id", "shell_cmd")).is_err());
    }
    #[test]
    fn non_ffs_verifier_preserves_only_the_safe_disklabel_profile() {
        let params = normalize_generated(PARAMS)
            .unwrap()
            .replace("verify_method ffs", "verify_method disklabel");
        assert_eq!(normalize_generated(&params).unwrap(), params);
        assert!(validate_params(&params).is_ok());
        for verifier in ["none", "mbr", "re-enter", "shell_cmd"] {
            assert!(validate_params(&params.replace(
                "verify_method disklabel",
                &format!("verify_method {verifier}")
            ))
            .is_err());
        }
    }

    #[test]
    fn allocation_and_device_names_do_not_accept_unrelated_devices() {
        assert_eq!(
            free_cgd("cgd7: not in use\ncgd0: not in use\ncgd1: /dev/vnd0d"),
            Some("cgd0".into())
        );
        assert_eq!(free_cgd("wd0: not in use\ncgd: not in use"), None);
        assert_eq!(device_number("/dev/vnd3d", "vnd").unwrap(), 3);
        for name in ["/dev/wd0d", "/dev/vndd", "vnd0", "/dev/vnd0d/foo"] {
            assert!(device_number(name, "vnd").is_err());
        }
    }
    /// Run as root on a disposable NetBSD test host, never a development host:
    /// cargo test -p lurker-core native_vnode_identity -- --ignored
    #[cfg(target_os = "netbsd")]
    #[test]
    #[ignore = "allocates disposable vnode devices and mounts a private tmpfs"]
    fn native_vnode_identity_works_on_ffs_and_tmpfs() {
        assert_eq!(
            unsafe { libc::geteuid() },
            0,
            "Native vnode identity test needs root"
        );
        let root = PathBuf::from(format!(
            "/tmp/lurker-vnd-identity-test-{}",
            std::process::id()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        fn fixture(root: &Path) -> AppResult<()> {
            let image = root.join("image");
            let alias = root.join("alias");
            File::create(&image)
                .and_then(|f| f.set_len(2 * 1024 * 1024))
                .map_err(|e| AppError::io("Failed to create disposable identity image", e))?;
            fs::hard_link(&image, &alias)
                .map_err(|e| AppError::io("Failed to alias disposable image", e))?;
            let vnode = attach_file(&image, false)?;
            let tested = (|| {
                verify_backing(&vnode, &image)?;
                verify_backing(&vnode, &alias)?;
                fs::rename(&image, root.join("original"))
                    .map_err(|e| AppError::io("Failed to replace disposable image", e))?;
                File::create(&image)
                    .and_then(|f| f.set_len(2 * 1024 * 1024))
                    .map_err(|e| AppError::io("Failed to create replacement image", e))?;
                if verify_backing(&vnode, &image).is_ok() {
                    return Err(AppError::new("Vnode identity accepted a replaced image."));
                }
                verify_backing(&vnode, &alias)
            })();
            // Detach only the unit this fixture successfully acquired. Never
            // force a live mapping closed or remove fixture files on failure.
            detach_file(&vnode)?;
            tested
        }
        fixture(&root).unwrap();
        let ram = root.join("tmpfs");
        fs::DirBuilder::new().mode(0o700).create(&ram).unwrap();
        exec(
            "/sbin/mount_tmpfs",
            &[
                "-s".into(),
                "32M".into(),
                "tmpfs".into(),
                ram.as_os_str().into(),
            ],
            None,
        )
        .unwrap();
        let tested = fixture(&ram);
        // A failed unmount leaves evidence and data in place for inspection.
        exec("/sbin/umount", &[ram.as_os_str().into()], None).unwrap();
        tested.unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn vnode_identity_abi_matches_netbsd_amd64_headers() {
        assert_eq!(std::mem::size_of::<VndUser>(), 24);
        assert_eq!(std::mem::offset_of!(VndUser, device), 8);
        assert_eq!(std::mem::offset_of!(VndUser, inode), 16);
        assert_eq!(VNDIOCGET, 0xc0184603);
    }
    #[test]
    fn password_framing_and_native_limits() {
        assert_eq!(
            password_input(&Credential::Password(b"  test  ".to_vec()), true).unwrap(),
            b"  test  \n  test  \n"
        );
        assert!(password_input(&Credential::Password(vec![b'x'; 1024]), false).is_err());
        assert!(password_input(&Credential::Password(b"x\ny".to_vec()), false).is_err());
    }
}
