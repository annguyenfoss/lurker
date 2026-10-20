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
            target: self.0.join("vault with spaces.container"),
            size_gb: Some("0.03125".into()),
            force: false,
            source_kind: SourceKind::File,
            volume_type: kind,
            cipher,
            passphrase: Some(PASS.into()),
        }
    }
    fn mount(&self, path: &Path, pass: &str, readonly: bool, kind: VolumeType) -> CommandAction {
        CommandAction::Mount(MountCommand {
            source: path.into(),
            mountpoint: self.0.join("mounted vault"),
            tag: None,
            volume_type: kind,
            passphrase: Some(pass.into()),
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
    let source = f.0.join("plain.dmg");
    assert!(std::process::Command::new("/usr/bin/hdiutil")
        .args(["create", "-size", "16m", "-fs", "APFS"])
        .arg(&source)
        .output()
        .unwrap()
        .status
        .success());
    assert!(bad(f.mount(&source, PASS, false, VolumeType::Auto)).contains("unencrypted"));
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
