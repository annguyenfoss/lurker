//! Exclusive publication for native drivers lacking cross-parent directory rename.
//! Configuration publication remains with the caller, after every data entry.
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
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

fn move_file(source: &File, target: &File, name: &CStr, expected: &libc::stat) -> io::Result<()> {
    let file = open_at(source.as_raw_fd(), name, false)?;
    if !same(expected, &stat(&file)?) {
        return Err(invalid("Publication source changed"));
    }
    file.sync_all()?;
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
    target.sync_all()?;
    source.sync_all()
}

fn move_directory(
    source: &File,
    target: &File,
    name: &CStr,
    expected: &libc::stat,
    depth: usize,
) -> io::Result<()> {
    if depth > 64 {
        return Err(invalid("Publication nesting is too deep"));
    }
    let input = open_at(source.as_raw_fd(), name, true)?;
    if !same(expected, &stat(&input)?) {
        return Err(invalid("Publication directory changed"));
    }
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
    if unsafe { libc::unlinkat(source.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
        return Err(io::Error::last_os_error());
    }
    source.sync_all()?;
    target.sync_all()
}

fn move_contents(source: &File, target: &File, depth: usize) -> io::Result<()> {
    for name in names(source)? {
        let entry = stat_at(source.as_raw_fd(), &name)?;
        match entry.st_mode & libc::S_IFMT {
            libc::S_IFDIR => move_directory(source, target, &name, &entry, depth + 1)?,
            libc::S_IFREG => move_file(source, target, &name, &entry)?,
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
    move_directory(&input, &output, &source_name, &entry, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

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
            move_file(&source, &target, &name, &entry)
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
