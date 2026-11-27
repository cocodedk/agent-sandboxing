//! Parsing bpftrace JSON lines into records, root names and the dedupe window.

use std::time::Instant;

use serde_json::{Value, json};

use super::Events;
use crate::constants::{DEDUPE_WINDOW, SEEN_MAX};
use crate::danger::Credentials;
use crate::program::Names;
use crate::record::Record;

/// The one home the test credentials sit under.
pub(super) const HOME: &str = "/home/u";

/// One bpftrace -f json printf line; the comm byte length goes before comm.
pub(super) fn printf(kind: &str, pid: i64, root: i64, comm: &str, detail: &[&str]) -> String {
    let head = format!("{kind}\t{pid}\t{root}\t{}\t{comm}", comm.len());
    let data = [head.as_str()]
        .iter()
        .chain(detail)
        .copied()
        .collect::<Vec<_>>();
    json!({"type": "printf", "data": data.join("\t") + "\n"}).to_string()
}

pub(super) fn events() -> Events {
    let credentials = Credentials::new(&[HOME.to_string()]);
    Events::new(Names::from([(7, "pi".to_string())]), credentials)
}

fn one(events: &mut Events, line: &str) -> Record {
    let mut records = events.handle(line);
    assert_eq!(records.len(), 1);
    records.remove(0)
}

/// One field of the one record a line gives.
fn get(events: &mut Events, line: &str, key: &str) -> Value {
    one(events, line)[key].clone()
}

fn message(record: &Record) -> &str {
    record["message"].as_str().unwrap()
}

#[test]
fn seed_records() {
    let [record] = &events().seed_records()[..] else {
        panic!("one seed record")
    };
    assert_eq!(
        (&record["kind"], &record["agent"], &record["agent_pid"]),
        (&json!("agent"), &json!("pi"), &json!(7))
    );
}

#[test]
fn open_path_with_tab_and_newline() {
    let record = one(
        &mut events(),
        &printf("open", 8, 7, "node", &["2", "/tmp/a\tb\nc"]),
    );
    assert_eq!(
        (&record["path"], &record["mode"]),
        (&json!("/tmp/a\tb\nc"), &json!("write"))
    );
    let ids = [
        &record["agent"],
        &record["agent_pid"],
        &record["pid"],
        &record["comm"],
    ];
    assert_eq!(ids, [&json!("pi"), &json!(7), &json!(8), &json!("node")]);
    let keys: Vec<&str> = record.keys().map(String::as_str).take(6).collect();
    assert_eq!(keys, ["time", "kind", "agent", "agent_pid", "pid", "comm"]);
}

#[test]
fn read_exec_connect_dns() {
    let mut events = events();
    let open = printf("open", 8, 7, "n", &["0", "/etc/hosts"]);
    assert_eq!(get(&mut events, &open, "mode"), "read");
    let exec = printf("exec", 8, 7, "git", &["/usr/bin/git"]);
    assert_eq!(get(&mut events, &exec, "path"), "/usr/bin/git");
    let connect = one(&mut events, &printf("connect", 8, 7, "n", &["443", "::1"]));
    assert_eq!(
        (&connect["addr"], &connect["port"]),
        (&json!("::1"), &json!(443))
    );
    let dns = printf("dns", 8, 7, "n", &["example.org"]);
    assert_eq!(get(&mut events, &dns, "host"), "example.org");
}

#[test]
fn agent_names_its_root_and_end_is_kept() {
    let mut events = events();
    events.handle(&printf("agent", 30, 30, "claude", &[""]));
    let dns = printf("dns", 31, 30, "c", &["a.b"]);
    assert_eq!(get(&mut events, &dns, "agent"), "claude");
    one(&mut events, &printf("end", 30, 30, "claude", &[""]));
    one(&mut events, &printf("end", 30, 30, "claude", &[""]));
    assert_eq!(
        get(&mut events, &printf("dns", 1, 99, "c", &["a.b"]), "agent"),
        "?"
    );
}

#[test]
fn dedupe_window_and_reset() {
    let (mut events, start) = (events(), Instant::now());
    let line = printf("exec", 8, 7, "n", &["/bin/ls"]);
    assert_eq!(events.handle_at(&line, start).len(), 1);
    assert!(events.handle_at(&line, start).is_empty());
    let other = printf("exec", 9, 7, "m", &["/bin/cat"]);
    assert_eq!(events.handle_at(&other, start).len(), 1);
    assert_eq!(events.handle_at(&line, start + DEDUPE_WINDOW).len(), 1);
    assert!(events.handle_at(&line, start + DEDUPE_WINDOW).is_empty());
}

#[test]
fn lost_events_both_shapes() {
    for line in [
        json!({"type": "lost_events", "count": 5}),
        json!({"type": "lost_events", "data": {"events": 5}}),
    ] {
        let record = one(&mut events(), &line.to_string());
        assert_eq!(
            (&record["kind"], &record["count"]),
            (&json!("lost"), &json!(5))
        );
    }
}

#[test]
fn attached_unknown_and_unreadable() {
    let mut events = events();
    let attached = one(
        &mut events,
        &json!({"type": "attached_probes", "count": 7}).to_string(),
    );
    assert_eq!(message(&attached), "attached 7 probes");
    let other = one(
        &mut events,
        &json!({"type": "runtime_error", "data": "x"}).to_string(),
    );
    assert_eq!(message(&other), "bpftrace runtime_error: \"x\"");
    let short = json!({"type": "printf", "data": "open\t8\n"}).to_string();
    let bad_len = json!({"type": "printf", "data": "dns\t8\t7\t9\tn\ta.b\n"}).to_string();
    let bad_port = printf("connect", 8, 7, "n", &["x", "1.2.3.4"]);
    let mid = printf("open", 8, 7, "æ", &["0", "/x"]).replace("\\t2\\t", "\\t1\\t");
    let lossy = printf("open", 8, 7, "\u{FFFD}\t\tx", &["0", "/s"]).replace("\\t6\\t", "\\t4\\t");
    for bad in ["not json", &short, &bad_len, &bad_port, &mid, &lossy, "[]"] {
        assert_eq!(one(&mut events, bad)["kind"], "tracer");
    }
}

#[test]
fn comm_with_tabs_and_multibyte_keeps_fields() {
    let mut events = events();
    let record = one(
        &mut events,
        &printf("open", 8, 7, "a\t1\tæ", &["0", "/etc/hosts"]),
    );
    let fields = [&record["comm"], &record["mode"], &record["path"]];
    assert_eq!(
        fields,
        [&json!("a\t1\tæ"), &json!("read"), &json!("/etc/hosts")]
    );
    let connect = one(
        &mut events,
        &printf("connect", 8, 7, "a\tb", &["443", "1.2.3.4"]),
    );
    let fields = [&connect["comm"], &connect["port"], &connect["addr"]];
    assert_eq!(fields, [&json!("a\tb"), &json!(443), &json!("1.2.3.4")]);
}

#[test]
fn dedupe_set_is_bounded() {
    let mut events = events();
    one(&mut events, &printf("exec", 8, 7, "n", &["/a"]));
    events.seen.extend(0..SEEN_MAX as u64);
    one(&mut events, &printf("exec", 8, 7, "n", &["/c"]));
    assert_eq!(events.seen.len(), 1);
    one(&mut events, &printf("exec", 8, 7, "n", &["/a"]));
}

/// bpftrace may escape a byte to a lone surrogate, which a Rust string cannot hold.
#[test]
fn lone_surrogate_escape_is_kept_as_text() {
    let line = r#"{"type": "printf", "data": "dns\t8\t7\t1\tn\t\ud800\n"}"#;
    let record = one(&mut events(), line);
    assert_eq!(record["kind"], Value::from("tracer"));
    assert!(message(&record).ends_with(line));
}
