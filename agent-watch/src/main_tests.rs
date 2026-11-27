//! The run loop: records, stderr, lost events, exit codes and signals.

use std::fs;
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use serde_json::Value;
use tempfile::TempDir;

use super::{CHILD, Logs, STOPPING, run, serve};
use crate::constants::{
    ALERT_FILE_NAME, EXIT_OK, EXIT_TRACER_DIED, KEEP_ROTATED, LOG_FILE_NAME, MAX_LOG_BYTES,
};
use crate::danger::Credentials;
use crate::events::Events;
use crate::program::Names;
use crate::program::tests::{TEMPLATE, fake_process};
use crate::writer::Writer;

const SHELL: &str = "/bin/sh";
const DNS: &str = r#"{"type": "printf", "data": "dns\t8\t7\t4\tnode\texample.org\n"}"#;
const LOST: &str = r#"{"type": "lost_events", "count": 3}"#;
const FOREIGN: &str = r#"{"type": "printf", "data": "open\t9\t7\t3\tcat\t0\t/home/u/.ssh/id\n"}"#;
const REPLACED: char = char::REPLACEMENT_CHARACTER;
const SIGNAL_DELAY: Duration = Duration::from_millis(500);
const POLL: Duration = Duration::from_millis(10);

/// run() uses process-wide signal state: one test at a time.
static RUNS: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> MutexGuard<'static, ()> {
    let guard = RUNS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    STOPPING.store(false, Ordering::SeqCst); // a signal test leaves it set
    guard
}

fn records(path: &std::path::Path) -> Vec<Value> {
    let text = fs::read_to_string(path).unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn writer(path: &std::path::Path) -> Mutex<Writer> {
    Mutex::new(Writer::new(path, MAX_LOG_BYTES, KEEP_ROTATED, None).unwrap())
}

/// The exit code, the main log's records, the alerts file's records, and how many went to the notifier.
fn run_script(script: &str) -> (u8, Vec<Value>, Vec<Value>, usize) {
    let tmp = TempDir::new().unwrap();
    let (path, alerts) = (
        tmp.path().join(LOG_FILE_NAME),
        tmp.path().join(ALERT_FILE_NAME),
    );
    let (notify, notified) = channel();
    let logs = Logs {
        all: writer(&path),
        alerts: writer(&alerts),
        notify,
    };
    let command = [SHELL, "-c", script];
    let credentials = Credentials::new(&["/home/u".to_string()]);
    let mut events = Events::new(Names::from([(7, "pi".to_string())]), credentials);
    let code = run(&command, &logs, &mut events).unwrap();
    (
        code,
        records(&path),
        records(&alerts),
        notified.try_iter().count(),
    )
}

fn kinds(records: &[Value]) -> Vec<&str> {
    let mut kinds: Vec<&str> = records
        .iter()
        .map(|record| record["kind"].as_str().unwrap())
        .collect();
    kinds.sort(); // stderr is read on its own thread
    kinds
}

fn messages(records: &[Value]) -> Vec<&str> {
    records
        .iter()
        .filter_map(|record| record["message"].as_str())
        .collect()
}

#[test]
fn tracer_death_is_logged_and_fails() {
    let _guard = one_at_a_time();
    let lines = format!("'{DNS}' '{LOST}' '{FOREIGN}'");
    let script = format!("printf '%s\\n' {lines}; echo 'WARNING: x' >&2; exit 3");
    let (code, records, alerts, notified) = run_script(&script);
    assert_eq!(code, EXIT_TRACER_DIED);
    let all = [
        "agent", "alert", "dns", "lost", "open", "tracer", "tracer", "tracer",
    ];
    assert_eq!(kinds(&records), all);
    assert_eq!(kinds(&alerts), ["alert", "lost"]);
    assert_eq!(notified, 1); // the alert, not the lost record
    let messages = messages(&records);
    assert!(messages.contains(&"bpftrace stderr: WARNING: x"));
    assert_eq!(messages.last(), Some(&"bpftrace stopped, exit code 3"));
}

#[test]
fn bytes_that_are_not_utf8_do_not_crash() {
    let _guard = one_at_a_time();
    let (code, records, _, _) = run_script(r"printf '\377\n'; printf '\376\n' >&2");
    assert_eq!(code, EXIT_TRACER_DIED);
    let messages = messages(&records);
    assert!(messages.contains(&format!("unreadable bpftrace output: {REPLACED}").as_str()));
    assert!(messages.contains(&format!("bpftrace stderr: {REPLACED}").as_str()));
}

#[test]
fn signal_stops_cleanly() {
    let _guard = one_at_a_time();
    let sender = thread::spawn(|| {
        while CHILD.load(Ordering::SeqCst) == 0 {
            thread::sleep(POLL);
        }
        thread::sleep(SIGNAL_DELAY);
        // SAFETY: signals this test process, whose handler run() installed.
        unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
    });
    let (code, records, _, _) = run_script(&format!("printf '%s\\n' '{DNS}'; exec sleep 30"));
    sender.join().unwrap();
    assert_eq!(code, EXIT_OK);
    assert_eq!(records[2]["host"], "example.org");
    let last = records.last().unwrap()["message"].as_str().unwrap();
    assert_eq!(
        last,
        format!("bpftrace stopped, exit code -{}", libc::SIGINT)
    );
}

#[test]
fn serve_seeds_fills_and_logs() {
    let _guard = one_at_a_time();
    let tmp = TempDir::new().unwrap();
    let (proc, program, log) = (
        tmp.path().join("proc"),
        tmp.path().join("prog.bt"),
        tmp.path().join("log"),
    );
    fs::create_dir(&proc).unwrap();
    fake_process(&proc, 5, "opencode", 1, &[]);
    fs::write(&program, TEMPLATE).unwrap();
    let check = r#"grep -qF '(uint64)5;' "$0" && ! grep -qF '@@' "$0""#;
    let log_file = log.join(LOG_FILE_NAME);
    let code = serve(&proc, &program, &log_file, &[SHELL, "-c", check]).unwrap();
    let records = records(&log_file);
    assert_eq!(code, EXIT_TRACER_DIED);
    assert!(log.join(ALERT_FILE_NAME).exists());
    assert_eq!(records[1]["agent"], "opencode");
    assert_eq!(
        records.last().unwrap()["message"],
        "bpftrace stopped, exit code 0"
    );
}
