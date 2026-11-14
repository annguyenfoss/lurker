use std::process::{Command, Stdio};

#[test]
fn cryptomator_fencing_is_a_numeric_macos_helper_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_lurker-helper"))
        .arg("capabilities")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(capabilities["protocol_version"], 3);
    assert_eq!(
        capabilities["cryptomator_fence_protocol"].as_u64(),
        if cfg!(target_os = "macos") {
            Some(1)
        } else {
            None
        }
    );
}
