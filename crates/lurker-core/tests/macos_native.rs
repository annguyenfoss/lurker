#![cfg(target_os = "macos")]
//! Opt-in tests use only disposable file containers, never physical disks.
//! cargo test -p lurker-core --test macos_native -- --ignored --test-threads=1
use lurker_core::*;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PASS: &str = "  Lurker native test passphrase ü!  ";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "lurker-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn create(&self, kind: VolumeType, cipher: CreateCipher) -> CreateCommand {
        CreateCommand {
            peer_sources: Vec::new(),
            privilege_mode: lurker_core::PrivilegeMode::Privileged,
            target: self.0.join("vault with spaces.container"),
            size_gb: Some("0.03125".into()),
            force: false,
            source_kind: SourceKind::File,
            volume_type: kind,
            cipher,
            filesystem: None,
            passphrase: Some(PASS.into()),
            keyfile: None,
        }
    }
    fn mount(&self, path: &Path, pass: &str, readonly: bool, kind: VolumeType) -> CommandAction {
        CommandAction::Mount(MountCommand {
            peer_sources: Vec::new(),
            allow_degraded: false,
            privilege_mode: lurker_core::PrivilegeMode::Privileged,
            source: path.into(),
            mountpoint: self.0.join("mounted vault"),
            tag: None,
            volume_type: kind,
            passphrase: Some(pass.into()),
            keyfile: None,
            readonly,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(volumes) = list_active_volumes() {
            for v in volumes {
                if v.source_path
                    .as_ref()
                    .is_some_and(|p| p.starts_with(&self.0))
                {
                    if !run_buffered(unmount(v.source_path.as_ref().unwrap())).ok {
                        eprintln!(
                            "Retaining test fixture with a busy volume: {}",
                            self.0.display()
                        );
                        return;
                    }
                }
            }
        } else {
            return;
        }
        // The installed FUSE-T build can leave VeraCrypt service processes alive
        // after all of a fixture's volumes are closed. Reap only services whose
        // command line names this unique disposable fixture, never user volumes.
        // This is test cleanup, not a production workaround for native timeouts.
        if let Ok(output) = std::process::Command::new("/bin/ps")
            .args(["-axo", "pid=,args="])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let Some((pid, command)) = line.trim().split_once(' ') else {
                    continue;
                };
                let command = command.trim_start();
                if command.starts_with("/Applications/VeraCrypt.app/Contents/MacOS/VeraCrypt ")
                    && command.contains(self.0.to_string_lossy().as_ref())
                {
                    if let Ok(pid) = pid.parse::<i32>() {
                        // SAFETY: pid was just read from ps and uniquely identifies
                        // a leftover service for this unmounted disposable fixture.
                        unsafe {
                            libc::kill(pid, libc::SIGKILL);
                        }
                    }
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn unmount(target: &Path) -> CommandAction {
    CommandAction::Unmount(UnmountCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        target: target.into(),
        tag: None,
        volume_type: VolumeType::Auto,
    })
}
fn ok(command: CommandAction) {
    let action = match &command {
        CommandAction::Create(_) => "create",
        CommandAction::Mount(_) => "mount",
        CommandAction::Unmount(_) => "unmount",
        CommandAction::Keyslots(_) => "keyslots",
    };
    let response = run_buffered(command);
    assert!(response.ok, "{action}: {:?}", response.error);
}
fn bad(command: CommandAction) -> String {
    let response = run_buffered(command);
    assert!(!response.ok);
    let text = format!("{:?}", response);
    assert!(!text.contains(PASS));
    response.error.unwrap()
}

#[test]
#[ignore = "requires native macOS VeraCrypt and its matching FUSE runtime"]
fn veracrypt_keyfile_only_round_trips() {
    use std::io::Read;
    for cipher in [
        CreateCipher::Aes,
        CreateCipher::Serpent,
        CreateCipher::Twofish,
    ] {
        let fixture = Fixture::new();
        let key = fixture.0.join("key, with spaces.bin");
        let wrong = fixture.0.join("wrong key.bin");
        let mut random = [0u8; 256];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut random)
            .unwrap();
        fs::write(&key, random).unwrap();
        fs::write(&wrong, b"wrong keyfile").unwrap();
        let mut create = fixture.create(VolumeType::Veracrypt, cipher);
        create.passphrase = None;
        create.keyfile = Some(key.clone());
        let source = create.target.clone();
        ok(CommandAction::Create(create));
        let keyed_mount = |key: &Path, readonly| {
            let CommandAction::Mount(mut command) =
                fixture.mount(&source, PASS, readonly, VolumeType::Auto)
            else {
                unreachable!()
            };
            command.passphrase = None;
            command.keyfile = Some(key.into());
            CommandAction::Mount(command)
        };
        bad(keyed_mount(&wrong, false));
        bad(fixture.mount(&source, PASS, false, VolumeType::Veracrypt));
        let CommandAction::Mount(mut combined) = keyed_mount(&key, false) else {
            unreachable!()
        };
        combined.passphrase = Some(PASS.into());
        assert!(bad(CommandAction::Mount(combined)).contains("never both"));
        ok(keyed_mount(&key, false));
        let mountpoint = fixture.0.join("mounted vault");
        fs::write(mountpoint.join("persistent"), b"keyfile-only data").unwrap();
        ok(unmount(&source));
        ok(keyed_mount(&key, true));
        assert_eq!(
            fs::read(mountpoint.join("persistent")).unwrap(),
            b"keyfile-only data"
        );
        assert!(fs::write(mountpoint.join("forbidden"), b"no").is_err());
        ok(unmount(&source));
    }
}

fn lifecycle(kind: VolumeType, cipher: CreateCipher) {
    eprintln!("Testing {kind}/{cipher}");
    let f = Fixture::new();
    let mut create = f.create(kind, cipher);
    let source = create.target.clone();
    let mountpoint = f.0.join("mounted vault");
    fs::create_dir(&mountpoint).unwrap();
    ok(CommandAction::Create(create.clone()));
    assert_eq!(
        fs::metadata(&source).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let before = fs::read(&source).unwrap();
    assert!(bad(CommandAction::Create(create.clone())).contains("already exists"));
    assert_eq!(fs::read(&source).unwrap(), before);
    // Bad credentials must not leave an attached virtual device.
    bad(f.mount(&source, "incorrect-password", false, VolumeType::Auto));
    assert!(!list_active_volumes()
        .unwrap()
        .iter()
        .any(|v| v.source_path.as_ref() == Some(&source)));
    // A non-empty mount directory must never have its contents hidden.
    fs::write(mountpoint.join("existing"), b"keep").unwrap();
    assert!(bad(f.mount(&source, PASS, false, kind)).contains("empty directory"));
    fs::remove_file(mountpoint.join("existing")).unwrap();
    ok(f.mount(&source, PASS, false, VolumeType::Auto));
    let mounted = list_active_volumes()
        .unwrap()
        .into_iter()
        .find(|v| v.source_path.as_ref() == Some(&source))
        .expect("container listed");
    assert!(!mounted.readonly);
    assert_eq!(mounted.mountpoint.as_ref(), Some(&mountpoint));
    fs::write(
        mountpoint.join("persistent.txt"),
        b"encrypted data survives remount",
    )
    .unwrap();
    bad(f.mount(&source, PASS, false, VolumeType::Auto));
    create.force = true;
    assert!(bad(CommandAction::Create(create.clone())).contains("attached"));
    let other = if kind == VolumeType::Apple {
        VolumeType::Veracrypt
    } else {
        VolumeType::Apple
    };
    bad(CommandAction::Unmount(UnmountCommand {
        privilege_mode: lurker_core::PrivilegeMode::Privileged,
        target: source.clone(),
        tag: None,
        volume_type: other,
    }));
    // A process holding its working directory inside a volume makes it busy.
    let mut holder = std::process::Command::new("/bin/sleep")
        .arg("30")
        .current_dir(&mountpoint)
        .spawn()
        .unwrap();
    let busy = run_buffered(unmount(&source));
    let _ = holder.kill();
    let _ = holder.wait();
    assert!(!busy.ok, "busy volume must not be forcibly detached");
    assert!(mountpoint.join("persistent.txt").exists());
    ok(unmount(&source));
    ok(f.mount(&source, PASS, true, kind));
    assert_eq!(
        fs::read(mountpoint.join("persistent.txt")).unwrap(),
        b"encrypted data survives remount"
    );
    assert!(fs::write(mountpoint.join("blocked.txt"), b"no").is_err());
    let mounted = list_active_volumes()
        .unwrap()
        .into_iter()
        .find(|v| v.source_path.as_ref() == Some(&source))
        .unwrap();
    assert!(mounted.readonly);
    ok(unmount(&mounted.mapper_path));
    ok(f.mount(&source, PASS, false, kind));
    ok(unmount(&mountpoint));
    bad(unmount(&source));
    // Failed forced replacement leaves the previous image intact.
    let before = fs::read(&source).unwrap();
    create.passphrase = Some("invalid\npassword".into());
    bad(CommandAction::Create(create.clone()));
    assert_eq!(fs::read(&source).unwrap(), before);
    create.passphrase = Some(PASS.into());
    ok(CommandAction::Create(create));
    fs::remove_dir(&mountpoint).unwrap();
    ok(f.mount(&source, PASS, false, kind));
    assert!(!mountpoint.join("persistent.txt").exists());
    ok(unmount(&source));
    assert!(fs::read_dir(&f.0).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".lurker-")));
    eprintln!("Passed {kind}/{cipher}");
}

#[test]
#[ignore = "creates disposable encrypted disk images"]
fn apple_lifecycle() {
    lifecycle(VolumeType::Apple, CreateCipher::Aes);
}
#[test]
#[ignore = "requires VeraCrypt and its FUSE runtime"]
fn veracrypt_aes_lifecycle() {
    lifecycle(VolumeType::Veracrypt, CreateCipher::Aes);
}
#[test]
#[ignore = "requires VeraCrypt and its FUSE runtime"]
fn veracrypt_serpent_lifecycle() {
    lifecycle(VolumeType::Veracrypt, CreateCipher::Serpent);
}
#[test]
#[ignore = "requires VeraCrypt and its FUSE runtime"]
fn veracrypt_twofish_lifecycle() {
    lifecycle(VolumeType::Veracrypt, CreateCipher::Twofish);
}

#[test]
fn invalid_commands_are_rejected_without_side_effects() {
    let f = Fixture::new();
    let create = f.create(VolumeType::Apple, CreateCipher::Aes);
    for kind in [VolumeType::Luks, VolumeType::Auto] {
        let mut c = create.clone();
        c.volume_type = kind;
        bad(CommandAction::Create(c));
    }
    for cipher in [CreateCipher::Serpent, CreateCipher::Twofish] {
        let mut c = create.clone();
        c.cipher = cipher;
        assert!(bad(CommandAction::Create(c)).contains("AES-256"));
    }
    let mut c = create.clone();
    c.source_kind = SourceKind::Block;
    c.force = true;
    assert!(bad(CommandAction::Create(c)).contains("file containers only"));
    for size in [
        "0",
        "-1",
        "NaN",
        "inf",
        "0.001",
        "99999999999999999999999999999999999999999",
    ] {
        let mut c = create.clone();
        c.size_gb = Some(size.into());
        bad(CommandAction::Create(c));
    }
    let referent = f.0.join("keep");
    fs::write(&referent, b"untouched").unwrap();
    symlink(&referent, &create.target).unwrap();
    let mut c = create;
    c.force = true;
    bad(CommandAction::Create(c));
    assert_eq!(fs::read(referent).unwrap(), b"untouched");
}

#[test]
#[ignore = "creates an unencrypted disposable disk image to verify rejection"]
fn plaintext_images_are_refused() {
    let f = Fixture::new();
    // A blank read-write DMG can be reported as raw, like a VeraCrypt file.
    // Use an Apple sparse image to exercise the recognized plaintext case.
    let source = f.0.join("plain.sparseimage");
    let output = std::process::Command::new("/usr/bin/hdiutil")
        .args(["create", "-size", "64m", "-fs", "APFS", "-type", "SPARSE"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rejection = bad(f.mount(&source, PASS, false, VolumeType::Auto));
    assert!(rejection.contains("unencrypted"), "{rejection}");
    assert!(!f.0.join("mounted vault").exists());
}

#[test]
#[ignore = "uses a disposable exFAT disk image to test external storage and failed-create rollback"]
fn external_host_failure_preserves_existing_container() {
    let f = Fixture::new();
    let host = f.0.join("host.dmg");
    let host_mount = f.0.join("host");
    assert!(std::process::Command::new("/usr/bin/hdiutil")
        .args(["create", "-size", "64m", "-fs", "exFAT"])
        .arg(&host)
        .output()
        .unwrap()
        .status
        .success());
    assert!(std::process::Command::new("/usr/bin/hdiutil")
        .args(["attach", "-nobrowse", "-mountpoint"])
        .arg(&host_mount)
        .arg(&host)
        .output()
        .unwrap()
        .status
        .success());
    struct Host(PathBuf);
    impl Drop for Host {
        fn drop(&mut self) {
            let _ = std::process::Command::new("/usr/bin/hdiutil")
                .arg("detach")
                .arg(&self.0)
                .output();
        }
    }
    let mounted_host = Host(host_mount.clone());
    let mut c = f.create(VolumeType::Apple, CreateCipher::Aes);
    ok(CommandAction::Create(c.clone()));
    let external = host_mount.join("apple.dmg");
    fs::copy(&c.target, &external).unwrap();
    c.target = external;
    let before = fs::read(&c.target).unwrap();
    c.force = true;
    c.size_gb = Some("0.125".into()); // 128 MiB cannot fit in the 64 MiB host.
    bad(CommandAction::Create(c.clone()));
    assert_eq!(fs::read(&c.target).unwrap(), before);
    assert!(fs::read_dir(&host_mount).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".lurker-")));
    ok(f.mount(&c.target, PASS, false, VolumeType::Apple));
    ok(unmount(&c.target));
    drop(mounted_host);
}

#[test]
#[ignore = "creates a disposable plain disk image containing an encrypted APFS volume"]
fn apfs_encrypted_image_and_virtual_partition() {
    use std::io::Write;
    use std::process::Command;
    let f = Fixture::new();
    let mut c = f.create(VolumeType::Apfs, CreateCipher::Aes);
    c.size_gb = Some("0.25".into());
    ok(CommandAction::Create(c.clone()));
    ok(f.mount(&c.target, PASS, false, VolumeType::Apfs));
    let payload = f.0.join("mounted vault/payload");
    let mut file = fs::File::create(&payload).unwrap();
    file.write_all(b"APFS encrypted round trip").unwrap();
    file.sync_all().unwrap();
    drop(file);
    let row = list_active_volumes()
        .unwrap()
        .into_iter()
        .find(|v| v.source_path.as_ref() == Some(&c.target))
        .unwrap();
    assert_eq!(row.filesystem_type.as_deref(), Some("apfs"));
    ok(unmount(&c.target));
    bad(f.mount(&c.target, "wrong password", false, VolumeType::Apfs));
    ok(f.mount(&c.target, PASS, false, VolumeType::Auto));
    assert_eq!(fs::read(&payload).unwrap(), b"APFS encrypted round trip");
    ok(unmount(&f.0.join("mounted vault")));
    // Attach the outer image with no credentials. Only its APFS volume is locked.
    let attached = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-nomount", "-plist"])
        .arg(&c.target)
        .output()
        .unwrap();
    assert!(attached.status.success());
    let tree: plist::Value = plist::from_bytes(&attached.stdout).unwrap();
    let outer = tree.as_dictionary().unwrap()["system-entities"]
        .as_array()
        .unwrap()[0]
        .as_dictionary()
        .unwrap()["dev-entry"]
        .as_string()
        .unwrap()
        .to_owned();
    let _attachment = ApfsTestAttachment(outer.clone());
    let listed = Command::new("/usr/sbin/diskutil")
        .args(["apfs", "list", "-plist"])
        .output()
        .unwrap();
    let list: plist::Value = plist::from_bytes(&listed.stdout).unwrap();
    let container = list.as_dictionary().unwrap()["Containers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c.as_dictionary().unwrap()["PhysicalStores"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| {
                    s.as_dictionary().unwrap()["DeviceIdentifier"]
                        .as_string()
                        .unwrap()
                        .starts_with(outer.trim_start_matches("/dev/"))
                })
        })
        .unwrap()
        .as_dictionary()
        .unwrap();
    let volume = container["Volumes"].as_array().unwrap()[0]
        .as_dictionary()
        .unwrap();
    assert_eq!(volume["FileVault"].as_boolean(), Some(true));
    assert_eq!(volume["Locked"].as_boolean(), Some(true));
    let native = PathBuf::from(format!(
        "/dev/{}",
        volume["DeviceIdentifier"].as_string().unwrap()
    ));
    ok(f.mount(&native, PASS, false, VolumeType::Apfs));
    assert_eq!(fs::read(&payload).unwrap(), b"APFS encrypted round trip");
    ok(unmount(&native));
    // The wrapper closes the owned image as well, even through the native-volume entry point.
    assert!(!f.0.join("mounted vault").exists() || !payload.exists());
}

#[test]
#[ignore = "initializes only a partition inside a disposable disk image"]
fn apfs_create_on_virtual_partition() {
    use std::process::Command;
    let f = Fixture::new();
    let image = f.0.join("partition fixture.dmg");
    let created = Command::new("/usr/bin/hdiutil")
        .args([
            "create", "-size", "512m", "-type", "UDIF", "-layout", "GPTSPUD", "-fs", "ExFAT",
        ])
        .arg(&image)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let attached = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-nomount", "-plist"])
        .arg(&image)
        .output()
        .unwrap();
    assert!(attached.status.success());
    let tree: plist::Value = plist::from_bytes(&attached.stdout).unwrap();
    let entities = tree.as_dictionary().unwrap()["system-entities"]
        .as_array()
        .unwrap();
    let _attachment = ApfsTestAttachment(
        entities[0].as_dictionary().unwrap()["dev-entry"]
            .as_string()
            .unwrap()
            .into(),
    );
    let partition = entities
        .iter()
        .find(|e| {
            e.as_dictionary()
                .unwrap()
                .get("content-hint")
                .and_then(|v| v.as_string())
                == Some("Microsoft Basic Data")
        })
        .unwrap()
        .as_dictionary()
        .unwrap()["dev-entry"]
        .as_string()
        .unwrap();
    let mut create = f.create(VolumeType::Apfs, CreateCipher::Aes);
    create.target = partition.into();
    create.source_kind = SourceKind::Block;
    create.size_gb = None;
    bad(CommandAction::Create(create.clone()));
    create.force = true;
    ok(CommandAction::Create(create));
    ok(f.mount(Path::new(partition), PASS, false, VolumeType::Apfs));
    fs::write(f.0.join("mounted vault/payload"), b"virtual partition").unwrap();
    ok(unmount(Path::new(partition)));
    ok(f.mount(&image, PASS, false, VolumeType::Apfs));
    assert_eq!(
        fs::read(f.0.join("mounted vault/payload")).unwrap(),
        b"virtual partition"
    );
    ok(unmount(&image));
}

struct ApfsTestAttachment(String);
impl Drop for ApfsTestAttachment {
    fn drop(&mut self) {
        let _ = std::process::Command::new("/usr/bin/hdiutil")
            .args(["detach", &self.0])
            .output();
    }
}
