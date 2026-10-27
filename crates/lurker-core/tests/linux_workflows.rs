#![cfg(target_os = "linux")]

//! Real Linux crypto/filesystem regression suite. Run serially as root:
//! cargo test -p lurker-core --test linux_workflows -- --ignored --nocapture
//! Uses only fresh temporary images and loop devices allocated from those images.
use lurker_core::*;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output as ProcessOutput, Stdio};

struct Sandbox {
    dir: PathBuf,
    mappers: Vec<PathBuf>,
    loops: Vec<(String, PathBuf)>,
    raw_mounts: Vec<PathBuf>,
}
impl Sandbox {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("lurker-integration-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        Self {
            dir,
            mappers: vec![],
            loops: vec![],
            raw_mounts: vec![],
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
    fn mapper(&mut self, mount: &Path) -> PathBuf {
        let volume = list_active_volumes()
            .unwrap()
            .into_iter()
            .find(|v| v.mountpoint.as_deref() == Some(mount))
            .expect("volume missing from active list");
        assert_eq!(volume.filesystem_type.as_deref(), Some("btrfs"));
        assert!(volume.mapper_name.starts_with("lurker_"));
        if !self.mappers.contains(&volume.mapper_path) {
            self.mappers.push(volume.mapper_path.clone());
        }
        volume.mapper_path
    }
    fn owns_loop(&self, device: &Path) -> bool {
        self.loops
            .iter()
            .any(|(dev, backing)| Path::new(dev) == device && loop_backing_matches(device, backing))
    }
    fn loop_device(&mut self, name: &str) -> PathBuf {
        let file = fs::File::create(self.path(name)).unwrap();
        file.set_len(256 * 1024 * 1024).unwrap();
        let out = tool(
            "losetup",
            &["--find", "--show", self.path(name).to_str().unwrap()],
        );
        let dev = String::from_utf8(out.stdout).unwrap().trim().to_owned();
        self.loops.push((dev.clone(), self.path(name)));
        PathBuf::from(dev)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        // Never touch pre-existing mappings; only names captured at our own mountpoints.
        for mapper in &self.mappers {
            let _ = Command::new("umount").arg(mapper).output();
            let _ = Command::new("cryptsetup").arg("close").arg(mapper).output();
        }
        for mount in &self.raw_mounts {
            let _ = Command::new("umount").arg(mount).output();
        }
        for (dev, backing) in &self.loops {
            if loop_backing_matches(Path::new(dev), backing) {
                let _ = Command::new("losetup").arg("-d").arg(dev).output();
            }
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn loop_backing_matches(device: &Path, backing: &Path) -> bool {
    let Some(name) = device.file_name() else {
        return false;
    };
    let path = Path::new("/sys/class/block")
        .join(name)
        .join("loop/backing_file");
    fs::read_to_string(path).ok().is_some_and(|value| {
        fs::canonicalize(value.trim_end_matches('\n')).ok() == fs::canonicalize(backing).ok()
            && backing.exists()
    })
}

fn tool(program: &str, args: &[&str]) -> ProcessOutput {
    let result = Command::new(program).args(args).output().unwrap();
    assert!(
        result.status.success(),
        "{program} {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}
fn ok(action: CommandAction) -> OperationResponse {
    let response = run_buffered(action);
    assert!(response.ok, "{response:?}");
    response
}
fn bad(action: CommandAction) -> OperationResponse {
    let response = run_buffered(action);
    assert!(!response.ok, "unexpected success: {response:?}");
    response
}
const PASSWORD: &str = "Lurker integration p@ssphrase 你好 123456789!";
fn create(target: &Path, kind: SourceKind, ty: VolumeType, cipher: CreateCipher) -> CommandAction {
    CommandAction::Create(CreateCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        target: target.into(),
        size_gb: (kind == SourceKind::File).then(|| "0.25".into()),
        force: kind == SourceKind::Block,
        source_kind: kind,
        volume_type: ty,
        cipher,
        passphrase: Some(PASSWORD.into()),
    })
}
fn mount(
    source: &Path,
    at: &Path,
    ty: VolumeType,
    readonly: bool,
    tag: Option<&str>,
) -> CommandAction {
    CommandAction::Mount(MountCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        source: source.into(),
        mountpoint: at.into(),
        volume_type: ty,
        readonly,
        tag: tag.map(String::from),
        passphrase: Some(PASSWORD.into()),
    })
}
fn unmount(target: &Path, ty: VolumeType, tag: Option<&str>) -> CommandAction {
    CommandAction::Unmount(UnmountCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        target: target.into(),
        volume_type: ty,
        tag: tag.map(String::from),
    })
}

#[test]
#[ignore = "requires root, Linux device-mapper, loop devices, cryptsetup, VeraCrypt and btrfs-progs"]
fn real_linux_workflow_matrix() {
    assert!(
        probe_system().unwrap().is_root,
        "run this ignored test as root"
    );
    let mut sandbox = Sandbox::new();
    let at = sandbox.path("mount with spaces");
    let other = sandbox.path("second mount");
    fs::create_dir(&at).unwrap();
    fs::create_dir(&other).unwrap();
    let mut cases = 0;
    for ty in [VolumeType::Luks, VolumeType::Veracrypt] {
        for cipher in [
            CreateCipher::Aes,
            CreateCipher::Serpent,
            CreateCipher::Twofish,
        ] {
            for kind in [SourceKind::File, SourceKind::Block] {
                let name = format!("{ty}-{cipher}-{kind:?} vault.img");
                if let Ok(filter) = std::env::var("LURKER_TEST_CASE") {
                    if !name.starts_with(&filter) {
                        continue;
                    }
                }
                cases += 1;
                let source = if kind == SourceKind::Block {
                    sandbox.loop_device(&name)
                } else {
                    sandbox.path(&name)
                };
                eprintln!(
                    "MATRIX create/mount/persistence/readonly/unmount: {ty} {cipher} {kind:?}"
                );
                if kind == SourceKind::Block {
                    let mut action = create(&source, kind, ty, cipher);
                    if let CommandAction::Create(ref mut c) = action {
                        c.force = false;
                    }
                    bad(action);
                }
                ok(create(&source, kind, ty, cipher));
                if kind == SourceKind::File {
                    assert_eq!(
                        fs::metadata(&source).unwrap().permissions().mode() & 0o777,
                        0o600
                    );
                    assert_eq!(fs::metadata(&source).unwrap().len(), 256 * 1024 * 1024);
                    bad(create(&source, kind, ty, cipher)); // no implicit overwrite
                }
                if ty == VolumeType::Luks {
                    let dump = tool("cryptsetup", &["luksDump", source.to_str().unwrap()]);
                    let dump = String::from_utf8_lossy(&dump.stdout);
                    assert!(dump.contains(&format!("{cipher}-xts-plain64")), "{dump}");
                    assert!(dump.contains("argon2id"));
                }
                // Explicit type mismatch must fail before opening anything.
                let wrong = if ty == VolumeType::Luks {
                    VolumeType::Veracrypt
                } else {
                    VolumeType::Luks
                };
                bad(mount(&source, &at, wrong, false, None));
                let mut wrong_password = mount(&source, &at, ty, false, None);
                if let CommandAction::Mount(ref mut m) = wrong_password {
                    m.passphrase = Some("incorrect password".into());
                }
                bad(wrong_password);
                ok(mount(&source, &at, VolumeType::Auto, false, None));
                let mapper = sandbox.mapper(&at);
                let opts = tool(
                    "findmnt",
                    &["-n", "-o", "OPTIONS", "--target", at.to_str().unwrap()],
                );
                assert!(String::from_utf8_lossy(&opts.stdout).contains("compress=zstd"));
                fs::write(at.join("payload"), format!("{name}: persisted secret")).unwrap();
                // Busy unmount must leave the mapper and filesystem usable.
                let child = Command::new("sleep")
                    .arg("30")
                    .current_dir(&at)
                    .spawn()
                    .unwrap();
                let mut child = child;
                bad(unmount(&at, ty, None));
                child.kill().unwrap();
                child.wait().unwrap();
                assert!(mapper.exists());
                // A second mount and a forced create cannot tear down the first mapping.
                bad(mount(&source, &other, ty, false, None));
                let mut forced = create(&source, kind, ty, cipher);
                if let CommandAction::Create(ref mut c) = forced {
                    c.force = true;
                }
                bad(forced);
                assert!(at.join("payload").exists());
                bad(mount(&source, &at, ty, false, None));
                bad(unmount(&at, wrong, None));
                ok(unmount(&source, VolumeType::Auto, None));
                assert!(!mapper.exists());
                ok(mount(&source, &at, ty, true, None));
                let mapper = sandbox.mapper(&at);
                assert_eq!(
                    fs::read_to_string(at.join("payload")).unwrap(),
                    format!("{name}: persisted secret")
                );
                assert!(fs::write(at.join("forbidden"), b"no").is_err());
                let flags = tool("blockdev", &["--getro", mapper.to_str().unwrap()]);
                assert_eq!(String::from_utf8_lossy(&flags.stdout).trim(), "1");
                ok(unmount(&mapper, VolumeType::Auto, None));
                assert!(!mapper.exists());
                bad(unmount(&source, ty, None));
                // Symlink mount and mountpoint unmount; LUKS tag round trip.
                let alias = sandbox.path("source alias");
                symlink(&source, &alias).unwrap();
                let tag = (ty == VolumeType::Luks).then_some("integration work");
                ok(mount(&alias, &at, VolumeType::Auto, false, tag));
                sandbox.mapper(&at);
                ok(unmount(&at, VolumeType::Auto, None));
                if let Some(tag) = tag {
                    ok(mount(&source, &at, ty, false, Some(tag)));
                    sandbox.mapper(&at);
                    ok(unmount(&source, ty, Some(tag)));
                } else {
                    bad(mount(&source, &at, ty, false, Some("unsupported")));
                }
                fs::remove_file(alias).unwrap();
                // Cross-tool password verification proves interoperability of created headers.
                if ty == VolumeType::Veracrypt {
                    let mut child = Command::new("veracrypt")
                        .args([
                            "-t",
                            "--non-interactive",
                            "--stdin",
                            "--pim=0",
                            "-k",
                            "",
                            "--protect-hidden=no",
                            "--filesystem=none",
                            "--mount",
                        ])
                        .arg(&source)
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .unwrap();
                    child
                        .stdin
                        .take()
                        .unwrap()
                        .write_all(PASSWORD.as_bytes())
                        .unwrap();
                    let result = child.wait_with_output().unwrap();
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    tool("veracrypt", &["-t", "-u", source.to_str().unwrap()]);
                }
                if kind == SourceKind::Block {
                    // Native VeraCrypt may already detach a device-hosted loop at unmount.
                    // Never detach a loop number that has been reassigned to another image.
                    if sandbox.owns_loop(&source) {
                        tool("losetup", &["-d", source.to_str().unwrap()]);
                    }
                    sandbox.loops.retain(|(dev, _)| Path::new(dev) != source);
                }
                assert!(!fs::read_dir(&sandbox.dir).unwrap().any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("lurker-tmp")));
            }
        }
    }
    assert!(cases > 0);
    if std::env::var_os("LURKER_TEST_CASE").is_none() {
        assert_eq!(cases, 12);
    }
    eprintln!("PASS: {cases} real create profiles and all lifecycle scenarios");
}

#[test]
#[ignore = "requires root and real Linux crypto tools; run ignored tests with --test-threads=1"]
fn real_failure_recovery() {
    assert!(probe_system().unwrap().is_root);
    let mut sandbox = Sandbox::new();
    let at = sandbox.path("mount");
    fs::create_dir(&at).unwrap();
    let wrappers = sandbox.path("tools");
    fs::create_dir(&wrappers).unwrap();
    let original_path = std::env::var_os("PATH").unwrap();
    let wrapped_path = std::env::join_paths(
        std::iter::once(wrappers.clone()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    struct RestorePath(std::ffi::OsString);
    impl Drop for RestorePath {
        fn drop(&mut self) {
            std::env::set_var("PATH", &self.0);
        }
    }
    let _restore = RestorePath(original_path.clone());
    let wrapper = |name: &str| {
        let path = wrappers.join(name);
        fs::write(&path, "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    };
    for ty in [VolumeType::Luks, VolumeType::Veracrypt] {
        eprintln!("FAILURE RECOVERY: {ty}");
        let target = sandbox.path(&format!("{ty}.img"));
        fs::write(&target, "must survive failed create").unwrap();
        wrapper("mkfs.btrfs");
        std::env::set_var("PATH", &wrapped_path);
        let mut action = create(&target, SourceKind::File, ty, CreateCipher::Aes);
        if let CommandAction::Create(ref mut c) = action {
            c.force = true;
        }
        let before = list_active_volumes()
            .unwrap()
            .into_iter()
            .map(|v| v.mapper_name)
            .collect::<Vec<_>>();
        bad(action);
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "must survive failed create"
        );
        assert_eq!(
            before,
            list_active_volumes()
                .unwrap()
                .into_iter()
                .map(|v| v.mapper_name)
                .collect::<Vec<_>>()
        );
        assert!(!fs::read_dir(&sandbox.dir).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("lurker-tmp")));
        std::env::set_var("PATH", &original_path);
        fs::remove_file(wrappers.join("mkfs.btrfs")).unwrap();
        fs::remove_file(&target).unwrap();
        ok(create(&target, SourceKind::File, ty, CreateCipher::Aes));
        wrapper("mount");
        std::env::set_var("PATH", &wrapped_path);
        bad(mount(&target, &at, ty, false, None));
        assert_eq!(
            before,
            list_active_volumes()
                .unwrap()
                .into_iter()
                .map(|v| v.mapper_name)
                .collect::<Vec<_>>()
        );
        std::env::set_var("PATH", &original_path);
        fs::remove_file(wrappers.join("mount")).unwrap();
        // Missing optional tools: mount still works without VeraCrypt or blkid.
        for name in ["cryptsetup", "mount", "umount", "lsblk", "mkfs.btrfs"] {
            symlink(format!("/usr/bin/{name}"), wrappers.join(name)).unwrap();
        }
        std::env::set_var("PATH", &wrappers);
        ok(mount(&target, &at, VolumeType::Auto, false, None));
        sandbox.mapper(&at);
        ok(unmount(&target, VolumeType::Auto, None));
        if ty == VolumeType::Veracrypt {
            let other = sandbox.path("missing-veracrypt.img");
            bad(create(&other, SourceKind::File, ty, CreateCipher::Aes));
            assert!(!other.exists());
        }
        std::env::set_var("PATH", &original_path);
        for name in ["cryptsetup", "mount", "umount", "lsblk", "mkfs.btrfs"] {
            fs::remove_file(wrappers.join(name)).unwrap();
        }
        // A damaged header or random input is never accepted as a volume.
        let invalid = sandbox.path("invalid.img");
        fs::write(&invalid, vec![0xa5; 1024 * 1024]).unwrap();
        bad(mount(&invalid, &at, VolumeType::Auto, false, None));
        fs::remove_file(invalid).unwrap();
    }
    let dev = sandbox.loop_device("safety.img");
    wrapper("lsblk");
    std::env::set_var("PATH", &wrapped_path);
    bad(create(
        &dev,
        SourceKind::Block,
        VolumeType::Luks,
        CreateCipher::Aes,
    ));
    std::env::set_var("PATH", &original_path);
    let mut bytes = [1u8; 4096];
    use std::io::Read;
    fs::File::open(&dev)
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    assert!(bytes.iter().all(|b| *b == 0));
    // Protect a mounted raw filesystem, and an open but unmounted holder.
    tool("mkfs.btrfs", &["-q", dev.to_str().unwrap()]);
    tool("mount", &[dev.to_str().unwrap(), at.to_str().unwrap()]);
    bad(create(
        &dev,
        SourceKind::Block,
        VolumeType::Veracrypt,
        CreateCipher::Aes,
    ));
    tool("umount", &[at.to_str().unwrap()]);
    eprintln!(
        "PASS: filesystem/create/mount failures, fallback tools, invalid data and block safety"
    );
}

#[test]
#[ignore = "requires root, real Linux crypto tools and mkfs.ext4"]
fn external_volumes_and_input_safety() {
    assert!(probe_system().unwrap().is_root);
    let mut sandbox = Sandbox::new();
    let at = sandbox.path("mount");
    fs::create_dir(&at).unwrap();
    let input_tool = |program: &str, args: &[&str]| {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(PASSWORD.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{program}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    for ty in [VolumeType::Luks, VolumeType::Veracrypt] {
        let file = sandbox.path(&format!("external-{ty}.img"));
        eprintln!("EXTERNAL: {ty} with ext4 and non-Lurker header defaults");
        if ty == VolumeType::Luks {
            fs::File::create(&file)
                .unwrap()
                .set_len(256 * 1024 * 1024)
                .unwrap();
            input_tool(
                "cryptsetup",
                &[
                    "luksFormat",
                    "--type",
                    "luks1",
                    "--batch-mode",
                    "--iter-time",
                    "100",
                    file.to_str().unwrap(),
                    "-",
                ],
            );
        } else {
            input_tool(
                "veracrypt",
                &[
                    "-t",
                    "--create",
                    "--non-interactive",
                    "--stdin",
                    "--size=256M",
                    "--volume-type=normal",
                    "--encryption=AES",
                    "--hash=SHA-256",
                    "--filesystem=none",
                    "--pim=0",
                    "-k",
                    "",
                    "--random-source=/dev/urandom",
                    file.to_str().unwrap(),
                ],
            );
        }
        let name = format!("lurker_external_{}_{}", std::process::id(), ty);
        let mapper = Path::new("/dev/mapper").join(&name);
        assert!(!mapper.exists());
        sandbox.mappers.push(mapper.clone());
        if ty == VolumeType::Luks {
            input_tool(
                "cryptsetup",
                &["open", "--key-file", "-", file.to_str().unwrap(), &name],
            );
        } else {
            input_tool(
                "cryptsetup",
                &[
                    "open",
                    "--type",
                    "tcrypt",
                    "--veracrypt",
                    "--batch-mode",
                    file.to_str().unwrap(),
                    &name,
                ],
            );
        }
        tool("mkfs.ext4", &["-q", mapper.to_str().unwrap()]);
        // Unmount an explicit mapper without a mounted filesystem.
        ok(unmount(&mapper, VolumeType::Auto, None));
        ok(mount(&file, &at, VolumeType::Auto, false, None));
        let volume = list_active_volumes()
            .unwrap()
            .into_iter()
            .find(|v| v.mountpoint.as_deref() == Some(&at))
            .unwrap();
        sandbox.mappers.push(volume.mapper_path.clone());
        assert_eq!(volume.filesystem_type.as_deref(), Some("ext4"));
        let opts = tool(
            "findmnt",
            &["-n", "-o", "OPTIONS", "--target", at.to_str().unwrap()],
        );
        assert!(!String::from_utf8_lossy(&opts.stdout).contains("compress="));
        fs::write(at.join("external-data"), "persisted").unwrap();
        ok(unmount(&at, VolumeType::Auto, None));
        ok(mount(&file, &at, ty, true, None));
        assert_eq!(
            fs::read_to_string(at.join("external-data")).unwrap(),
            "persisted"
        );
        assert!(fs::write(at.join("forbidden"), b"x").is_err());
        ok(unmount(&file, VolumeType::Auto, None));
        let missing = sandbox.path("missing");
        bad(mount(&missing, &at, ty, false, None));
        bad(mount(&file, &missing, ty, false, None));
        let mut no_password = mount(&file, &at, ty, false, None);
        if let CommandAction::Mount(ref mut m) = no_password {
            m.passphrase = None;
        }
        bad(no_password);
        let link = sandbox.path("symlink-create");
        symlink(&file, &link).unwrap();
        let mut symlink_action = create(&link, SourceKind::File, ty, CreateCipher::Aes);
        if let CommandAction::Create(ref mut c) = symlink_action {
            c.force = true;
        }
        bad(symlink_action);
        fs::remove_file(link).unwrap();
    }
    let target = sandbox.path("invalid-create.img");
    for size in ["0", "-1", "NaN", "1e3"] {
        let mut action = create(
            &target,
            SourceKind::File,
            VolumeType::Luks,
            CreateCipher::Aes,
        );
        if let CommandAction::Create(ref mut c) = action {
            c.size_gb = Some(size.into());
        }
        bad(action);
        assert!(!target.exists());
    }
    let mut auto = create(
        &target,
        SourceKind::File,
        VolumeType::Auto,
        CreateCipher::Aes,
    );
    bad(auto.clone());
    if let CommandAction::Create(ref mut c) = auto {
        c.volume_type = VolumeType::Veracrypt;
        c.passphrase = Some("first\nsecond".into());
    }
    bad(auto);
    assert!(!target.exists());
    eprintln!("PASS: external LUKS1 / SHA-256 VeraCrypt, ext4, open-only mappers and input guards");
}

#[test]
#[ignore = "requires root, loop devices and cryptsetup"]
fn block_holder_safety() {
    assert!(probe_system().unwrap().is_root);
    let mut sandbox = Sandbox::new();
    let device = sandbox.loop_device("holder-safety.img");
    let name = format!("lurker_holder_safety_{}", std::process::id());
    let mapper = Path::new("/dev/mapper").join(&name);
    assert!(!mapper.exists());
    sandbox.mappers.push(mapper.clone());
    tool(
        "cryptsetup",
        &[
            "open",
            "--type",
            "plain",
            "--cipher",
            "aes-xts-plain64",
            "--key-size",
            "512",
            "--key-file",
            "/dev/zero",
            "--keyfile-size",
            "64",
            device.to_str().unwrap(),
            &name,
        ],
    );
    for ty in [VolumeType::Luks, VolumeType::Veracrypt] {
        let response = bad(create(&device, SourceKind::Block, ty, CreateCipher::Aes));
        assert!(response.error.unwrap().contains("holders"));
        assert!(mapper.exists());
        bad(unmount(&mapper, VolumeType::Auto, None)); // plain mappings are not a supported encrypted type
        assert!(mapper.exists());
    }
    tool("cryptsetup", &["close", &name]);
    use std::io::Read;
    let mut bytes = [1; 4096];
    fs::File::open(device)
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    assert!(bytes.iter().all(|b| *b == 0));
    eprintln!("PASS: active unmounted holders are never overwritten; unsupported mapper types are not closed");
}

#[test]
#[ignore = "requires root, loop partitions, sfdisk and real Linux crypto tools"]
fn partition_round_trip() {
    assert!(probe_system().unwrap().is_root);
    let mut sandbox = Sandbox::new();
    let device = sandbox.loop_device("partitioned.img");
    let mut child = Command::new("sfdisk")
        .args(["--quiet", device.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"label: gpt\n, , L\n")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    tool("partx", &["--update", device.to_str().unwrap()]);
    let partition = PathBuf::from(format!("{}p1", device.display()));
    assert!(partition.exists());
    let at = sandbox.path("mount");
    fs::create_dir(&at).unwrap();
    for ty in [VolumeType::Luks, VolumeType::Veracrypt] {
        ok(create(&partition, SourceKind::Block, ty, CreateCipher::Aes));
        ok(mount(&partition, &at, VolumeType::Auto, false, None));
        let mapper = sandbox.mapper(&at);
        fs::write(
            at.join("partition-data"),
            format!("{ty} partition persisted"),
        )
        .unwrap();
        // The whole disk must also be protected when its child partition has a holder.
        let response = bad(create(&device, SourceKind::Block, ty, CreateCipher::Aes));
        assert!(response.error.unwrap().contains("holders"));
        ok(unmount(&partition, VolumeType::Auto, None));
        assert!(!mapper.exists());
        ok(mount(&partition, &at, ty, true, None));
        sandbox.mapper(&at);
        assert_eq!(
            fs::read_to_string(at.join("partition-data")).unwrap(),
            format!("{ty} partition persisted")
        );
        ok(unmount(&at, VolumeType::Auto, None));
    }
    eprintln!("PASS: GPT partition create/mount/persistence/readonly/unmount for both formats; parent-disk protection");
}

#[test]
#[ignore = "requires root, mkfs.vfat, mkfs.exfat and real Linux crypto tools"]
fn containers_on_portable_filesystems() {
    assert!(probe_system().unwrap().is_root);
    let mut sandbox = Sandbox::new();
    let host = sandbox.path("host");
    fs::create_dir(&host).unwrap();
    let at = sandbox.path("mount");
    fs::create_dir(&at).unwrap();
    sandbox.raw_mounts.push(host.clone());
    for (filesystem, ty) in [("vfat", VolumeType::Luks), ("exfat", VolumeType::Veracrypt)] {
        eprintln!("PORTABLE HOST: {filesystem} / {ty}");
        let device = sandbox.loop_device(&format!("{filesystem}.img"));
        fs::OpenOptions::new()
            .write(true)
            .open(sandbox.path(&format!("{filesystem}.img")))
            .unwrap()
            .set_len(512 * 1024 * 1024)
            .unwrap();
        tool("losetup", &["--set-capacity", device.to_str().unwrap()]);
        tool(&format!("mkfs.{filesystem}"), &[device.to_str().unwrap()]);
        tool(
            "mount",
            &[
                "-t",
                filesystem,
                "-o",
                "fmask=0177,dmask=0077",
                device.to_str().unwrap(),
                host.to_str().unwrap(),
            ],
        );
        let target = host.join("portable vault.img");
        ok(create(&target, SourceKind::File, ty, CreateCipher::Aes));
        bad(create(&target, SourceKind::File, ty, CreateCipher::Aes));
        ok(mount(&target, &at, VolumeType::Auto, false, None));
        sandbox.mapper(&at);
        fs::write(at.join("portable-data"), "persisted").unwrap();
        ok(unmount(&at, VolumeType::Auto, None));
        ok(mount(&target, &at, ty, true, None));
        sandbox.mapper(&at);
        assert_eq!(
            fs::read_to_string(at.join("portable-data")).unwrap(),
            "persisted"
        );
        ok(unmount(&target, VolumeType::Auto, None));
        fs::remove_file(&target).unwrap();
        // Forced replacement is also atomic on a filesystem without hard links.
        let temp_target = host.join("replacement.img");
        fs::write(&temp_target, "replace me").unwrap();
        let mut action = create(&temp_target, SourceKind::File, ty, CreateCipher::Aes);
        if let CommandAction::Create(ref mut c) = action {
            c.force = true;
        }
        ok(action);
        ok(mount(&temp_target, &at, ty, false, None));
        sandbox.mapper(&at);
        ok(unmount(&at, VolumeType::Auto, None));
        tool("umount", &[host.to_str().unwrap()]);
    }
    eprintln!("PASS: creation, atomic no-clobber/replacement, persistence and readonly on FAT/exFAT hosts");
}
