#![cfg(target_os = "macos")]
use lurker_core::OperationResponse;
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn request(payload: &Value) -> OperationResponse {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lurker-helper"))
        .arg("run")
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(payload).unwrap())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    serde_json::from_slice(&result.stdout).expect("helper must return only JSON on stdout")
}

#[test]
fn macos_helper_returns_errors_without_linux_elevation() {
    let response = request(
        &json!({"action":"create", "target":"unused", "size_gb":"1", "force":false, "source_kind":"file", "volume_type":"luks", "cipher":"aes", "passphrase":"private"}),
    );
    assert!(!response.ok);
    assert!(response.error.unwrap().contains("LUKS requires Linux"));
    assert!(!request(&json!({"action":"unknown"})).ok);
}

#[test]
#[ignore = "creates and mounts a disposable native encrypted disk image through the desktop helper"]
fn apple_helper_lifecycle() {
    let path = std::env::temp_dir().join(format!("lurker-helper-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let source = path.join("apple.dmg");
    let mount = path.join("mount");
    let create = request(
        &json!({"action":"create", "target":source, "size_gb":"0.03125", "force":false, "source_kind":"file", "volume_type":"apple", "cipher":"aes", "passphrase":"helper test passphrase"}),
    );
    assert!(create.ok, "{:?}", create.error);
    let response = request(
        &json!({"action":"mount", "source":source, "mountpoint":mount, "volume_type":"auto", "passphrase":"helper test passphrase", "readonly":false}),
    );
    assert!(response.ok, "{:?}", response.error);
    let written = std::fs::write(mount.join("test"), b"helper wrote data");
    let response = request(&json!({"action":"unmount", "target":source, "volume_type":"auto"}));
    assert!(response.ok, "{:?}", response.error);
    written.unwrap();
    std::fs::remove_dir_all(path).unwrap();
}
