//! Capability checks for separately installed native NetBSD FUSE libraries.
use crate::{AppError, AppResult};
use std::ffi::CStr;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

fn prerequisite(detail: &str) -> AppError {
    AppError::new(format!(
        "NetBSD native FUSE permission support is unavailable ({detail}). Install the host prerequisite described in native/netbsd/README.md before retrying; no filesystem driver was started."
    ))
}

fn trusted_library(path: &Path) -> AppResult<PathBuf> {
    let canonical =
        fs::canonicalize(path).map_err(|_| prerequisite("native library is missing"))?;
    if !canonical.starts_with("/usr/lib") {
        return Err(prerequisite(
            "library is outside the native system directory",
        ));
    }
    let metadata =
        fs::metadata(&canonical).map_err(|_| prerequisite("cannot inspect native library"))?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(prerequisite(
            "native library must be administrator-controlled",
        ));
    }
    for parent in canonical.ancestors().skip(1) {
        let metadata = fs::metadata(parent)
            .map_err(|_| prerequisite("cannot inspect native library parent"))?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(prerequisite(
                "native library parent is not administrator-controlled",
            ));
        }
    }
    let mut magic = [0u8; 4];
    File::open(&canonical)
        .and_then(|mut file| file.read_exact(&mut magic))
        .map_err(|_| prerequisite("cannot read native library"))?;
    if &magic != b"\x7fELF" {
        return Err(prerequisite("native library is not ELF"));
    }
    Ok(canonical)
}

fn require_marker(path: &Path, symbol: &CStr, expected: libc::c_uint) -> AppResult<()> {
    use std::os::unix::ffi::OsStrExt;
    let canonical = trusted_library(path)?;
    let name = std::ffi::CString::new(canonical.as_os_str().as_bytes())
        .map_err(|_| prerequisite("native library path is invalid"))?;
    // Only an administrator-controlled ELF reaches the native loader, which
    // validates its machine/class. The symbol must also belong to this provider.
    let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        return Err(prerequisite("cannot load native access capability"));
    }
    struct Library(*mut libc::c_void);
    impl Drop for Library {
        fn drop(&mut self) {
            // SAFETY: this is the live dlopen handle owned by this guard.
            unsafe { libc::dlclose(self.0) };
        }
    }
    let library = Library(handle);
    // SAFETY: the NUL-terminated symbol name is valid for this live handle.
    let address = unsafe { libc::dlsym(library.0, symbol.as_ptr()) };
    if address.is_null() {
        return Err(prerequisite(&format!(
            "{} lacks {}",
            path.display(),
            symbol.to_string_lossy()
        )));
    }
    let mut provider = std::mem::MaybeUninit::<libc::Dl_info>::zeroed();
    // SAFETY: dlsym returned a live address and the output buffer is writable.
    if unsafe { libc::dladdr(address, provider.as_mut_ptr()) } == 0 {
        return Err(prerequisite("cannot identify native capability provider"));
    }
    // SAFETY: successful dladdr initialized the structure.
    let provider = unsafe { provider.assume_init() };
    if provider.dli_fname.is_null() {
        return Err(prerequisite("native capability provider has no path"));
    }
    // SAFETY: dladdr owns this NUL-terminated pathname for the live handle.
    let provider_name = unsafe { CStr::from_ptr(provider.dli_fname) };
    let provider_path = Path::new(std::ffi::OsStr::from_bytes(provider_name.to_bytes()));
    let provider_canonical = fs::canonicalize(provider_path)
        .map_err(|_| prerequisite("cannot resolve native capability provider"))?;
    if provider_canonical != canonical {
        return Err(prerequisite("native capability provider differs"));
    }
    trusted_library(&provider_canonical)?;
    // These versioned host capability functions have this documented C ABI.
    let version: unsafe extern "C" fn() -> libc::c_uint = unsafe { std::mem::transmute(address) };
    if unsafe { version() } != expected {
        return Err(prerequisite("native access capability version differs"));
    }
    Ok(())
}

fn require_puffs() -> AppResult<()> {
    require_marker(
        Path::new("/usr/lib/libpuffs.so.2"),
        c"puffs_access_abi_version",
        1,
    )
}

pub(crate) fn require_perfuse() -> AppResult<()> {
    require_puffs()?;
    require_marker(
        Path::new("/usr/lib/libperfuse.so.0"),
        c"perfuse_access_cache_version",
        1,
    )
}

pub(crate) fn require_refuse() -> AppResult<()> {
    require_puffs()?;
    require_marker(
        Path::new("/usr/lib/librefuse.so.2"),
        c"refuse_access_policy_version",
        2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_library_without_the_required_capability_is_rejected() {
        let error = require_marker(
            Path::new("/usr/lib/librefuse.so.2"),
            c"lurker_deliberately_absent_native_access_capability",
            1,
        )
        .unwrap_err();
        assert!(
            error.message.contains("lacks lurker_deliberately_absent"),
            "{}",
            error.message
        );
    }

    #[test]
    fn a_symbol_from_a_native_dependency_is_not_provider_qualification() {
        let error = require_marker(Path::new("/usr/lib/librefuse.so.2"), c"getpid", 1).unwrap_err();
        assert!(
            error.message.contains("capability provider differs"),
            "{}",
            error.message
        );
    }

    #[test]
    fn a_private_or_writable_file_cannot_be_loaded_as_a_native_library() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("fake.so");
        fs::write(&file, b"\x7fELFnot-a-library").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(trusted_library(&file).is_err());
    }
}
