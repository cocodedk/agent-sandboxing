//! Alerts-file records: a credential path opened by a program that is not its own tool, and lost events.

use serde_json::{Value, json};

use super::tests::{HOME, events, printf};

fn open(comm: &str, mode: &str, path: &str) -> String {
    printf("open", 9, 7, comm, &[mode, &format!("{HOME}/{path}")])
}

fn kinds(records: &[serde_json::Map<String, Value>]) -> Vec<&str> {
    records
        .iter()
        .map(|record| record["kind"].as_str().unwrap())
        .collect()
}

#[test]
fn a_foreign_read_gives_the_open_and_an_alert() {
    let records = events().handle(&open("cat", "0", ".ssh/id_ed25519"));
    assert_eq!(kinds(&records), ["open", "alert"]);
    let alert = &records[1];
    assert_eq!(
        (&alert["agent"], &alert["comm"], &alert["mode"]),
        (&json!("pi"), &json!("cat"), &json!("read"))
    );
    assert_eq!(alert["path"], format!("{HOME}/.ssh/id_ed25519"));
}

#[test]
fn the_folder_tool_gives_no_alert() {
    let records = events().handle(&open("ssh", "0", ".ssh/id_ed25519"));
    assert_eq!(kinds(&records), ["open"]);
}

#[test]
fn a_second_reader_is_caught_after_the_open_is_deduped() {
    let mut events = events();
    events.handle(&open("ssh", "0", ".ssh/id_ed25519"));
    let second = events.handle(&open("cat", "0", ".ssh/id_ed25519"));
    assert_eq!(kinds(&second), ["alert"]);
    assert!(
        events
            .handle(&open("cat", "0", ".ssh/id_ed25519"))
            .is_empty()
    );
}

#[test]
fn a_write_is_an_alert_with_mode_write() {
    let records = events().handle(&open("python3", "2", ".aws/credentials"));
    assert_eq!(records[1]["mode"], "write");
}

#[test]
fn lost_events_with_an_unquoted_hex_count() {
    let line = r#"{"type": "lost_events", "count": 4c, "data": {"events": 4c}}"#;
    let records = events().handle(line);
    assert_eq!(
        (&records[0]["kind"], &records[0]["count"]),
        (&json!("lost"), &json!(76))
    );
}
