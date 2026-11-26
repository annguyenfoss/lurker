//! Use the host's FUSE device and installed filesystem drivers. DragonFly's
//! native FUSE mount source names the FUSE device vnode, not the disk image.
use super::super::{args, exec, mapping_name};
use crate::bsd::{Mounted, STATE, exec_in_locale, mounts};
use crate::{AppError, AppResult, CreateFilesystem};
use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use std::time::Duration;

// sys/vfs/fuse/fuse_mount.h. Keep `from` on the same /dev/fuse vnode as fd:
// fuse_unmount performs VOP_CLOSE on that vnode. A decrypted disk is invalid.
#[repr(C)]
struct FuseMountInfo {
    flags: libc::c_int,
    fd: libc::c_int,
    max_read: libc::c_int,
    subtype: *const libc::c_char,
    from: *const libc::c_char,
}
extern "C" {
    #[link_name = "mount"]
    fn mount_syscall(
        filesystem: *const libc::c_char,
        target: *const libc::c_char,
        flags: libc::c_int,
        data: *mut libc::c_void,
    ) -> libc::c_int;
}

fn label(mapped: &str) -> AppResult<PathBuf> {
    let path = Path::new(STATE).join(format!("fuse-{}", mapping_name(mapped)?));
    check_native_path(&path)?;
    Ok(path)
}

fn check_native_path(path: &Path) -> AppResult<()> {
    // DragonFly's statfs mount names are 80-byte, NUL-terminated arrays.
    if !path.is_absolute() || path.as_os_str().as_bytes().len() >= 80 {
        return Err(AppError::new(
            "DragonFly FUSE mount paths must be absolute and shorter than 80 bytes.",
        ));
    }
    Ok(())
}

fn verified_label(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_symlink()
            && metadata.uid() == 0
            && fs::read_link(path).is_ok_and(|target| target == Path::new("/dev/fuse"))
    })
}

pub(super) fn matches(mapped: &str, mounted: &Mounted) -> bool {
    mounted.filesystem == "fuse"
        && label(mapped).is_ok_and(|path| mounted.source == path && verified_label(&path))
}

pub(super) fn close(mapped: &str) -> AppResult<()> {
    let path = label(mapped)?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(AppError::io("Inspect DragonFly FUSE label", error)),
        Ok(_) => (),
    }
    if !verified_label(&path) || mounts()?.iter().any(|mount| mount.source == path) {
        return Err(AppError::new(
            "DragonFly FUSE label changed or remains mounted; preserving its mapping.",
        ));
    }
    fs::remove_file(path).map_err(|error| AppError::io("Remove DragonFly FUSE label", error))
}

pub(super) fn mount(
    mapped: &str,
    target: &Path,
    filesystem: CreateFilesystem,
    readonly: bool,
) -> AppResult<()> {
    let path = label(mapped)?;
    check_native_path(target)?;
    let owner = crate::new_filesystem::invoking_owner()?;
    let mode = if readonly { "ro" } else { "rw" };
    let private = format!("{mode},uid={},gid={},umask=077", owner.uid, owner.gid);
    let (program, mut argv) = match filesystem {
        CreateFilesystem::Exfat => (
            "/usr/local/sbin/mount.exfat-fuse",
            args(&["-o", &private, mapped]),
        ),
        CreateFilesystem::Ntfs => ("/usr/local/bin/ntfs-3g", args(&["-o", &private, mapped])),
        CreateFilesystem::Ext4 => ("/usr/local/bin/fuse-ext2", args(&[mapped])),
        _ => return Err(AppError::new("Unsupported DragonFly FUSE filesystem.")),
    };
    let program = crate::trusted_privileged_executable(Path::new(program))?;
    if !Path::new("/dev/fuse").exists() {
        exec("/sbin/kldload", &args(&["fuse"]), None)?;
    }
    // The global registry is root-private and locked by the shared lifecycle.
    // A distinct root-owned alias gives the mount an exact mapping identity
    // while retaining the FUSE device vnode required by the native ABI.
    symlink("/dev/fuse", &path)
        .map_err(|error| AppError::io("Reserve DragonFly FUSE mount label", error))?;
    let descriptor = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/fuse")
        .map_err(|error| AppError::io("Open DragonFly FUSE device", error))?;
    if filesystem == CreateFilesystem::Ext4 {
        argv.push(target.as_os_str().into());
        argv.extend(args(&["-o", if readonly { "ro" } else { "rw+" }]));
    } else {
        argv.push(target.as_os_str().into());
    }
    let mut launch = args(&[
        &format!("FUSE_DEV_FD={}", descriptor.as_raw_fd()),
        "FUSE_NO_MOUNT=1",
    ]);
    launch.push(program.as_os_str().into());
    launch.extend(argv);
    // exec_in_locale validates /usr/bin/env, clears the ambient environment,
    // bounds launcher execution and inherits only this explicit device fd.
    exec_in_locale(
        "/usr/bin/env",
        &launch,
        None,
        "C",
        None,
        Duration::from_secs(45),
        Some(descriptor.as_raw_fd()),
    )?;
    let source = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| AppError::new("DragonFly FUSE label contains NUL."))?;
    let target = CString::new(target.as_os_str().as_bytes())
        .map_err(|_| AppError::new("DragonFly FUSE mountpoint contains NUL."))?;
    let mut info = FuseMountInfo {
        flags: 0x01 | 0x02, // default_permissions | allow_other
        fd: descriptor.as_raw_fd(),
        max_read: 0,
        subtype: std::ptr::null(),
        from: source.as_ptr(),
    };
    let flags = libc::MNT_NOSUID
        | libc::MNT_NODEV
        | libc::MNT_NOEXEC
        | if readonly { libc::MNT_RDONLY } else { 0 };
    // SAFETY: strings, the native argument block and the inherited FUSE device
    // stay alive for the syscall; the trusted driver services that exact fd.
    if unsafe {
        mount_syscall(
            c"fuse".as_ptr(),
            target.as_ptr(),
            flags,
            (&mut info as *mut FuseMountInfo).cast(),
        )
    } != 0
    {
        return Err(AppError::io(
            "Mount native DragonFly FUSE filesystem",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mount_labels_stay_in_the_private_mapping_namespace() {
        assert_eq!(
            label("/dev/mapper/lurker_test").unwrap(),
            Path::new(STATE).join("fuse-lurker_test")
        );
        assert!(label("/dev/mapper/root").is_err());
        assert!(label("/dev/mapper/lurker_../../root").is_err());
        assert!(label(&format!("/dev/mapper/lurker_{}", "x".repeat(70))).is_err());
    }
    #[test]
    fn mount_limits_count_native_bytes_and_reserve_the_terminator() {
        assert!(check_native_path(Path::new(&format!("/{}", "x".repeat(78)))).is_ok());
        assert!(check_native_path(Path::new(&format!("/{}", "x".repeat(79)))).is_err());
        assert!(check_native_path(Path::new(&format!("/{}", "é".repeat(40)))).is_err());
        assert!(check_native_path(Path::new("relative")).is_err());
    }
}
