#![cfg(target_os = "linux")]
//! Real header operations need cryptsetup, but no root, loops or device-mapper.
//! cargo test -p lurker-core --test keyslots_native -- --ignored --nocapture
use lurker_core::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("lurker-keyslots-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn tool(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new("cryptsetup")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input).unwrap();
    drop(stdin);
    child.wait_with_output().unwrap()
}
fn setup(image: &Path, version: u16, secret: &[u8]) {
    fs::File::create(image)
        .unwrap()
        .set_len(32 * 1024 * 1024)
        .unwrap();
    // Fast disposable fixtures. Lurker's additions use its real profile KDF costs.
    let result = tool(
        &[
            "luksFormat",
            "--type",
            &format!("luks{version}"),
            "--pbkdf",
            "pbkdf2",
            "--pbkdf-force-iterations",
            "1000",
            "--batch-mode",
            image.to_str().unwrap(),
            "-",
        ],
        secret,
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
fn request(
    source: &Path,
    operation: KeyslotOperation,
    slot: Option<u8>,
    old_password: Option<&str>,
    old_keyfile: Option<&Path>,
    new_password: Option<&str>,
    new_keyfile: Option<&Path>,
) -> CommandAction {
    CommandAction::Keyslots(KeyslotsCommand {
        privilege_mode: PrivilegeMode::Privileged,
        source: source.into(),
        operation,
        slot,
        passphrase: old_password.map(str::to_owned),
        keyfile: old_keyfile.map(Path::to_path_buf),
        new_passphrase: new_password.map(str::to_owned),
        new_keyfile: new_keyfile.map(Path::to_path_buf),
    })
}
fn ok(action: CommandAction) -> OperationResponse {
    let response = run_buffered(action);
    assert!(response.ok, "{response:?}");
    response
}
fn bad(action: CommandAction, fragment: &str) {
    let response = run_buffered(action);
    assert!(!response.ok, "{response:?}");
    assert!(
        response.error.as_deref().unwrap().contains(fragment),
        "{response:?}"
    );
}
fn authenticates(image: &Path, slot: u8, secret: &[u8]) -> bool {
    tool(
        &[
            "open",
            "--test-passphrase",
            "--key-file",
            "-",
            "--key-slot",
            &slot.to_string(),
            image.to_str().unwrap(),
        ],
        secret,
    )
    .status
    .success()
}

#[test]
#[ignore = "requires cryptsetup; uses real KDF costs, disposable headers only, no root required"]
fn independent_password_keyfile_slots_and_safe_removal() {
    let f = Fixture::new();
    let key1 = f.path("key 1, binary");
    let key2 = f.path("key 2");
    let binary1: Vec<_> = (0..=255).collect();
    let binary2: Vec<_> = (0..=255).rev().collect();
    fs::write(&key1, &binary1).unwrap();
    fs::write(&key2, &binary2).unwrap();
    let password = "  original 密碼  ";
    let second = " second independent password ";
    for version in [1, 2] {
        let image = f.path(&format!("luks{version}"));
        setup(&image, version, password.as_bytes());
        let high = if version == 1 { 7 } else { 31 };
        let listed = ok(request(
            &image,
            KeyslotOperation::List,
            None,
            None,
            None,
            None,
            None,
        ));
        assert!(listed
            .logs
            .iter()
            .any(|entry| entry.message.contains("active keyslots: 0")));
        bad(
            request(
                &image,
                KeyslotOperation::Remove,
                Some(0),
                Some(password),
                None,
                None,
                None,
            ),
            "last usable",
        );
        bad(
            request(
                &image,
                KeyslotOperation::Add,
                Some(0),
                Some(password),
                None,
                Some(second),
                None,
            ),
            "occupied",
        );
        bad(
            request(
                &image,
                KeyslotOperation::Add,
                Some(1),
                Some("incorrect"),
                None,
                Some(second),
                None,
            ),
            "Failed to update",
        );
        assert!(!authenticates(&image, 1, second.as_bytes()));
        // password -> password, password -> keyfile, keyfile -> password, keyfile -> keyfile
        ok(request(
            &image,
            KeyslotOperation::Add,
            Some(1),
            Some(password),
            None,
            Some(second),
            None,
        ));
        ok(request(
            &image,
            KeyslotOperation::Add,
            Some(high),
            Some(password),
            None,
            None,
            Some(&key1),
        ));
        ok(request(
            &image,
            KeyslotOperation::Add,
            Some(2),
            None,
            Some(&key1),
            Some("third"),
            None,
        ));
        ok(request(
            &image,
            KeyslotOperation::Add,
            Some(3),
            None,
            Some(&key1),
            None,
            Some(&key2),
        ));
        for (slot, secret) in [
            (0, password.as_bytes()),
            (1, second.as_bytes()),
            (2, b"third".as_slice()),
            (3, binary2.as_slice()),
            (high, binary1.as_slice()),
        ] {
            assert!(
                authenticates(&image, slot, secret),
                "LUKS{version} slot {slot}"
            );
            assert!(!authenticates(&image, slot, b"wrong"));
        }
        // The credential being deleted cannot authorize its own removal.
        bad(
            request(
                &image,
                KeyslotOperation::Remove,
                Some(0),
                Some(password),
                None,
                None,
                None,
            ),
            "Failed to update",
        );
        assert!(authenticates(&image, 0, password.as_bytes()));
        ok(request(
            &image,
            KeyslotOperation::Remove,
            Some(0),
            None,
            Some(&key1),
            None,
            None,
        ));
        assert!(!authenticates(&image, 0, password.as_bytes()));
        assert!(authenticates(&image, high, &binary1));
        bad(
            request(
                &image,
                KeyslotOperation::Remove,
                Some(0),
                Some(second),
                None,
                None,
                None,
            ),
            "not active",
        );
        ok(request(
            &image,
            KeyslotOperation::Remove,
            Some(high),
            Some(second),
            None,
            None,
            None,
        ));
        assert!(!authenticates(&image, high, &binary1));
        for slot in [2, 3] {
            ok(request(
                &image,
                KeyslotOperation::Remove,
                Some(slot),
                Some(second),
                None,
                None,
                None,
            ));
        }
        bad(
            request(
                &image,
                KeyslotOperation::Remove,
                Some(1),
                Some(second),
                None,
                None,
                None,
            ),
            "last usable",
        );
        assert!(authenticates(&image, 1, second.as_bytes()));
        eprintln!("LUKS{version}: all credential combinations, independent unlock and safe removal passed");
    }
}
