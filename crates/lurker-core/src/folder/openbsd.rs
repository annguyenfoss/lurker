//! OpenBSD base FUSE requires root. Keep elevation confined to CryFS and to
//! already-open vault, mountpoint and state directories using native unveil.
use super::*;
use crate::new_filesystem::{self, Owner};
use std::ffi::{CStr, CString};
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
mod runtime;

pub(super) fn validate_backend(path: &Path) -> AppResult<PathBuf> {
    runtime::validate(path)
}

pub(super) fn validate_command(command: &CommandAction) -> AppResult<()> {
    caller()?;
    if command.privilege_mode() != PrivilegeMode::Privileged {
        return Err(AppError::new(
            "OpenBSD CryFS uses the authorized native FUSE route.",
        ));
    }
    match command {
        CommandAction::Create(c) if c.volume_type == VolumeType::Cryfs => Ok(()),
        CommandAction::Mount(c)
            if c.volume_type == VolumeType::Cryfs
                || (c.volume_type == VolumeType::Auto
                    && detect_type(&c.source)? == VolumeType::Cryfs) =>
        {
            Ok(())
        }
        CommandAction::Unmount(c)
            if matches!(c.volume_type, VolumeType::Cryfs | VolumeType::Auto) =>
        {
            Ok(())
        }
        _ => Err(AppError::new(
            "Only OpenBSD CryFS uses privileged directory encryption.",
        )),
    }
}

pub(super) fn caller() -> AppResult<Owner> {
    if crate::effective_uid() != 0 {
        return Err(AppError::new(
            "OpenBSD CryFS needs administrator authorization for native FUSE.",
        ));
    }
    new_filesystem::invoking_owner()
}

pub(super) fn runtime() -> AppResult<PathBuf> {
    Ok(Path::new("/var/run").join(format!("lurker-folders-{}", caller()?.uid)))
}

fn cpath(path: &Path) -> AppResult<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| AppError::new("Directory path contains a NUL byte."))
}

fn fd_error(what: &str) -> AppError {
    AppError::io(what, std::io::Error::last_os_error())
}

fn open_at(parent: &File, name: &CStr, flags: i32, mode: u32) -> AppResult<File> {
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )
    };
    if fd < 0 {
        return Err(fd_error("Open anchored directory entry"));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// Walk each component without following links. An attacker can rename an
/// ancestor, but cannot redirect a later openat through its replacement.
fn directory(path: &Path) -> AppResult<File> {
    if !path.is_absolute() {
        return Err(AppError::new("An absolute directory path is required."));
    }
    let mut fd = File::open("/").map_err(|e| AppError::io("Open filesystem root", e))?;
    for component in path.components() {
        match component {
            std::path::Component::RootDir => (),
            std::path::Component::Normal(name) => {
                let name = CString::new(name.as_bytes())
                    .map_err(|_| AppError::new("Invalid directory component."))?;
                fd = open_at(&fd, &name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
            }
            _ => return Err(AppError::new("Directory paths must be normalized.")),
        }
    }
    Ok(fd)
}

fn identity(file: &File) -> AppResult<Identity> {
    let m = file
        .metadata()
        .map_err(|e| AppError::io("Inspect held directory", e))?;
    Ok(Identity {
        device: m.dev(),
        inode: m.ino(),
    })
}

fn owned_directory(file: &File, path: &Path, owner: Owner) -> AppResult<()> {
    let m = file
        .metadata()
        .map_err(|e| AppError::io("Inspect directory owner", e))?;
    let private_initialization = m.uid() == 0 && path.starts_with(runtime()?);
    if !m.is_dir() || (m.uid() != owner.uid && !private_initialization) || m.mode() & 0o022 != 0 {
        return Err(AppError::new(
            "CryFS directories must be private and owned by you or the administrator.",
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
enum CipherTree {
    File(Vec<u8>),
    Directory(Vec<(String, CipherTree)>),
}
#[derive(Serialize, Deserialize)]
struct PublishRequest {
    tree: Option<CipherTree>,
}

fn load_ciphertext(path: &Path, remaining: &mut usize) -> AppResult<CipherTree> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| AppError::io("Inspect private initialization", e))?;
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .map_err(|e| AppError::io("Read private initialization", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::io("Read private initialization entry", e))?;
        entries.sort_by_key(|entry| entry.file_name() == "cryfs.config");
        let mut children = Vec::new();
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| AppError::new("Invalid ciphertext entry name."))?;
            children.push((name, load_ciphertext(&entry.path(), remaining)?));
        }
        Ok(CipherTree::Directory(children))
    } else if metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= *remaining as u64 {
        let mut data = Vec::new();
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| AppError::io("Open private ciphertext", e))?
            .take(*remaining as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| AppError::io("Read private ciphertext", e))?;
        *remaining = remaining.checked_sub(data.len()).ok_or_else(|| {
            AppError::new("Private initialization exceeds its publication bound.")
        })?;
        Ok(CipherTree::File(data))
    } else {
        Err(AppError::new(
            "Unexpected private initialization link, special file or size; preserve it.",
        ))
    }
}

/// Initialize in root-private storage, then publish as the real invoking UID.
/// A replaced user-controlled parent can never redirect a privileged write.
pub(super) fn create(
    runtime: &Runtime,
    mut command: CreateCommand,
    output: Output,
) -> AppResult<()> {
    let owner = caller()?;
    let (parent, parent_fd, name) = publication_parent(&command.target, owner)?;
    let published = parent.join(std::ffi::OsStr::from_bytes(name.as_bytes()));
    let mut existing = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent_fd.as_raw_fd(),
            name.as_ptr(),
            existing.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        return Err(AppError::new("Vault target must not already exist."));
    }
    if std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
        return Err(fd_error("Inspect new vault target"));
    }
    let private = runtime.path.join(format!("create-{}", nonce()));
    command.target = private.clone();
    super::create_common(runtime, command, Output::silent())?;
    let tree = load_ciphertext(&private, &mut (16 * 1024 * 1024))?;
    let expected = publish(parent_fd, &name, owner, PublishRequest { tree: Some(tree) })?;
    if Identity::read(&published)? != expected {
        return Err(AppError::new(
            "New vault parent was renamed; ciphertext and private initialization are preserved.",
        ));
    }
    fs::remove_dir_all(&private)
        .map_err(|e| AppError::io("Remove published private initialization", e))?;
    output.success(&format!(
        "Created cryfs directory at {}",
        published.display()
    ));
    Ok(())
}

fn publication_parent(path: &Path, owner: Owner) -> AppResult<(PathBuf, File, CString)> {
    let parent = fs::canonicalize(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .map_err(|e| AppError::io("Resolve publication parent", e))?;
    let fd = directory(&parent)?;
    let m = fd
        .metadata()
        .map_err(|e| AppError::io("Inspect publication parent", e))?;
    if m.uid() != owner.uid || m.mode() & 0o022 != 0 || m.mode() & 0o300 != 0o300 {
        return Err(AppError::new(
            "Use a private directory owned by your invoking account.",
        ));
    }
    let name = cpath(Path::new(
        path.file_name()
            .ok_or_else(|| AppError::new("Directory needs a name."))?,
    ))?;
    if name.as_bytes() == b"." || name.as_bytes() == b".." {
        return Err(AppError::new("Invalid directory name."));
    }
    Ok((parent, fd, name))
}

fn publish(
    parent: File,
    name: &CStr,
    owner: Owner,
    request: PublishRequest,
) -> AppResult<Identity> {
    let executable = crate::trusted_privileged_executable(
        &std::env::current_exe().map_err(|e| AppError::io("Locate trusted publisher", e))?,
    )?;
    let fd = parent.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .arg("--lurker-openbsd-folder-publish")
        .arg(fd.to_string())
        .arg(std::ffi::OsStr::from_bytes(name.to_bytes()))
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(parent.as_raw_fd(), libc::F_SETFD, 0) < 0
                || libc::setgroups(0, std::ptr::null()) != 0
                || libc::setresgid(owner.gid, owner.gid, owner.gid) != 0
                || libc::setresuid(owner.uid, owner.uid, owner.uid) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::io("Start invoking-user ciphertext publisher", e))?;
    let data = serde_json::to_vec(&request).map_err(|e| AppError::new(e.to_string()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| AppError::new("Missing publisher pipe."))?
        .write_all(&data)
        .map_err(|e| AppError::io("Send new ciphertext to invoking-user publisher", e))?;
    let started = Instant::now();
    while child
        .try_wait()
        .map_err(|e| AppError::io("Wait for ciphertext publisher", e))?
        .is_none()
    {
        if started.elapsed() > WAIT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::new(
                "Invoking-user publication timed out; private initialization preserved.",
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let result = child
        .wait_with_output()
        .map_err(|e| AppError::io("Read ciphertext publication result", e))?;
    if !result.status.success() || result.stdout.len() > 1024 {
        return Err(AppError::new(
            "Invoking-user publication failed; private initialization preserved.",
        ));
    }
    serde_json::from_slice(&result.stdout)
        .map_err(|_| AppError::new("Invalid ciphertext publication identity."))
}

fn publish_entries(tree: &CipherTree, target: &File) -> AppResult<()> {
    let CipherTree::Directory(entries) = tree else {
        return Err(AppError::new("Publication root must be a directory."));
    };
    for (name, node) in entries {
        if name.is_empty() || name == "." || name == ".." || name.contains('/') {
            return Err(AppError::new("Invalid publication entry."));
        }
        let name = CString::new(name.as_bytes())
            .map_err(|_| AppError::new("Invalid publication name."))?;
        match node {
            CipherTree::Directory(_) => {
                if unsafe { libc::mkdirat(target.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
                    return Err(fd_error("Publish ciphertext directory"));
                }
                let fd = open_at(target, &name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
                publish_entries(node, &fd)?;
            }
            CipherTree::File(data) => {
                let mut output = open_at(
                    target,
                    &name,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                    0o600,
                )?;
                output
                    .write_all(data)
                    .and_then(|_| output.sync_all())
                    .map_err(|e| AppError::io("Publish ciphertext file", e))?;
            }
        }
    }
    target
        .sync_all()
        .map_err(|e| AppError::io("Sync published directory", e))
}

pub(super) fn dispatch_publisher() -> Option<i32> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).and_then(|v| v.to_str()) != Some("--lurker-openbsd-folder-publish") {
        return None;
    }
    let run = || -> AppResult<()> {
        if args.len() != 4 || unsafe { libc::getuid() } != crate::effective_uid() {
            return Err(AppError::new("Invalid publisher invocation."));
        }
        let fd = args[2]
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|fd| *fd >= 3)
            .ok_or_else(|| AppError::new("Invalid publisher descriptor."))?;
        let parent = unsafe { File::from_raw_fd(fd) };
        let metadata = parent
            .metadata()
            .map_err(|e| AppError::io("Inspect publisher parent", e))?;
        if !metadata.is_dir()
            || metadata.uid() != crate::effective_uid()
            || metadata.mode() & 0o022 != 0
        {
            return Err(AppError::new(
                "Publisher parent must belong privately to its actual UID.",
            ));
        }
        let name = cpath(Path::new(&args[3]))?;
        if name.as_bytes().is_empty()
            || name.as_bytes() == b"."
            || name.as_bytes() == b".."
            || name.as_bytes().contains(&b'/')
        {
            return Err(AppError::new("Invalid publisher target."));
        }
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::io("Read publication pipe", e))?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(AppError::new("Publication exceeds its bound."));
        }
        let request: PublishRequest = serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("Invalid publication protocol."))?;
        let made = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0;
        if !made
            && (request.tree.is_some()
                || std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST))
        {
            return Err(fd_error("Publication target must not already exist"));
        }
        let target = open_at(&parent, &name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
        let m = target
            .metadata()
            .map_err(|e| AppError::io("Inspect publication target", e))?;
        if m.uid() != crate::effective_uid() || m.mode() & 0o022 != 0 {
            return Err(AppError::new("Publication target changed owner."));
        }
        if let Some(tree) = &request.tree {
            publish_entries(tree, &target)?;
        }
        serde_json::to_writer(std::io::stdout(), &identity(&target)?)
            .map_err(|e| AppError::new(e.to_string()))?;
        Ok(())
    };
    Some(if run().is_ok() { 0 } else { 1 })
}

pub(super) fn held_mountpoint(path: &Path) -> AppResult<Identity> {
    let file = directory(path)?;
    owned_directory(&file, path, caller()?)?;
    identity(&file)
}

pub(super) fn ensure_mountpoint(path: &Path) -> AppResult<()> {
    let owner = caller()?;
    if path.starts_with(runtime()?) {
        secure_dir(path)?;
        return Ok(());
    }
    let (_, parent, name) = publication_parent(path, owner)?;
    let expected = publish(parent, &name, owner, PublishRequest { tree: None })?;
    if held_mountpoint(path)? != expected {
        return Err(AppError::new(
            "Mountpoint parent changed; preserve existing storage.",
        ));
    }
    Ok(())
}

pub(super) fn initialize_owner(session: &Session) -> AppResult<()> {
    let root = directory(&session.mountpoint)?;
    if Some(identity(&root)?) != session.mounted_directory
        || mount_identity(&session.mountpoint)? != session.mount_identity
    {
        return Err(AppError::new(
            "New plaintext filesystem changed before ownership initialization.",
        ));
    }
    new_filesystem::assign_root(&root, caller()?)
}

fn unveil(path: &CStr, permissions: &CStr) -> std::io::Result<()> {
    if unsafe { libc::unveil(path.as_ptr(), permissions.as_ptr()) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub(super) fn configure_supervisor(command: &mut Command, executable: &Path) -> AppResult<()> {
    let owner = caller()?;
    let executable = validate_backend(executable)?;
    command
        .env_clear()
        .env("PATH", "/sbin:/bin:/usr/sbin:/usr/bin:/usr/local/bin")
        .env("LC_ALL", "C")
        .env("HOME", runtime()?)
        .env("SUDO_UID", owner.uid.to_string())
        .env(
            "LURKER_BACKEND_DIR",
            executable
                .parent()
                .ok_or_else(|| AppError::new("Missing trusted backend directory."))?,
        );
    Ok(())
}

fn engine_state(owner: Owner, initializing: bool) -> AppResult<PathBuf> {
    if initializing {
        let path = runtime()?.join("initial-state");
        secure_dir(&path)?;
        return Ok(path);
    }
    let parent = Path::new("/var/db/lurker-cryfs-state");
    match fs::DirBuilder::new().mode(0o755).create(parent) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(AppError::io("Create native CryFS state parent", e)),
    }
    crate::trust::trusted_runtime_directory(parent)?;
    let parent_fd = directory(parent)?;
    let name = CString::new(owner.uid.to_string()).unwrap();
    let made = unsafe { libc::mkdirat(parent_fd.as_raw_fd(), name.as_ptr(), 0o700) } == 0;
    if !made && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) { return Err(fd_error("Create caller CryFS state")); }
    let held = open_at(&parent_fd, &name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    if made { new_filesystem::assign_root(&held, owner)?; }
    let metadata = held.metadata().map_err(|e| AppError::io("Inspect caller CryFS state", e))?;
    if metadata.uid() != owner.uid || metadata.gid() != owner.gid || metadata.mode() & 0o077 != 0 { return Err(AppError::new("Existing CryFS state must remain private and owned by its caller.")); }
    Ok(parent.join(owner.uid.to_string()))
}

/// Pass validated held directories to the native engine. It applies and locks
/// unveil inside main, after exec has reset any inherited unveil policy.
pub(super) fn confine(
    command: &mut Command,
    _executable: &Path,
    session: &Session,
    initializing: bool,
) -> AppResult<()> {
    let source = directory(&session.source)?;
    let mountpoint = directory(&session.mountpoint)?;
    let owner = caller()?;
    owned_directory(&source, &session.source, owner)?;
    owned_directory(&mountpoint, &session.mountpoint, owner)?;
    if identity(&source)? != session.source_identity
        || Some(identity(&mountpoint)?) != session.openbsd_underlay
    {
        return Err(AppError::new(
            "Vault or empty mountpoint changed before confined launch.",
        ));
    }
    let engine_owner = if initializing { Owner { uid: 0, gid: 0 } } else { owner };
    let state_path = engine_state(engine_owner, initializing)?;
    let state = directory(&state_path)?;
    command
        .env_clear()
        .env("PATH", "/sbin:/bin:/usr/sbin:/usr/bin:/usr/local/bin")
        .env("LC_ALL", "C")
        .env("HOME", runtime()?)
        .env("CRYFS_FRONTEND", "noninteractive")
        .env("CRYFS_NO_UPDATE_CHECK", "1")
        .env("CRYFS_LOCAL_STATE_DIR", state_path)
        .env("LURKER_CRYFS_UNVEIL_PROTOCOL", "1")
        .env("LURKER_CRYFS_CALLER_UID", engine_owner.uid.to_string())
        .env("LURKER_CRYFS_CALLER_GID", engine_owner.gid.to_string())
        .env("LURKER_CRYFS_SOURCE_FD", source.as_raw_fd().to_string())
        .env("LURKER_CRYFS_MOUNT_FD", mountpoint.as_raw_fd().to_string())
        .env("LURKER_CRYFS_STATE_FD", state.as_raw_fd().to_string());
    unsafe {
        command.pre_exec(move || {
            for directory in [&source, &mountpoint, &state] {
                if libc::fcntl(directory.as_raw_fd(), libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(())
}

/// Resolve relative to a held parent, so a renamed ancestor cannot redirect
/// teardown. A mounted leaf cannot be renamed by an unprivileged caller.
pub(super) fn unmount(path: &Path, expected: &str) -> AppResult<std::process::Output> {
    let value: serde_json::Value = serde_json::from_str(
        expected
            .strip_prefix("openbsd-fsid:")
            .ok_or_else(|| AppError::new("Missing native folder mount identity."))?,
    )
    .map_err(|_| AppError::new("Invalid native folder mount identity."))?;
    let fsid: Vec<u8> = value["fsid"]
        .as_array()
        .ok_or_else(|| AppError::new("Missing native fsid."))?
        .iter()
        .map(|n| {
            n.as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| AppError::new("Invalid native fsid."))
        })
        .collect::<AppResult<_>>()?;
    if fsid.len() != std::mem::size_of::<libc::fsid_t>() {
        return Err(AppError::new("Unexpected native fsid size."));
    }
    let parent = directory(
        path.parent()
            .ok_or_else(|| AppError::new("Invalid mountpoint parent."))?,
    )?;
    let name = cpath(Path::new(
        path.file_name()
            .ok_or_else(|| AppError::new("Invalid mountpoint name."))?,
    ))?;
    let executable = crate::trusted_system_executable("true")?;
    let mut command = Command::new(executable);
    command
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(parent.as_raw_fd()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let fd = libc::open(
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            );
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
            let queried = libc::fstatfs(fd, info.as_mut_ptr());
            let error = std::io::Error::last_os_error();
            libc::close(fd);
            if queried != 0 {
                return Err(error);
            }
            let pointer = std::ptr::addr_of!((*info.as_ptr()).f_fsid);
            let observed = std::slice::from_raw_parts(
                pointer.cast::<u8>(),
                std::mem::size_of::<libc::fsid_t>(),
            );
            if observed != fsid {
                return Err(std::io::Error::other(
                    "Native folder fsid changed; preserve its mount.",
                ));
            }
            if libc::unmount(name.as_ptr(), 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.output().map_err(|e| {
        AppError::io(
            "Normal anchored native folder unmount failed; session preserved",
            e,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publication_descriptors_confine_renamed_parents_and_reject_links() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let moved = root.path().join("moved");
        let outside = root.path().join("outside");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&outside).unwrap();
        let held = directory(&original).unwrap();
        fs::rename(&original, &moved).unwrap();
        std::os::unix::fs::symlink(&outside, &original).unwrap();
        let tree = CipherTree::Directory(vec![(
            "ciphertext".into(),
            CipherTree::File(b"new ciphertext".to_vec()),
        )]);
        publish_entries(&tree, &held).unwrap();
        assert_eq!(
            fs::read(moved.join("ciphertext")).unwrap(),
            b"new ciphertext"
        );
        assert!(!outside.join("ciphertext").exists());
        assert!(directory(&original).is_err());
        let target = moved.join("alias");
        fs::write(outside.join("preserve"), b"preserve").unwrap();
        std::os::unix::fs::symlink(outside.join("preserve"), &target).unwrap();
        let tree = CipherTree::Directory(vec![(
            "alias".into(),
            CipherTree::File(b"replace".to_vec()),
        )]);
        assert!(publish_entries(&tree, &held).is_err());
        assert_eq!(fs::read(outside.join("preserve")).unwrap(), b"preserve");
    }

    #[test]
    fn unveil_remembers_directory_identity_across_path_replacement() {
        const CHILD: &str = "LURKER_TEST_OPENBSD_UNVEIL_FD";
        if let Ok(fd) = std::env::var(CHILD) {
            let fd: i32 = fd.parse().unwrap();
            let target = std::env::var_os("LURKER_TEST_OPENBSD_UNVEIL_TARGET").unwrap();
            assert_eq!(unsafe { libc::fchdir(fd) }, 0);
            unveil(c".", c"rwc").unwrap();
            assert_eq!(
                unsafe { libc::unveil(std::ptr::null(), std::ptr::null()) },
                0
            );
            assert!(fs::write(target, b"must not write").is_err());
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let moved = root.path().join("moved");
        fs::create_dir(&original).unwrap();
        let held = directory(&original).unwrap();
        fs::rename(&original, &moved).unwrap();
        fs::create_dir(&original).unwrap();
        let target = original.join("must-not-write");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "folder::openbsd::tests::unveil_remembers_directory_identity_across_path_replacement"])
            .env_clear().env(CHILD, held.as_raw_fd().to_string()).env("LURKER_TEST_OPENBSD_UNVEIL_TARGET", &target)
            .stdout(Stdio::null()).stderr(Stdio::null());
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(held.as_raw_fd(), libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        assert!(command.status().unwrap().success());
        assert!(!target.exists());
    }
}
