//! Keep native BSD file creation anchored to the directories it acquired.
use super::{unique_id, Identity};
use crate::{AppError, AppResult};
use std::ffi::{CStr, CString, OsStr};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const IMAGE: &CStr = c"image";
#[cfg(target_os = "netbsd")]
const PARAMS: &CStr = c"image.cgdparams";

pub(super) struct FileStage {
    path: PathBuf,
    name: CString,
    parent: File,
    directory: File,
    image: Option<Identity>,
    #[cfg(target_os = "netbsd")]
    params: Option<Identity>,
}

fn open_at(directory: &File, name: &CStr, flags: libc::c_int) -> io::Result<File> {
    // SAFETY: directory and name remain live for the call. The fourth
    // argument supplies a private creation mode when O_CREAT is set.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a new descriptor owned by this call.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn entry_name(name: &OsStr) -> AppResult<CString> {
    CString::new(name.as_bytes()).map_err(|_| AppError::new("BSD filename contains a NUL byte."))
}

fn stat_entry(directory: &File, name: &CStr) -> io::Result<libc::stat> {
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the descriptor and name are live and the buffer can hold a stat.
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful fstatat initialized the metadata.
    Ok(unsafe { metadata.assume_init() })
}

impl FileStage {
    pub(super) fn create(parent_path: &Path) -> AppResult<Self> {
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(parent_path)
            .map_err(|e| AppError::io("Open BSD destination directory", e))?;
        let name = format!(".lurker-bsd-{}", unique_id());
        let path = parent_path.join(&name);
        let name = entry_name(OsStr::new(&name))?;
        // SAFETY: the parent descriptor and the single entry name are live.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            return Err(AppError::io(
                "Create private BSD staging directory",
                io::Error::last_os_error(),
            ));
        }
        let directory = open_at(&parent, &name, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(|e| AppError::io("Open BSD staging directory", e))?;
        let metadata = directory
            .metadata()
            .map_err(|e| AppError::io("Inspect BSD staging directory", e))?;
        if !metadata.is_dir()
            || metadata.uid() != crate::effective_uid()
            || metadata.mode() & 0o077 != 0
        {
            return Err(AppError::new(
                "BSD staging directory must be private and owned by the current user.",
            ));
        }
        Ok(Self {
            path,
            name,
            parent,
            directory,
            image: None,
            #[cfg(target_os = "netbsd")]
            params: None,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn source(&self) -> PathBuf {
        self.path.join("image")
    }

    pub(super) fn create_image(&mut self) -> AppResult<File> {
        let file = open_at(
            &self.directory,
            IMAGE,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )
        .map_err(|e| AppError::io("Create staged BSD container", e))?;
        self.image = Some(Identity::from_file(&file)?);
        Ok(file)
    }

    pub(super) fn verify_path(&self) -> AppResult<()> {
        for (path, directory) in [
            (self.path.parent().unwrap(), &self.parent),
            (&self.path, &self.directory),
        ] {
            let identity = Identity::from_file(directory)?;
            let metadata = fs::symlink_metadata(path)
                .map_err(|e| AppError::io("Check BSD staging directory path", e))?;
            if !metadata.is_dir()
                || metadata.dev() != identity.dev
                || metadata.ino() != identity.ino
            {
                return Err(AppError::new(
                    "BSD staging or destination directory changed.",
                ));
            }
        }
        Ok(())
    }

    #[cfg(target_os = "netbsd")]
    pub(super) fn capture_params(&mut self) -> AppResult<File> {
        let file = open_at(&self.directory, PARAMS, libc::O_RDWR | libc::O_NONBLOCK)
            .map_err(|e| AppError::io("Open staged CGD recovery parameters", e))?;
        let metadata = file
            .metadata()
            .map_err(|e| AppError::io("Inspect staged CGD recovery parameters", e))?;
        if !metadata.is_file() {
            return Err(AppError::new(
                "CGD recovery parameters must be a regular file.",
            ));
        }
        self.params = Some(Identity::from_metadata(&metadata)?);
        Ok(file)
    }

    #[cfg(any(not(target_os = "netbsd"), test))]
    pub(super) fn publish(&self, target: &Path, force: bool) -> AppResult<()> {
        self.verify_path()?;
        self.publish_entry(IMAGE, self.image.as_ref(), target, force)
    }

    fn publish_entry(
        &self,
        from: &CStr,
        identity: Option<&Identity>,
        target: &Path,
        force: bool,
    ) -> AppResult<()> {
        if target.parent() != self.path.parent() {
            return Err(AppError::new("BSD publication target changed directories."));
        }
        if !matches_file(&self.directory, from, identity) {
            return Err(AppError::new("Staged BSD publication source changed."));
        }
        let destination = entry_name(
            target
                .file_name()
                .ok_or_else(|| AppError::new("BSD publication needs a filename."))?,
        )?;
        if force {
            match stat_entry(&self.parent, &destination) {
                Ok(m) if m.st_mode & libc::S_IFMT == libc::S_IFREG => (),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Ok(_) => {
                    return Err(AppError::new(
                        "BSD publication target is no longer a regular file.",
                    ))
                }
                Err(e) => return Err(AppError::io("Check BSD publication target", e)),
            }
        }
        let source_fd = self.directory.as_raw_fd();
        let destination_fd = self.parent.as_raw_fd();
        // SAFETY: both descriptors and both NUL-terminated names remain live.
        let result = unsafe {
            if force {
                libc::renameat(
                    source_fd,
                    from.as_ptr(),
                    destination_fd,
                    destination.as_ptr(),
                )
            } else {
                libc::linkat(
                    source_fd,
                    from.as_ptr(),
                    destination_fd,
                    destination.as_ptr(),
                    0,
                )
            }
        };
        if result != 0 {
            return Err(AppError::io(
                "Publish BSD container file",
                io::Error::last_os_error(),
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "netbsd")]
    pub(super) fn publish_cgd(&self, target: &Path, params: &Path) -> AppResult<()> {
        self.publish_cgd_pair(target, params, || self.parent.sync_all())
    }

    #[cfg(target_os = "netbsd")]
    fn publish_cgd_pair(
        &self,
        target: &Path,
        params: &Path,
        sync_parameters: impl FnOnce() -> io::Result<()>,
    ) -> AppResult<()> {
        self.verify_path()?;
        let file = open_at(&self.directory, PARAMS, libc::O_RDONLY | libc::O_NONBLOCK)
            .map_err(|e| AppError::io("Open staged CGD recovery parameters", e))?;
        file.sync_all()
            .map_err(|e| AppError::io("Sync CGD recovery parameters", e))?;
        let name = entry_name(
            params
                .file_name()
                .ok_or_else(|| AppError::new("CGD recovery parameters need a filename."))?,
        )?;
        self.publish_entry(PARAMS, self.params.as_ref(), params, false)?;
        // The second link exposes the image. Persist its required sidecar's
        // directory entry first so the image never depends on an unsynced link.
        let published = sync_parameters()
            .map_err(|e| AppError::io("Sync published CGD recovery parameters", e))
            .and_then(|()| self.publish_entry(IMAGE, self.image.as_ref(), target, false));
        if let Err(error) = published {
            if matches_file(&self.parent, &name, self.params.as_ref()) {
                remove_at(&self.parent, &name, 0);
                let _ = self.parent.sync_all();
            }
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn finish(&self, published: AppResult<()>) -> AppResult<()> {
        if published.is_ok() {
            // Keep staged data if the kernel cannot confirm publication.
            self.parent
                .sync_all()
                .map_err(|e| AppError::io("Sync BSD destination directory", e))?;
        }
        self.discard();
        published
    }

    pub(super) fn discard(&self) {
        if matches_file(&self.directory, IMAGE, self.image.as_ref()) {
            remove_at(&self.directory, IMAGE, 0);
        }
        #[cfg(target_os = "netbsd")]
        if matches_file(&self.directory, PARAMS, self.params.as_ref()) {
            remove_at(&self.directory, PARAMS, 0);
        }
        // Compare the current entry within the pinned parent before removing
        // the empty directory. Native unlinkat never traverses its contents.
        let current = stat_entry(&self.parent, &self.name);
        if self.directory.metadata().is_ok_and(|owned| {
            current.is_ok_and(|m| {
                m.st_mode & libc::S_IFMT == libc::S_IFDIR
                    && m.st_dev as u64 == owned.dev()
                    && m.st_ino as u64 == owned.ino()
            })
        }) {
            remove_at(&self.parent, &self.name, libc::AT_REMOVEDIR);
        }
    }
}

fn matches_file(directory: &File, name: &CStr, identity: Option<&Identity>) -> bool {
    identity.is_some_and(|identity| {
        stat_entry(directory, name).is_ok_and(|m| {
            m.st_mode & libc::S_IFMT == libc::S_IFREG
                && m.st_dev as u64 == identity.dev
                && m.st_ino as u64 == identity.ino
        })
    })
}

fn remove_at(directory: &File, name: &CStr, flags: libc::c_int) {
    // SAFETY: directory and the single NUL-terminated entry name are live.
    unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), flags) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn a_moved_stage_preserves_a_different_directory_at_its_former_path() {
        let root = std::env::temp_dir().join(format!("lurker-stage-test-{}", unique_id()));
        fs::create_dir(&root).unwrap();
        let mut stage = FileStage::create(&root).unwrap();
        assert_eq!(fs::metadata(stage.path()).unwrap().mode() & 0o077, 0);
        stage.create_image().unwrap().write_all(b"staged").unwrap();

        let moved = root.join("moved-stage");
        fs::rename(stage.path(), &moved).unwrap();
        fs::create_dir(stage.path()).unwrap();
        fs::write(stage.source(), "other contents").unwrap();
        let result = stage.publish(&root.join("target"), false);
        stage.discard();

        assert!(result.is_err());
        assert!(!root.join("target").exists());
        assert!(!moved.join("image").exists());
        assert_eq!(
            fs::read_to_string(stage.source()).unwrap(),
            "other contents"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "netbsd")]
    #[test]
    fn cgd_rollback_keeps_the_existing_destination() {
        let root = std::env::temp_dir().join(format!("lurker-cgd-publish-{}", unique_id()));
        fs::create_dir(&root).unwrap();
        let mut stage = FileStage::create(&root).unwrap();
        stage
            .create_image()
            .unwrap()
            .write_all(b"new data")
            .unwrap();
        fs::write(stage.path().join("image.cgdparams"), "new parameters").unwrap();
        stage.capture_params().unwrap();
        let image = root.join("target");
        let params = root.join("target.cgdparams");
        fs::write(&image, "existing data").unwrap();
        assert!(stage.finish(stage.publish_cgd(&image, &params)).is_err());
        assert_eq!(fs::read_to_string(&image).unwrap(), "existing data");
        assert!(!params.exists());
        assert!(!stage.path().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "netbsd")]
    #[test]
    fn cgd_does_not_publish_the_image_if_syncing_the_sidecar_name_fails() {
        let root = std::env::temp_dir().join(format!("lurker-cgd-sync-{}", unique_id()));
        fs::create_dir(&root).unwrap();
        let mut stage = FileStage::create(&root).unwrap();
        stage
            .create_image()
            .unwrap()
            .write_all(b"new data")
            .unwrap();
        fs::write(stage.path().join("image.cgdparams"), "new parameters").unwrap();
        stage.capture_params().unwrap();
        let image = root.join("target");
        let params = root.join("target.cgdparams");

        let publication = stage.publish_cgd_pair(&image, &params, || {
            assert_eq!(fs::read_to_string(&params).unwrap(), "new parameters");
            assert!(!image.exists());
            Err(io::Error::other("injected directory sync failure"))
        });
        let error = stage.finish(publication).unwrap_err();
        assert!(error
            .message
            .contains("Sync published CGD recovery parameters"));
        assert!(!image.exists());
        assert!(!params.exists());
        assert!(!stage.path().exists());
        fs::remove_dir_all(root).unwrap();
    }
}
