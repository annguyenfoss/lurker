//! Exclusive publication for native drivers lacking cross-parent directory rename.
//! Configuration publication remains with the caller, after every data entry.
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileExt;
use std::path::Path;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn name(path: &Path) -> io::Result<CString> {
    let value = path
        .file_name()
        .ok_or_else(|| invalid("Missing publication name"))?;
    CString::new(value.as_bytes()).map_err(|_| invalid("Invalid publication name"))
}

fn open_at(parent: i32, name: &CStr, directory: bool) -> io::Result<File> {
    let flags = libc::O_RDONLY
        | libc::O_CLOEXEC
        | libc::O_NOFOLLOW
        | libc::O_NONBLOCK
        | if directory { libc::O_DIRECTORY } else { 0 };
    let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn stat_at(parent: i32, name: &CStr) -> io::Result<libc::stat> {
    let mut value = std::mem::MaybeUninit::uninit();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            value.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { value.assume_init() })
}

fn stat(file: &File) -> io::Result<libc::stat> {
    let mut value = std::mem::MaybeUninit::uninit();
    if unsafe { libc::fstat(file.as_raw_fd(), value.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { value.assume_init() })
}

fn same(a: &libc::stat, b: &libc::stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode & libc::S_IFMT == b.st_mode & libc::S_IFMT
}

// Some native drivers expose Darwin attributes only as AppleDouble files.
// fcopyfile(COPYFILE_XATTR) can report success without enumerating those
// attributes, and removing the base directory also removes its companion.
// Capture the anchored companion before either operation. A prefix alone is
// never sufficient: ordinary ._* files must not be discarded as metadata.
struct Companion {
    name: CString,
    identity: libc::stat,
    file: File,
    bytes: Vec<u8>,
}

fn companion_name(base: &CStr) -> io::Result<CString> {
    let mut bytes = b"._".to_vec();
    bytes.extend_from_slice(base.to_bytes());
    CString::new(bytes).map_err(|_| invalid("Invalid companion name"))
}

fn is_apple_double(bytes: &[u8]) -> bool {
    if bytes.len() < 26 || bytes[..8] != [0, 5, 22, 7, 0, 2, 0, 0] {
        return false;
    }
    let count = u16::from_be_bytes([bytes[24], bytes[25]]) as usize;
    let table_end = 26 + count * 12;
    if !(1..=16).contains(&count) || table_end > bytes.len() {
        return false;
    }
    let mut ranges = Vec::new();
    let mut finder_info = false;
    let mut ids = std::collections::BTreeSet::new();
    for entry in bytes[26..table_end].chunks_exact(12) {
        let id = u32::from_be_bytes(entry[..4].try_into().unwrap());
        let offset = u32::from_be_bytes(entry[4..8].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(entry[8..12].try_into().unwrap()) as usize;
        let Some(end) = offset.checked_add(length) else {
            return false;
        };
        if offset < table_end || end > bytes.len() || !ids.insert(id) {
            return false;
        }
        if id == 9 && length >= 32 {
            finder_info = true;
        }
        ranges.push((offset, end));
    }
    ranges.sort_unstable();
    finder_info && ranges.windows(2).all(|pair| pair[0].1 <= pair[1].0)
}

fn capture_companion(parent: &File, base: &CStr) -> io::Result<Option<Companion>> {
    let name = companion_name(base)?;
    let expected = match stat_at(parent.as_raw_fd(), &name) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if expected.st_mode & libc::S_IFMT != libc::S_IFREG
        || !(26..=16 * 1024 * 1024).contains(&expected.st_size)
    {
        return Err(invalid(
            "Publication companion is not bounded native metadata",
        ));
    }
    let mut file = open_at(parent.as_raw_fd(), &name, false)?;
    if !same(&expected, &stat(&file)?) {
        return Err(invalid("Publication companion changed"));
    }
    let mut bytes = Vec::with_capacity(expected.st_size as usize);
    (&mut file)
        .take(expected.st_size as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() != expected.st_size as usize || !is_apple_double(&bytes) {
        return Err(invalid(
            "Publication refuses an ordinary file at a metadata name",
        ));
    }
    file.sync_all()?;
    Ok(Some(Companion {
        name,
        identity: expected,
        file,
        bytes,
    }))
}

fn check_companion_destination(target: &File, base: &CStr) -> io::Result<()> {
    // A preexisting ._* winner must remain untouched even when the source has
    // no companion: native attribute operations could otherwise adopt it.
    match stat_at(target.as_raw_fd(), &companion_name(base)?) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
        Ok(_) => return Err(io::Error::from_raw_os_error(libc::EEXIST)),
    }
    Ok(())
}

fn companion_matches(file: &File, companion: &Companion) -> io::Result<bool> {
    let actual = stat(file)?;
    if actual.st_mode & libc::S_IFMT != libc::S_IFREG
        || actual.st_size != companion.bytes.len() as i64
    {
        return Ok(false);
    }
    let mut bytes = vec![0; companion.bytes.len()];
    file.read_exact_at(&mut bytes, 0)?;
    Ok(bytes == companion.bytes)
}

fn publish_companion(source: &File, target: &File, companion: &Companion) -> io::Result<()> {
    let fd = unsafe {
        libc::openat(
            target.as_raw_fd(),
            companion.name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd >= 0 {
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(&companion.bytes)?;
        file.sync_all()?;
        if !same(
            &stat(&file)?,
            &stat_at(target.as_raw_fd(), &companion.name)?,
        ) {
            return Err(invalid("Published companion changed"));
        }
    } else {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EEXIST) {
            return Err(error);
        }
        // Native rename may already have moved the exact companion. Never
        // replace a winner, including metadata created by fcopyfile itself.
        let file = open_at(target.as_raw_fd(), &companion.name, false)?;
        if !companion_matches(&file, companion)? {
            return Err(error);
        }
        file.sync_all()?;
    }
    target.sync_all()?;
    match stat_at(source.as_raw_fd(), &companion.name) {
        Ok(current) => {
            if !same(&current, &companion.identity)
                || !companion_matches(&companion.file, companion)?
            {
                return Err(invalid("Original companion changed"));
            }
            if unsafe { libc::unlinkat(source.as_raw_fd(), companion.name.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error());
            }
            source.sync_all()?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    Ok(())
}

struct Listing(*mut libc::DIR);
impl Drop for Listing {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

fn names(directory: &File) -> io::Result<Vec<CString>> {
    // fdopendir owns this duplicate, leaving the anchored directory FD intact.
    let fd = unsafe { libc::fcntl(directory.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let pointer = unsafe { libc::fdopendir(fd) };
    if pointer.is_null() {
        let error = io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(error);
    }
    let listing = Listing(pointer);
    let mut result = Vec::new();
    loop {
        unsafe {
            *libc::__error() = 0;
        }
        let entry = unsafe { libc::readdir(listing.0) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                return Err(error);
            }
            break;
        }
        let value = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        if value.to_bytes() != b"." && value.to_bytes() != b".." {
            if result.len() >= 100_000 {
                return Err(invalid("Publication directory is too large"));
            }
            result.push(value.to_owned());
        }
    }
    result.sort();
    Ok(result)
}

fn move_file(
    source: &File,
    target: &File,
    name: &CStr,
    expected: &libc::stat,
    companion: Option<Companion>,
) -> io::Result<()> {
    let file = open_at(source.as_raw_fd(), name, false)?;
    if !same(expected, &stat(&file)?) {
        return Err(invalid("Publication source changed"));
    }
    file.sync_all()?;
    check_companion_destination(target, name)?;
    if unsafe {
        libc::renameatx_np(
            source.as_raw_fd(),
            name.as_ptr(),
            target.as_raw_fd(),
            name.as_ptr(),
            libc::RENAME_EXCL,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if !same(expected, &stat_at(target.as_raw_fd(), name)?) {
        return Err(invalid("Published file changed during transfer"));
    }
    if let Some(companion) = companion {
        publish_companion(source, target, &companion)?;
    }
    target.sync_all()?;
    source.sync_all()
}

fn move_directory(
    source: &File,
    target: &File,
    name: &CStr,
    expected: &libc::stat,
    depth: usize,
    companion: Option<Companion>,
) -> io::Result<()> {
    if depth > 64 {
        return Err(invalid("Publication nesting is too deep"));
    }
    let input = open_at(source.as_raw_fd(), name, true)?;
    if !same(expected, &stat(&input)?) {
        return Err(invalid("Publication directory changed"));
    }
    check_companion_destination(target, name)?;
    if unsafe { libc::mkdirat(target.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let output = open_at(target.as_raw_fd(), name, true)?;
    let destination = stat(&output)?;
    if destination.st_mode & 0o777 != 0o700 {
        return Err(invalid(
            "Publication directory does not retain private permissions",
        ));
    }
    // Darwin may store directory attributes in an AppleDouble companion.
    // Preserve them through anchored FDs before removing the original directory;
    // unlinking it also removes its companion on these native filesystems.
    if unsafe {
        libc::fcopyfile(
            input.as_raw_fd(),
            output.as_raw_fd(),
            std::ptr::null_mut(),
            libc::COPYFILE_XATTR,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    target.sync_all()?;
    move_contents(&input, &output, depth)?;
    output.sync_all()?;
    if !same(expected, &stat_at(source.as_raw_fd(), name)?)
        || !same(&destination, &stat_at(target.as_raw_fd(), name)?)
    {
        return Err(invalid("Publication directory was replaced"));
    }
    if let Some(companion) = companion {
        // Make the captured metadata durable before Darwin can remove its
        // original AppleDouble file together with the source directory.
        publish_companion(source, target, &companion)?;
    }
    if unsafe { libc::unlinkat(source.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
        return Err(io::Error::last_os_error());
    }
    source.sync_all()?;
    target.sync_all()
}

fn move_contents(source: &File, target: &File, depth: usize) -> io::Result<()> {
    let initialized = names(source)?;
    let mut companions = std::collections::BTreeMap::new();
    let mut deferred = std::collections::BTreeSet::new();
    let mut metadata_bytes = 0usize;
    for name in &initialized {
        if let Some(companion) = capture_companion(source, name)? {
            metadata_bytes += companion.bytes.len();
            if metadata_bytes > 64 * 1024 * 1024 {
                return Err(invalid("Publication metadata is too large"));
            }
            deferred.insert(companion.name.clone());
            companions.insert(name.clone(), companion);
        }
    }
    for name in initialized {
        if deferred.contains(&name) {
            continue;
        }
        let entry = stat_at(source.as_raw_fd(), &name)?;
        let companion = companions.remove(&name);
        match entry.st_mode & libc::S_IFMT {
            libc::S_IFDIR => move_directory(source, target, &name, &entry, depth + 1, companion)?,
            libc::S_IFREG => move_file(source, target, &name, &entry, companion)?,
            _ => {
                return Err(invalid(
                    "Vault publication refuses symlinks and special files",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn publish_directory(source: &Path, target: &Path) -> io::Result<()> {
    let source_name = name(source)?;
    let target_name = name(target)?;
    if source_name != target_name {
        return Err(invalid("Publication names differ"));
    }
    let parent_name = |path: &Path| {
        CString::new(
            path.parent()
                .unwrap_or(Path::new("."))
                .as_os_str()
                .as_bytes(),
        )
        .map_err(|_| invalid("Invalid publication parent"))
    };
    let input = open_at(libc::AT_FDCWD, &parent_name(source)?, true)?;
    let output = open_at(libc::AT_FDCWD, &parent_name(target)?, true)?;
    let entry = stat_at(input.as_raw_fd(), &source_name)?;
    if entry.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Err(invalid("Publication source is not a directory"));
    }
    let companion = capture_companion(&input, &source_name)?;
    move_directory(&input, &output, &source_name, &entry, 0, companion)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn apple_double() -> Vec<u8> {
        let mut value = vec![0u8; 26 + 24 + 32 + 7];
        value[..8].copy_from_slice(&[0, 5, 22, 7, 0, 2, 0, 0]);
        value[24..26].copy_from_slice(&2u16.to_be_bytes());
        value[26..30].copy_from_slice(&9u32.to_be_bytes());
        value[30..34].copy_from_slice(&50u32.to_be_bytes());
        value[34..38].copy_from_slice(&32u32.to_be_bytes());
        value[38..42].copy_from_slice(&2u32.to_be_bytes());
        value[42..46].copy_from_slice(&82u32.to_be_bytes());
        value[46..50].copy_from_slice(&7u32.to_be_bytes());
        value[82..].copy_from_slice(b"fork123");
        value
    }

    #[test]
    fn retains_exact_native_companions_of_nested_directories() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(input.join("child")).unwrap();
        fs::create_dir(root.path().join("vault")).unwrap();
        fs::write(input.join("child/block"), b"ciphertext").unwrap();
        let metadata = apple_double();
        assert!(is_apple_double(&metadata));
        fs::write(input.with_file_name("._data"), &metadata).unwrap();
        fs::write(input.join("._child"), &metadata).unwrap();
        publish_directory(&input, &output).unwrap();
        assert_eq!(fs::read(output.with_file_name("._data")).unwrap(), metadata);
        assert_eq!(fs::read(output.join("._child")).unwrap(), metadata);
        assert_eq!(fs::read(output.join("child/block")).unwrap(), b"ciphertext");
        assert!(!input.exists());
        assert!(!input.with_file_name("._data").exists());
    }

    #[test]
    fn ordinary_dot_underscore_files_are_retained_and_never_ignored() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(input.join("child")).unwrap();
        fs::create_dir(root.path().join("vault")).unwrap();
        fs::write(input.join("._child"), b"ordinary independent data").unwrap();
        assert!(publish_directory(&input, &output).is_err());
        assert_eq!(
            fs::read(input.join("._child")).unwrap(),
            b"ordinary independent data"
        );
        assert!(input.join("child").is_dir());
        let orphan = root.path().join("stage/orphan");
        fs::create_dir(&orphan).unwrap();
        fs::write(orphan.join("._standalone"), b"independent").unwrap();
        publish_directory(&orphan, &root.path().join("vault/orphan")).unwrap();
        assert_eq!(
            fs::read(root.path().join("vault/orphan/._standalone")).unwrap(),
            b"independent"
        );
    }

    #[test]
    fn companion_winner_is_not_overwritten_or_removed() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir(root.path().join("vault")).unwrap();
        fs::write(input.with_file_name("._data"), apple_double()).unwrap();
        fs::write(output.with_file_name("._data"), b"winner").unwrap();
        assert_eq!(
            publish_directory(&input, &output)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EEXIST)
        );
        assert_eq!(
            fs::read(output.with_file_name("._data")).unwrap(),
            b"winner"
        );
        assert!(input.is_dir());
        assert!(input.with_file_name("._data").is_file());
        assert!(!output.exists());
        fs::remove_file(input.with_file_name("._data")).unwrap();
        assert_eq!(
            publish_directory(&input, &output)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EEXIST)
        );
        assert_eq!(
            fs::read(output.with_file_name("._data")).unwrap(),
            b"winner"
        );
        assert!(input.is_dir());
        assert!(!output.exists());
    }

    #[test]
    fn malformed_and_symlink_companions_are_retained_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir(root.path().join("vault")).unwrap();
        let companion = input.with_file_name("._data");
        let mut outside = apple_double();
        outside[30..34].copy_from_slice(&u32::MAX.to_be_bytes());
        let mut overlapping = apple_double();
        overlapping[42..46].copy_from_slice(&50u32.to_be_bytes());
        for bytes in [apple_double()[..12].to_vec(), outside, overlapping] {
            fs::write(&companion, &bytes).unwrap();
            assert!(publish_directory(&input, &output).is_err());
            assert_eq!(fs::read(&companion).unwrap(), bytes);
            assert!(input.is_dir());
            assert!(!output.exists());
        }
        fs::remove_file(&companion).unwrap();
        let independent = root.path().join("independent");
        fs::write(&independent, b"untouched").unwrap();
        symlink(&independent, &companion).unwrap();
        assert!(publish_directory(&input, &output).is_err());
        assert_eq!(fs::read(&independent).unwrap(), b"untouched");
        assert!(
            fs::symlink_metadata(&companion)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!output.exists());
    }

    #[test]
    fn publishes_nested_data_with_private_directories() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(input.join("nested")).unwrap();
        fs::create_dir(root.path().join("vault")).unwrap();
        fs::write(input.join("nested/block"), b"ciphertext").unwrap();
        publish_directory(&input, &output).unwrap();
        assert!(!input.exists());
        assert_eq!(
            fs::read(output.join("nested/block")).unwrap(),
            b"ciphertext"
        );
        assert_eq!(
            fs::metadata(output.join("nested"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn preserves_directory_attributes_through_open_descriptors() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir(root.path().join("vault")).unwrap();
        let file = File::open(&input).unwrap();
        let attribute = CString::new("com.lurker.publication-test").unwrap();
        let value = b"preserved";
        assert_eq!(
            unsafe {
                libc::fsetxattr(
                    file.as_raw_fd(),
                    attribute.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                    0,
                )
            },
            0
        );
        publish_directory(&input, &output).unwrap();
        let file = File::open(&output).unwrap();
        let mut actual = [0u8; 32];
        let count = unsafe {
            libc::fgetxattr(
                file.as_raw_fd(),
                attribute.as_ptr(),
                actual.as_mut_ptr().cast(),
                actual.len(),
                0,
                0,
            )
        };
        assert_eq!(count, value.len() as isize);
        assert_eq!(&actual[..count as usize], value);
    }

    #[test]
    fn refuses_existing_directory_and_regular_file_winners() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(input.join("block"), b"source").unwrap();
        fs::write(output.join("block"), b"winner").unwrap();
        assert_eq!(
            publish_directory(&input, &output)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EEXIST)
        );
        let source = File::open(&input).unwrap();
        let target = File::open(&output).unwrap();
        let name = CString::new("block").unwrap();
        let entry = stat_at(source.as_raw_fd(), &name).unwrap();
        assert_eq!(
            move_file(&source, &target, &name, &entry, None)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EEXIST)
        );
        assert_eq!(fs::read(input.join("block")).unwrap(), b"source");
        assert_eq!(fs::read(output.join("block")).unwrap(), b"winner");
    }

    #[test]
    fn partial_error_retains_data_without_following_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("stage/data");
        let output = root.path().join("vault/data");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir(root.path().join("vault")).unwrap();
        fs::write(input.join("a-block"), b"ciphertext").unwrap();
        fs::write(root.path().join("outside"), b"untouched").unwrap();
        symlink(root.path().join("outside"), input.join("z-link")).unwrap();
        assert!(publish_directory(&input, &output).is_err());
        assert_eq!(fs::read(output.join("a-block")).unwrap(), b"ciphertext");
        assert!(
            fs::symlink_metadata(input.join("z-link"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(root.path().join("outside")).unwrap(), b"untouched");
        assert!(!root.path().join("vault/vault.cryptomator").exists());
    }
}
