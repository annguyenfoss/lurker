#![cfg(target_os = "linux")]
//! Full userspace stack, run as an ordinary user on fresh disposable files:
//! LURKER_USERSPACE_DIR=$PWD/target/release/userspace cargo test -p lurker-core
//! --test userspace_workflows -- --ignored --nocapture --test-threads=1
use lurker_core::*;
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PASSWORD: &str = "  Userspace 密碼 integration passphrase  ";
struct Fixture {
    root: PathBuf,
    old_runtime: Option<std::ffi::OsString>,
    old_path: Option<std::ffi::OsString>,
}
impl Fixture {
    fn new() -> Self {
        assert_ne!(effective_uid(), 0, "Run this suite without sudo");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "lurker-userspace-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let runtime = root.join("runtime");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let old_runtime = std::env::var_os("XDG_RUNTIME_DIR");
        let old_path = std::env::var_os("PATH");
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
        let deny = root.join("deny-elevation");
        fs::create_dir(&deny).unwrap();
        for name in [
            "sudo",
            "doas",
            "pkexec",
            "mount",
            "losetup",
            "dmsetup",
            "cryptsetup",
        ] {
            let file = deny.join(name);
            fs::write(
                &file,
                "#!/bin/sh\necho 'Forbidden elevation/kernel tool invoked' >&2\nexit 91\n",
            )
            .unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut path = vec![deny];
        path.extend(std::env::split_paths(
            old_path.as_deref().unwrap_or_default(),
        ));
        std::env::set_var("PATH", std::env::join_paths(path).unwrap());
        Self {
            root,
            old_runtime,
            old_path,
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
    fn active(&self, source: &Path) -> ActiveVolume {
        list_active_volumes()
            .unwrap()
            .into_iter()
            .find(|v| v.source_path.as_deref() == Some(source))
            .expect("active userspace volume missing")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let mut clean = true;
        if let Ok(volumes) = list_active_volumes() {
            for v in volumes {
                if v.privilege_mode == PrivilegeMode::Unprivileged
                    && v.source_path
                        .as_ref()
                        .is_some_and(|p| p.starts_with(&self.root))
                {
                    let response = run_buffered(unmount(v.source_path.as_ref().unwrap()));
                    if !response.ok {
                        eprintln!(
                            "Retaining busy test fixture {}: {:?}",
                            self.root.display(),
                            response.error
                        );
                        clean = false;
                    }
                }
            }
        } else {
            clean = false;
        }
        match &self.old_runtime {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
        match &self.old_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
        let prefix = format!("{}/", self.root.display());
        let mounts = fs::read_to_string("/proc/self/mountinfo").unwrap();
        if clean
            && !mounts.lines().any(|line| {
                line.split_whitespace()
                    .nth(4)
                    .is_some_and(|p| p.starts_with(&prefix))
            })
        {
            let _ = fs::remove_dir_all(&self.root);
        } else {
            eprintln!("Retaining live test fixture {}", self.root.display());
        }
    }
}
fn create(source: &Path, cipher: CreateCipher, force: bool) -> CommandAction {
    CommandAction::Create(CreateCommand {
        privilege_mode: PrivilegeMode::Unprivileged,
        target: source.into(),
        size_gb: Some("0.125".into()),
        force,
        source_kind: SourceKind::File,
        volume_type: VolumeType::Veracrypt,
        cipher,
        passphrase: Some(PASSWORD.into()),
    })
}
fn mount(source: &Path, at: &Path, readonly: bool) -> CommandAction {
    CommandAction::Mount(MountCommand {
        privilege_mode: PrivilegeMode::Unprivileged,
        source: source.into(),
        mountpoint: at.into(),
        volume_type: VolumeType::Auto,
        tag: None,
        passphrase: Some(PASSWORD.into()),
        readonly,
    })
}
fn unmount(target: &Path) -> CommandAction {
    CommandAction::Unmount(UnmountCommand {
        privilege_mode: PrivilegeMode::Unprivileged,
        target: target.into(),
        tag: None,
        volume_type: VolumeType::Auto,
    })
}
fn ok(command: CommandAction) {
    let response = run_buffered(command);
    assert!(response.ok, "{:?}", response.error);
    assert!(!format!("{response:?}").contains(PASSWORD));
}
fn bad(command: CommandAction) -> String {
    let response = run_buffered(command);
    assert!(!response.ok, "unexpected success");
    assert!(!format!("{response:?}").contains(PASSWORD));
    response.error.unwrap()
}
fn tool(program: &str, args: &[&std::ffi::OsStr]) -> std::process::Output {
    let result = Command::new(program).args(args).output().unwrap();
    assert!(
        result.status.success(),
        "{program}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

#[test]
#[ignore = "requires /dev/fuse and the built userspace helpers; run WITHOUT sudo"]
fn real_userspace_veracrypt_workflows() {
    let f = Fixture::new();
    for cipher in [
        CreateCipher::Aes,
        CreateCipher::Serpent,
        CreateCipher::Twofish,
    ] {
        eprintln!("USERSPACE MATRIX: {cipher}");
        let source = f.path(&format!("{cipher} vault 密碼.hc"));
        let at = f.path("mount with spaces");
        ok(create(&source, cipher, false));
        assert_eq!(
            fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::metadata(&source).unwrap().len(), 128 * 1024 * 1024);
        let before = fs::read(&source).unwrap();
        assert!(bad(create(&source, cipher, false)).contains("already exists"));
        assert_eq!(fs::read(&source).unwrap(), before);
        ok(mount(&source, &at, false));
        let volume = f.active(&source);
        assert_eq!(volume.privilege_mode, PrivilegeMode::Unprivileged);
        assert_eq!(volume.filesystem_type.as_deref(), Some("btrfs"));
        assert!(!volume.readonly);
        assert_eq!(fs::metadata(&at).unwrap().uid(), effective_uid());
        fs::create_dir(at.join("folder")).unwrap();
        fs::write(at.join("folder/data"), b"persistent userspace data").unwrap();
        assert_eq!(
            fs::metadata(at.join("folder/data")).unwrap().uid(),
            effective_uid()
        );
        fs::hard_link(at.join("folder/data"), at.join("hard-link")).unwrap();
        symlink("folder/data", at.join("symbolic-link")).unwrap();
        assert_eq!(
            fs::read(at.join("symbolic-link")).unwrap(),
            b"persistent userspace data"
        );
        fs::rename(at.join("hard-link"), at.join("renamed")).unwrap();
        let mut sparse = fs::File::create(at.join("sparse")).unwrap();
        sparse.seek(SeekFrom::Start(1024 * 1024)).unwrap();
        sparse.write_all(b"tail").unwrap();
        sparse.sync_all().unwrap();
        drop(sparse);
        fs::write(at.join("compressed"), vec![b'z'; 2 * 1024 * 1024]).unwrap();
        let mut permission = fs::File::create(at.join("private")).unwrap();
        permission.write_all(b"private").unwrap();
        permission.sync_all().unwrap();
        drop(permission);
        fs::set_permissions(at.join("private"), fs::Permissions::from_mode(0o000)).unwrap();
        assert!(fs::read(at.join("private")).is_err());
        fs::set_permissions(at.join("private"), fs::Permissions::from_mode(0o600)).unwrap();
        bad(mount(&source, &f.path("duplicate"), false));
        bad(create(&source, cipher, true));
        bad(create(&source, cipher, true).with_privilege_mode(PrivilegeMode::Privileged));
        bad(mount(&source, &f.path("duplicate"), false)
            .with_privilege_mode(PrivilegeMode::Privileged));
        assert_eq!(
            fs::read(at.join("folder/data")).unwrap(),
            b"persistent userspace data"
        );
        let mut holder = Command::new("sleep")
            .arg("30")
            .current_dir(&at)
            .spawn()
            .unwrap();
        let busy = run_buffered(unmount(&source));
        holder.kill().unwrap();
        holder.wait().unwrap();
        assert!(!busy.ok, "busy volume must stay mounted");
        assert!(at.join("folder/data").exists());
        ok(unmount(&source));
        assert!(!volume.mapper_path.exists());
        assert!(
            !at.exists(),
            "automatically created mountpoint should be removed"
        );
        ok(mount(&source, &at, true));
        assert!(f.active(&source).readonly);
        assert_eq!(
            fs::read(at.join("renamed")).unwrap(),
            b"persistent userspace data"
        );
        assert_eq!(fs::read(at.join("sparse")).unwrap().len(), 1024 * 1024 + 4);
        assert!(fs::write(at.join("blocked"), b"no").is_err());
        let raw = f.active(&source).mapper_path;
        let raw_write = fs::OpenOptions::new()
            .write(true)
            .open(&raw)
            .and_then(|mut file| file.write_all(b"x"));
        assert!(raw_write.is_err(), "crypto layer must also be read-only");
        ok(unmount(&raw));
        // Symlink sources resolve to the same backing identity.
        let alias = f.path("source alias");
        symlink(&source, &alias).unwrap();
        ok(mount(&alias, &at, false));
        ok(unmount(&at));
        fs::remove_file(alias).unwrap();
        if cipher == CreateCipher::Aes {
            ok(mount(&source, &at, false));
            let raw = f.active(&source).mapper_path;
            let state_root = f
                .path("runtime")
                .join(format!("lurker-userspace-{}", effective_uid()));
            let session_file = fs::read_dir(state_root)
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.path().join("session.json"))
                .find(|p| p.is_file())
                .unwrap();
            let session: serde_json::Value =
                serde_json::from_slice(&fs::read(session_file).unwrap()).unwrap();
            let pid = session["process"]["pid"].as_u64().unwrap();
            tool("fusermount3", &["-u".as_ref(), at.as_os_str()]);
            let deadline = Instant::now() + Duration::from_secs(30);
            while fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .is_some_and(|s| !s.contains(") Z "))
            {
                assert!(Instant::now() < deadline, "filesystem did not drain");
                std::thread::sleep(Duration::from_millis(25));
            }
            let dump = tool(
                "btrfs",
                &[
                    "inspect-internal".as_ref(),
                    "dump-tree".as_ref(),
                    "-t".as_ref(),
                    "5".as_ref(),
                    raw.as_os_str(),
                ],
            );
            assert!(
                String::from_utf8_lossy(&dump.stdout).contains("compression 3"),
                "zstd-compressed extents were not found"
            );
            ok(unmount(&source));
            let mut wrong = mount(&source, &at, false);
            if let CommandAction::Mount(ref mut c) = wrong {
                c.passphrase = Some("wrong password".into());
            }
            bad(wrong);
            assert!(!list_active_volumes()
                .unwrap()
                .iter()
                .any(|v| v.source_path.as_deref() == Some(&source)));
            let before = fs::read(&source).unwrap();
            let mut failed = create(&source, cipher, true);
            if let CommandAction::Create(ref mut c) = failed {
                c.passphrase = Some("invalid\npassword".into());
            }
            bad(failed);
            assert_eq!(fs::read(&source).unwrap(), before);
            ok(create(&source, cipher, true));
            ok(mount(&source, &at, false));
            assert!(!at.join("folder").exists());
            ok(unmount(&source));
        }
        // Verify the standard kernel tool accepts our unmodified container header
        // without creating a mapper or acquiring administrator privileges.
        let mut child = Command::new("/usr/bin/cryptsetup")
            .args([
                "open",
                "--type",
                "tcrypt",
                "--veracrypt",
                "--test-passphrase",
                "--batch-mode",
            ])
            .arg(&source)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{PASSWORD}\n").as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        eprintln!("PASS {cipher}: user-owned Btrfs, POSIX operations, busy/duplicate guards, readonly at both layers, remount and standard header compatibility");
    }
    assert!(list_active_volumes().unwrap().iter().all(|v| !v
        .source_path
        .as_ref()
        .is_some_and(|p| p.starts_with(&f.root))));
}

#[test]
#[ignore = "requires userspace helpers, mkfs.exfat, mkfs.fat and mkfs.ext4; run WITHOUT sudo"]
fn external_filesystems_and_failure_recovery() {
    let f = Fixture::new();
    let crypto =
        PathBuf::from(std::env::var_os("LURKER_USERSPACE_DIR").expect("set LURKER_USERSPACE_DIR"))
            .join("lurker-veracrypt");
    let native = f.path("native");
    fs::create_dir(&native).unwrap();
    let at = f.path("external mount");
    for filesystem in ["exfat", "vfat", "ext4"] {
        let source = f.path(&format!("external {filesystem}.hc"));
        for args in [
            vec![
                "-t",
                "--create",
                "--force",
                "--filesystem=none",
                "--volume-type=normal",
                "--encryption=AES",
                "--hash=SHA-256",
                "--size=64M",
                "--random-source=/dev/urandom",
                "--stdin",
                "--non-interactive",
                "--pim=0",
                "-k",
                "",
            ],
            vec![
                "-t",
                "--mount",
                "--filesystem=none",
                "--mount-options=nokernelcrypto",
                "--stdin",
                "--non-interactive",
                "--pim=0",
                "-k",
                "",
                "--protect-hidden=no",
            ],
        ] {
            let mut child = Command::new(&crypto)
                .args(args)
                .arg(&source)
                .env("TMPDIR", &native)
                .env("LC_ALL", "C.UTF-8")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("{PASSWORD}\n").as_bytes())
                .unwrap();
            let result = child.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        let raw_dir = native.join(".veracrypt_aux_mnt1");
        let raw = raw_dir.join("volume");
        match filesystem {
            "exfat" => {
                tool("mkfs.exfat", &[raw.as_os_str()]);
            }
            "vfat" => {
                tool("mkfs.fat", &[raw.as_os_str()]);
            }
            "ext4" => {
                tool(
                    "mkfs.ext4",
                    &[
                        "-q".as_ref(),
                        "-E".as_ref(),
                        format!("root_owner={}:{}", effective_uid(), unsafe {
                            libc::getegid()
                        })
                        .as_ref(),
                        raw.as_os_str(),
                    ],
                );
            }
            _ => unreachable!(),
        }
        fs::File::open(&raw).unwrap().sync_all().unwrap();
        tool("fusermount3", &["-u".as_ref(), raw_dir.as_os_str()]);
        fs::remove_dir(&raw_dir).unwrap();
        ok(mount(&source, &at, false));
        assert_eq!(
            f.active(&source).filesystem_type.as_deref(),
            Some(filesystem)
        );
        fs::write(at.join("external-data"), filesystem).unwrap();
        assert_eq!(
            fs::metadata(at.join("external-data")).unwrap().uid(),
            effective_uid()
        );
        ok(unmount(&source));
        ok(mount(&source, &at, true));
        assert_eq!(
            fs::read_to_string(at.join("external-data")).unwrap(),
            filesystem
        );
        assert!(fs::write(at.join("blocked"), b"x").is_err());
        ok(unmount(&at));
        eprintln!("PASS external {filesystem}: SHA-256 header, automatic type, user writes, persistence and readonly");
    }
    let source = f.path("recovery.hc");
    ok(create(&source, CreateCipher::Aes, false));
    let before = fs::read(&source).unwrap();
    let failed_tools = f.path("failed-mkfs");
    fs::create_dir(&failed_tools).unwrap();
    let script = failed_tools.join("mkfs.btrfs");
    fs::write(
        &script,
        "#!/bin/sh\necho 'injected mkfs failure' >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::var_os("PATH").unwrap();
    std::env::set_var(
        "PATH",
        std::env::join_paths(std::iter::once(failed_tools).chain(std::env::split_paths(&path)))
            .unwrap(),
    );
    let failure = bad(create(&source, CreateCipher::Aes, true));
    std::env::set_var("PATH", path);
    assert!(failure.contains("injected mkfs failure"), "{failure}");
    assert_eq!(fs::read(&source).unwrap(), before);
    assert!(!list_active_volumes().unwrap().iter().any(|v| v
        .source_path
        .as_ref()
        .is_some_and(|p| p.starts_with(&f.root))));
    // Mountpoint identity guards must reject stale/replaced mounts.
    ok(mount(&source, &at, false));
    let session_file = fs::read_dir(
        f.path("runtime")
            .join(format!("lurker-userspace-{}", effective_uid())),
    )
    .unwrap()
    .filter_map(Result::ok)
    .map(|e| e.path().join("session.json"))
    .find(|p| p.is_file())
    .unwrap();
    let original = fs::read(&session_file).unwrap();
    let mut session: serde_json::Value = serde_json::from_slice(&original).unwrap();
    session["filesystem_mount_id"] = 0.into();
    fs::write(&session_file, serde_json::to_vec(&session).unwrap()).unwrap();
    assert!(bad(unmount(&source)).contains("different mount"));
    assert!(at.is_dir());
    fs::write(&session_file, original).unwrap();
    // A crashed driver must recover and acknowledge a clean journal shutdown
    // before the still-open crypto image is released.
    let durable = at.join("survives-driver-crash");
    fs::write(&durable, b"synced before crash").unwrap();
    fs::File::open(&durable).unwrap().sync_all().unwrap();
    let current: serde_json::Value =
        serde_json::from_slice(&fs::read(&session_file).unwrap()).unwrap();
    let pid = current["process"]["pid"].as_u64().unwrap() as i32;
    assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
    ok(unmount(&source));
    ok(mount(&source, &at, false));
    assert_eq!(fs::read(&durable).unwrap(), b"synced before crash");
    let volume = f.active(&source);
    // An open raw image prevents closing crypto, after the filesystem has drained.
    let raw_handle = fs::File::open(&volume.mapper_path).unwrap();
    assert!(bad(unmount(&source)).contains("busy"));
    assert!(f.active(&source).mountpoint.is_none());
    drop(raw_handle);
    // Recovery does not depend on either backend executable remaining installed.
    let tools = std::env::var_os("LURKER_USERSPACE_DIR");
    std::env::set_var("LURKER_USERSPACE_DIR", f.path("missing-tools"));
    ok(unmount(&source));
    match tools {
        Some(v) => std::env::set_var("LURKER_USERSPACE_DIR", v),
        None => std::env::remove_var("LURKER_USERSPACE_DIR"),
    }
    eprintln!("PASS: failed forced replacement preserves old data; stale mount identity is refused; busy raw image, driver-crash and missing-helper recovery");
}
