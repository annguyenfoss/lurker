//! Fail-closed underlying-directory permissions, independent of mount lifetime.
use super::*;
use std::os::fd::FromRawFd;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Journal {
    identity: Identity,
    owner: u32,
    original_mode: u32,
    pub(super) armed: bool,
}

#[derive(Serialize, Deserialize)]
struct Request { nonce: String }
#[derive(Serialize, Deserialize)]
struct Receipt { nonce: String, restored: bool }

pub(super) fn required(kind: VolumeType) -> bool {
    cfg!(target_os = "macos") && kind == VolumeType::Cryptomator
}

fn metadata_matches(journal: &Journal, metadata: &fs::Metadata, mode: u32) -> bool {
    metadata.is_dir() && metadata.uid() == journal.owner && journal.owner == crate::effective_uid()
        && metadata.dev() == journal.identity.device && metadata.ino() == journal.identity.inode
        && metadata.mode() & 0o7777 == mode && journal.original_mode <= 0o7777
}

fn validate(path: &Path, file: &File, journal: &Journal, mode: u32) -> AppResult<()> {
    let held = file.metadata().map_err(|e| AppError::io("Cannot inspect held mountpoint", e))?;
    let named = fs::symlink_metadata(path).map_err(|e| AppError::io("Cannot inspect fenced mountpoint", e))?;
    if !metadata_matches(journal, &held, mode) || !metadata_matches(journal, &named, mode) {
        return Err(AppError::new("Mountpoint identity, owner or permissions changed; fence left untouched."));
    }
    no_extended_acl(file)
}

#[cfg(target_os = "macos")]
fn no_extended_acl(file: &File) -> AppResult<()> {
    unsafe extern "C" {
        fn acl_get_fd_np(fd: i32, kind: i32) -> *mut libc::c_void;
        fn acl_get_entry(acl: *mut libc::c_void, entry: i32, result: *mut *mut libc::c_void) -> i32;
        fn acl_free(acl: *mut libc::c_void) -> i32;
    }
    // Darwin's extended ACL entries can grant writes despite POSIX mode 000.
    // Refuse them rather than modifying or approximating an owner's ACL.
    let acl = unsafe { acl_get_fd_np(file.as_raw_fd(), 0x100) };
    if acl.is_null() {
        let error = std::io::Error::last_os_error();
        // Darwin represents the absence of an extended ACL as ENOENT, rather
        // than returning an allocated ACL with no entries. Other failures must
        // still prevent arming the fence.
        return if error.raw_os_error() == Some(libc::ENOENT) { Ok(()) }
            else { Err(AppError::io("Cannot inspect mountpoint ACL", error)) };
    }
    let mut entry = std::ptr::null_mut();
    let result = unsafe { acl_get_entry(acl, 0, &mut entry) };
    let error = std::io::Error::last_os_error().raw_os_error();
    unsafe { acl_free(acl); }
    if result == -1 && error == Some(libc::EINVAL) { Ok(()) }
    else { Err(AppError::new("Fenced mountpoints must have no extended ACL; choose an empty private directory.")) }
}
#[cfg(not(target_os = "macos"))]
fn no_extended_acl(_: &File) -> AppResult<()> { Ok(()) }

fn empty(file: &File) -> AppResult<bool> {
    let descriptor = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if descriptor == -1 { return Err(AppError::io("Cannot duplicate mountpoint descriptor", std::io::Error::last_os_error())); }
    let directory = unsafe { libc::fdopendir(descriptor) };
    if directory.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe { libc::close(descriptor); }
        return Err(AppError::io("Cannot inspect held mountpoint entries", error));
    }
    unsafe { libc::rewinddir(directory); }
    let result = (|| {
        loop {
            #[cfg(any(target_os = "macos", target_os = "freebsd"))]
            unsafe { *libc::__error() = 0; }
            #[cfg(any(target_os = "linux", target_os = "dragonfly"))]
            unsafe { *libc::__errno_location() = 0; }
            #[cfg(any(target_os = "netbsd", target_os = "openbsd"))]
            unsafe { *libc::__errno() = 0; }
            let entry = unsafe { libc::readdir(directory) };
            if entry.is_null() {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(0) { return Err(AppError::io("Cannot read held mountpoint", error)); }
                return Ok(true);
            }
            let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name != b"." && name != b".." { return Ok(false); }
        }
    })();
    unsafe { libc::closedir(directory); }
    result
}

pub(super) fn prepare(path: &Path) -> AppResult<(Journal, File)> {
    let file = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path).map_err(|e| AppError::io("Cannot hold original mountpoint", e))?;
    let metadata = file.metadata().map_err(|e| AppError::io("Cannot inspect original mountpoint", e))?;
    let journal = Journal { identity: Identity { device: metadata.dev(), inode: metadata.ino() },
        owner: metadata.uid(), original_mode: metadata.mode() & 0o7777, armed: false };
    validate(path, &file, &journal, journal.original_mode)?;
    if !empty(&file)? { return Err(AppError::new("Fenced mountpoint must be empty.")); }
    Ok((journal, file))
}

pub(super) fn arm(path: &Path, file: &File, journal: &mut Journal) -> AppResult<()> {
    validate(path, file, journal, journal.original_mode)?;
    if unsafe { libc::fchmod(file.as_raw_fd(), 0) } != 0 {
        return Err(AppError::io("Cannot fence underlying mountpoint", std::io::Error::last_os_error()));
    }
    file.sync_all().map_err(|e| AppError::io("Cannot persist mountpoint fence", e))?;
    journal.armed = true;
    validate(path, file, journal, 0)?;
    if !empty(file)? { return Err(AppError::new("Mountpoint changed before fencing; protected directory retained.")); }
    Ok(())
}

pub(super) fn inherited(descriptor: Option<i32>, session: &Session) -> AppResult<Option<File>> {
    match (&session.fence, descriptor) {
        (None, None) => Ok(None),
        (Some(journal), Some(fd)) if journal.armed && fd >= 3 => {
            let file = unsafe { File::from_raw_fd(fd) };
            // The guardian keeps it; the encryption engine must not inherit it.
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
                return Err(AppError::io("Cannot protect held mountpoint descriptor", std::io::Error::last_os_error()));
            }
            validate(&session.mountpoint, &file, journal, 0)?;
            if !empty(&file)? { return Err(AppError::new("Fenced mountpoint is no longer empty.")); }
            Ok(Some(file))
        }
        _ => Err(AppError::new("Fenced session requires its original held directory descriptor.")),
    }
}

fn restore(session: &Session, file: &File) -> AppResult<()> {
    let journal = session.fence.as_ref().ok_or_else(|| AppError::new("Missing fence journal"))?;
    if Identity::read(&session.source)? != session.source_identity || mount_identity(&session.mountpoint)?.is_some()
        || session.process.as_ref().is_some_and(|p| alive(p).unwrap_or(true)) {
        return Err(AppError::new("Source, mount or process state cannot authorize fence restoration."));
    }
    validate(&session.mountpoint, file, journal, 0)?;
    if !empty(file)? { return Err(AppError::new("Underlying mountpoint changed; fence retained.")); }
    if unsafe { libc::fchmod(file.as_raw_fd(), journal.original_mode as libc::mode_t) } != 0 {
        return Err(AppError::io("Cannot restore held mountpoint permissions", std::io::Error::last_os_error()));
    }
    file.sync_all().map_err(|e| AppError::io("Cannot persist restored mountpoint permissions", e))?;
    Ok(())
}

pub(super) fn guard_until_requested(dir: &Path, session: &Session, file: &File) -> AppResult<()> {
    let mut previous = String::new();
    loop {
        if let Some(request) = supervisor::read_record::<Request>(dir, "fence-restore-request.json")? {
            if request.nonce.len() <= 128 && request.nonce != previous {
                previous = request.nonce;
                let restored = restore(session, file).is_ok();
                supervisor::write_record(dir, "fence-restore-result.json", &Receipt { nonce: previous.clone(), restored })?;
                if restored { return Ok(()); }
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub(super) fn request_restore(dir: &Path, session: &Session) -> AppResult<()> {
    let Some(journal) = &session.fence else { return Ok(()); };
    if !journal.armed {
        let metadata = fs::symlink_metadata(&session.mountpoint).map_err(|e| AppError::io("Cannot inspect fence intent", e))?;
        return if metadata_matches(journal, &metadata, journal.original_mode) { Ok(()) }
            else { Err(AppError::new("Incomplete fence intent retained; explicit manual recovery is required.")) };
    }
    // The guardian persists its ACK before exiting. A frontend may die after
    // that ACK but before updating the session journal. Recognize completion
    // without reopening/chmodding a path after the original descriptor is lost.
    if let (Some(request), Some(receipt)) = (
        supervisor::read_record::<Request>(dir, "fence-restore-request.json")?,
        supervisor::read_record::<Receipt>(dir, "fence-restore-result.json")?,
    ) {
        if !request.nonce.is_empty() && request.nonce == receipt.nonce && receipt.restored {
            let metadata = fs::symlink_metadata(&session.mountpoint)
                .map_err(|e| AppError::io("Cannot verify completed fence restoration", e))?;
            return if metadata_matches(journal, &metadata, journal.original_mode)
                && Identity::read(&session.source)? == session.source_identity
                && mount_identity(&session.mountpoint)?.is_none()
                && !session.process.as_ref().is_some_and(|p| alive(p).unwrap_or(true)) {
                Ok(())
            } else {
                Err(AppError::new("Completed fence restoration no longer matches its session; directory left untouched."))
            };
        }
    }
    let monitor = session.supervisor.as_ref().ok_or_else(|| AppError::new("Fence guardian identity missing; permissions left unchanged."))?;
    if !alive(monitor)? {
        return Err(AppError::new("Original mountpoint descriptor was lost with its guardian; permissions left unchanged. Explicit manual recovery is required."));
    }
    let request = Request { nonce: nonce().to_string() };
    supervisor::write_record(dir, "fence-restore-request.json", &request)?;
    let started = Instant::now();
    loop {
        if let Some(receipt) = supervisor::read_record::<Receipt>(dir, "fence-restore-result.json")? {
            if receipt.nonce == request.nonce {
                return if receipt.restored { Ok(()) } else { Err(AppError::new("Fence restoration refused: identity, permissions or lifecycle changed; protected directory left untouched.")) };
            }
        }
        if started.elapsed() > Duration::from_secs(10) || !alive(monitor)? {
            return Err(AppError::new("Fence restoration was not acknowledged; permissions must be inspected manually."));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("lurker-fence-unit-{}", nonce()));
        secure_dir(&root).unwrap();
        let source = root.join("source"); secure_dir(&source).unwrap();
        let target = root.join("target"); secure_dir(&target).unwrap();
        (root, source, target)
    }
    fn record(source: &Path, target: &Path, journal: Journal) -> Session {
        Session { schema: SCHEMA, backend: VolumeType::Cryptomator, source: source.into(),
            source_identity: Identity::read(source).unwrap(), mountpoint: target.into(), readonly: false,
            mount_identity: None, mounted_directory: None, process: None,
            supervisor: process_identity(std::process::id()), fence: Some(journal) }
    }
    fn cleanup(root: PathBuf, file: &File, mode: u32) {
        assert_eq!(unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) }, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn normal_explicit_restore_preserves_mode_and_denies_writes_until_requested() {
        let (root, source, target) = fixture();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o750)).unwrap();
        let (mut journal, file) = prepare(&target).unwrap();
        arm(&target, &file, &mut journal).unwrap();
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0);
        if crate::effective_uid() != 0 { assert!(fs::write(target.join("plaintext"), b"forbidden").is_err()); }
        let session = record(&source, &target, journal);
        let state = root.join("session"); secure_dir(&state).unwrap();
        let monitor_state = state.clone(); let monitor_session = session.clone();
        let keeper = file.try_clone().unwrap();
        let thread = std::thread::spawn(move || guard_until_requested(&monitor_state, &monitor_session, &keeper));
        request_restore(&state, &session).unwrap();
        thread.join().unwrap().unwrap();
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0o750);
        // Retry after the guardian has exited and the frontend has not yet
        // persisted armed=false. The durable receipt proves completion.
        let mut retry = session.clone();
        retry.supervisor = None;
        request_restore(&state, &retry).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(request_restore(&state, &retry).is_err());
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0o700);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacement_and_permission_changes_are_never_restored() {
        let (root, source, target) = fixture();
        let (mut journal, file) = prepare(&target).unwrap();
        arm(&target, &file, &mut journal).unwrap();
        let session = record(&source, &target, journal);
        let moved = root.join("moved"); fs::rename(&target, &moved).unwrap();
        secure_dir(&target).unwrap();
        assert!(restore(&session, &file).is_err());
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0o700);
        assert_eq!(moved.metadata().unwrap().mode() & 0o7777, 0);
        fs::remove_dir(&target).unwrap(); fs::rename(&moved, &target).unwrap();
        unsafe { libc::fchmod(file.as_raw_fd(), 0o500); }
        assert!(restore(&session, &file).is_err());
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0o500);
        cleanup(root, &file, 0o700);
    }

    #[test]
    fn lost_guardian_and_changed_source_leave_mode_zero() {
        let (root, source, target) = fixture();
        let (mut journal, file) = prepare(&target).unwrap();
        arm(&target, &file, &mut journal).unwrap();
        let mut session = record(&source, &target, journal);
        session.supervisor = None;
        assert!(request_restore(&root, &session).is_err());
        fs::rename(&source, root.join("old-source")).unwrap(); secure_dir(&source).unwrap();
        assert!(restore(&session, &file).is_err());
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0);
        cleanup(root, &file, 0o700);
    }

    #[test]
    fn contents_added_before_arm_are_detected_through_the_held_descriptor() {
        let (root, _, target) = fixture();
        let (mut journal, file) = prepare(&target).unwrap();
        fs::write(target.join("concurrent-file"), b"owned fixture").unwrap();
        assert!(arm(&target, &file, &mut journal).is_err());
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0);
        cleanup(root, &file, 0o700);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn absent_extended_acl_is_accepted_without_changing_permissions() {
        let (root, _, target) = fixture();
        let held = File::open(&target).unwrap();
        no_extended_acl(&held).unwrap();
        assert_eq!(held.metadata().unwrap().mode() & 0o7777, 0o700);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn extended_allow_acl_is_rejected_without_changing_its_entries_or_mode() {
        let (root, _, target) = fixture();
        let user = String::from_utf8(Command::new("/usr/bin/id").arg("-un").output().unwrap().stdout).unwrap();
        assert!(Command::new("/bin/chmod").args(["+a", &format!("user:{} allow list,add_file,search,add_subdirectory", user.trim())])
            .arg(&target).status().unwrap().success());
        let entries = || {
            let output = Command::new("/bin/ls").args(["-led"]).arg(&target).output().unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap().lines().skip(1).map(str::to_owned).collect::<Vec<_>>()
        };
        let before = entries();
        assert!(!before.is_empty());
        assert!(prepare(&target).unwrap_err().message.contains("ACL"));
        assert_eq!(entries(), before);
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0o700);
        // Demonstrate why mode bits alone are not a fence on an ACL target.
        let held = File::open(&target).unwrap();
        unsafe { libc::fchmod(held.as_raw_fd(), 0); }
        fs::write(target.join("acl-control"), b"owned test").unwrap();
        assert!(prepare(&target).unwrap_err().message.contains("ACL"));
        assert_eq!(entries(), before);
        assert_eq!(target.metadata().unwrap().mode() & 0o7777, 0);
        cleanup(root, &held, 0o700);
    }
}
