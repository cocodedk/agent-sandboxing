//! The unit file and install script agree with the constants, and the suite size is fixed.

use crate::constants::{
    ALERT_FILE_NAME, BPFTRACE, INSTALL_DIR, LOG_DIR, LOG_FILE_NAME, PROGRAM_FILE_NAME,
};

const BINARY_NAME: &str = env!("CARGO_BIN_NAME");
const UNIT: &str = include_str!("../agent-watch.service");
const INSTALL: &str = include_str!("../install.sh");
const TRAY_UNIT: &str = include_str!("../agent-watch-tray.service");
const TRAY_CONSTANTS: &str = include_str!("../tray/constants.py");
const SOURCES: [&str; 21] = [
    include_str!("main.rs"),
    include_str!("clock.rs"),
    include_str!("main_tests.rs"),
    include_str!("files_tests.rs"),
    include_str!("events.rs"),
    include_str!("events_tests.rs"),
    include_str!("program.rs"),
    include_str!("program_tests.rs"),
    include_str!("writer.rs"),
    include_str!("writer_tests.rs"),
    include_str!("constants/mod.rs"),
    include_str!("constants/network.rs"),
    include_str!("danger.rs"),
    include_str!("danger_tests.rs"),
    include_str!("record.rs"),
    include_str!("alert_tests.rs"),
    include_str!("notifier.rs"),
    include_str!("notifier_tests.rs"),
    include_str!("sni.rs"),
    include_str!("sni_tests.rs"),
    include_str!("network_tests.rs"),
];
const TEST_MARK: &str = "#[test]\n";
const EXPECTED_TESTS: usize = 51;

#[test]
fn unit_runs_the_installed_binary() {
    assert!(UNIT.contains(&format!("ExecStart={INSTALL_DIR}/{BINARY_NAME}\n")));
    assert!(UNIT.contains("KillMode=mixed\n"));
}

#[test]
fn install_script_uses_the_constants() {
    for value in [
        format!("INSTALL_DIR={INSTALL_DIR}"),
        format!("LOG_DIR={LOG_DIR}"),
        format!("BPFTRACE={BPFTRACE}"),
    ] {
        assert!(INSTALL.contains(&value), "{value}");
    }
    for name in [
        LOG_FILE_NAME,
        PROGRAM_FILE_NAME,
        &format!("target/release/{BINARY_NAME}"),
    ] {
        assert!(INSTALL.contains(name), "{name}");
    }
}

#[test]
fn tray_reads_the_alerts_file_from_the_installed_copy() {
    assert!(TRAY_CONSTANTS.contains(&format!("\"{LOG_DIR}/{ALERT_FILE_NAME}\"")));
    let start = format!("ExecStart=/usr/bin/python3 {INSTALL_DIR}/tray/agent_watch_tray.py\n");
    assert!(TRAY_UNIT.contains(&start));
    assert!(INSTALL.contains("TRAY_UNIT=agent-watch-tray.service"));
    assert!(INSTALL.contains("tray/{constants,alerts,network,agent_watch_tray}.py"));
}

#[test]
fn suite_size() {
    let count: usize = SOURCES
        .iter()
        .map(|source| source.matches(TEST_MARK).count())
        .sum();
    assert_eq!(count, EXPECTED_TESTS);
}
