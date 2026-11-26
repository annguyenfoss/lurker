//! Native ReFUSE is an administrator-configured host prerequisite.
use crate::{AppError, AppResult};
use std::os::unix::fs::FileTypeExt;

pub(super) fn require_fuse_access() -> AppResult<()> {
    let mut enabled: libc::c_int = 0;
    let mut length = std::mem::size_of_val(&enabled);
    let result = unsafe {
        libc::sysctlbyname(
            c"vfs.generic.usermount".as_ptr(),
            (&mut enabled as *mut libc::c_int).cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || length != std::mem::size_of_val(&enabled) {
        return Err(AppError::new(
            "Cannot verify NetBSD native user-mount policy; preserve existing storage.",
        ));
    }
    if enabled == 0 {
        return Err(AppError::new("NetBSD CryFS requires administrator-enabled vfs.generic.usermount=1 and native ReFUSE; no vault data was changed."));
    }
    let device = std::path::Path::new("/dev/puffs");
    let metadata =
        std::fs::metadata(device).map_err(|e| AppError::io("Inspect native puffs device", e))?;
    if !metadata.file_type().is_char_device()
        || unsafe { libc::access(c"/dev/puffs".as_ptr(), libc::R_OK | libc::W_OK) } != 0
    {
        return Err(AppError::new("NetBSD CryFS requires read/write access to the native puffs device; ask an administrator for scoped group access, not world-writable permissions."));
    }
    Ok(())
}
