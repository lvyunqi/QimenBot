use std::process::Command;

#[test]
fn redirected_logs_do_not_contain_ansi_escape_sequences() {
    let output = Command::new(env!("CARGO_BIN_EXE_qimenbot"))
        .arg("invalid-command")
        .output()
        .expect("qimenbot should start");

    assert!(!output.status.success());
    let combined = [output.stdout, output.stderr].concat();
    assert!(
        !combined.contains(&0x1b),
        "redirected log contained ANSI escapes: {}",
        String::from_utf8_lossy(&combined)
    );
    assert!(
        String::from_utf8_lossy(&combined).contains("unknown qimenbot command 'invalid-command'")
    );
}
