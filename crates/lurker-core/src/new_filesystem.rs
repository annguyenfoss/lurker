//! Initial ownership for newly created storage only. Unlocking existing data
//! must preserve its owners and permissions. Callers own and verify any mounts;
//! this module operates exclusively on already-open file descriptors.
use crate::{AppError, AppResult};
use std::ffi::{CStr, CString};
use std::fs::{File, Permissions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Owner {
    pub uid: libc::uid_t,
    pub gid: libc::gid_t,
}

#[derive(Debug, Eq, PartialEq)]
enum Caller {
    Uid(libc::uid_t),
    Name(String),
}

fn caller(
    real_uid: libc::uid_t,
    sudo_uid: Option<&str>,
    pkexec_uid: Option<&str>,
    doas_user: Option<&str>,
) -> AppResult<Caller> {
    if real_uid != 0 {
        return Ok(Caller::Uid(real_uid));
    }
    let parse = |value: &str| {
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AppError::new(
                "Invalid invoking-user identity from authorization.",
            ));
        }
        value
            .parse::<libc::uid_t>()
            .map_err(|_| AppError::new("Invoking-user identity exceeds this platform's UID range."))
    };
    let sudo = sudo_uid.map(parse).transpose()?;
    let pkexec = pkexec_uid.map(parse).transpose()?;
    if let (Some(a), Some(b)) = (sudo, pkexec) {
        if a != b {
            return Err(AppError::new(
                "Conflicting invoking-user identities from authorization.",
            ));
        }
    }
    if let Some(uid) = pkexec.or(sudo) {
        return Ok(Caller::Uid(uid));
    }
    if let Some(name) = doas_user {
        if name.is_empty() || name.as_bytes().contains(&0) {
            return Err(AppError::new(
                "Invalid invoking-user name from authorization.",
            ));
        }
        return Ok(Caller::Name(name.to_owned()));
    }
    Ok(Caller::Uid(0))
}

/// Kernel identity wins. Only an actual root process uses the identity supplied
/// by its trusted sudo/pkexec/doas launcher; an ordinary process cannot select
/// another owner through environment variables. Groups come from the OS account
/// database, not from an untrusted group-ID environment variable.
pub(crate) fn invoking_owner() -> AppResult<Owner> {
    let effective = unsafe { libc::geteuid() };
    if effective != 0 {
        return Ok(Owner {
            uid: effective,
            gid: unsafe { libc::getegid() },
        });
    }
    let sudo = std::env::var("SUDO_UID").ok();
    let pkexec = std::env::var("PKEXEC_UID").ok();
    let doas = std::env::var("DOAS_USER").ok();
    account(caller(
        unsafe { libc::getuid() },
        sudo.as_deref(),
        pkexec.as_deref(),
        doas.as_deref(),
    )?)
}

fn account(caller: Caller) -> AppResult<Owner> {
    let name = match &caller {
        Caller::Name(name) => {
            Some(CString::new(name.as_str()).map_err(|_| AppError::new("Invalid account name."))?)
        }
        _ => None,
    };
    let mut size = 16384;
    loop {
        let mut buffer = vec![0u8; size];
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut found = std::ptr::null_mut();
        let code = unsafe {
            match &caller {
                Caller::Uid(uid) => libc::getpwuid_r(
                    *uid,
                    &mut entry,
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &mut found,
                ),
                Caller::Name(_) => libc::getpwnam_r(
                    name.as_ref().unwrap().as_ptr(),
                    &mut entry,
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &mut found,
                ),
            }
        };
        if code == libc::ERANGE && size < 1024 * 1024 {
            size *= 2;
            continue;
        }
        if code != 0 {
            return Err(AppError::io(
                "Cannot resolve invoking user's account",
                std::io::Error::from_raw_os_error(code),
            ));
        }
        if found.is_null() || entry.pw_name.is_null() {
            return Err(AppError::new(
                "Invoking user's account does not exist; refusing storage creation.",
            ));
        }
        // Touch the name while its backing buffer is alive; no libc pointers
        // escape the account lookup.
        if unsafe { CStr::from_ptr(entry.pw_name) }.is_empty() {
            return Err(AppError::new("Invoking user's account has no name."));
        }
        return Ok(Owner {
            uid: entry.pw_uid,
            gid: entry.pw_gid,
        });
    }
}

fn assign(file: &File, owner: Owner, directory: bool) -> AppResult<()> {
    let metadata = file
        .metadata()
        .map_err(|e| AppError::io("Inspect newly created storage", e))?;
    if directory && !metadata.is_dir() || !directory && !metadata.is_file() {
        return Err(AppError::new(
            "New-storage ownership requires the expected open file type.",
        ));
    }
    if unsafe { libc::fchown(file.as_raw_fd(), owner.uid, owner.gid) } != 0 {
        return Err(AppError::io(
            "Assign newly created storage to its invoking user",
            std::io::Error::last_os_error(),
        ));
    }
    file.set_permissions(Permissions::from_mode(if directory {
        0o700
    } else {
        0o600
    }))
    .map_err(|e| AppError::io("Protect newly created storage", e))?;
    file.sync_all()
        .map_err(|e| AppError::io("Sync newly created storage ownership", e))
}

/// The caller must supply a newly staged image or recovery-metadata file.
pub(crate) fn assign_file(file: &File, owner: Owner) -> AppResult<()> {
    assign(file, owner, false)
}

/// The caller must have journaled and verified a private mount of a newly
/// formatted filesystem before opening its root directory with O_NOFOLLOW.
pub(crate) fn assign_root(directory: &File, owner: Owner) -> AppResult<()> {
    assign(directory, owner, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_caller_wins_over_supplied_environment() {
        assert_eq!(
            caller(1234, Some("0"), Some("invalid"), Some("root")).unwrap(),
            Caller::Uid(1234)
        );
    }

    #[test]
    fn elevation_identity_is_strict_and_conflicts_fail_closed() {
        assert_eq!(
            caller(0, Some("1234"), None, None).unwrap(),
            Caller::Uid(1234)
        );
        assert_eq!(
            caller(0, None, Some("1234"), None).unwrap(),
            Caller::Uid(1234)
        );
        assert_eq!(
            caller(0, None, None, Some("adjutant")).unwrap(),
            Caller::Name("adjutant".into())
        );
        assert_eq!(caller(0, None, None, None).unwrap(), Caller::Uid(0));
        assert!(caller(0, Some("1234"), Some("5678"), None).is_err());
        for value in ["", "-1", "+1", " 1234", "4294967296"] {
            assert!(caller(0, Some(value), None, None).is_err());
        }
    }

    #[test]
    fn account_groups_come_from_native_database() {
        let owner = account(Caller::Uid(unsafe { libc::getuid() })).unwrap();
        assert_eq!(owner.uid, unsafe { libc::getuid() });
    }
}
