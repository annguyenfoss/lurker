#![cfg(target_os = "linux")]

use std::process::Command;

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lurker"))
        .args(args)
        .env("PATH", "")
        .output()
        .unwrap()
}
#[test]
fn help_documents_linux_formats_and_readonly() {
    for args in [vec!["help"], vec!["--help"], vec!["-h"]] {
        let out = cli(&args);
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout);
        for word in ["veracrypt", "luks", "serpent", "twofish", "--readonly"] {
            assert!(text.contains(word));
        }
    }
}
#[test]
fn invalid_options_and_actions_fail_without_crypto_tools() {
    for args in [
        vec![],
        vec!["invalid"],
        vec!["create", "--type", "auto", "vault", "1"],
        vec!["createvc", "--type", "luks", "vault", "1"],
        vec!["create", "--cipher", "cascade", "vault", "1"],
        vec!["create", "--readonly", "vault", "1"],
        vec!["unmount", "--readonly", "/mnt/test"],
        vec!["mount", "--cipher", "aes", "vault", "/mnt/test"],
        vec!["--type"],
        vec!["--cipher"],
        vec!["--color", "invalid"],
    ] {
        let out = cli(&args);
        assert!(!out.status.success(), "accepted {args:?}");
    }
}
#[test]
fn missing_tools_are_reported_before_creating_a_file() {
    let dir = std::env::temp_dir().join(format!("lurker-cli-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    for verb in ["create", "createvc"] {
        let file = dir.join(format!("{verb}.img"));
        let out = cli(&[verb, file.to_str().unwrap(), "0.25"]);
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("cryptsetup"));
        assert!(!file.exists());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
