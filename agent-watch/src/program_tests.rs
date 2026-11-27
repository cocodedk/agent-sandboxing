//! Template fill and the /proc seed scan.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::{Names, Seeds, fill, scan, seed_block};
use crate::constants::{
    AGENT_NAMES, CHILD_TYPE_PREFIX, DEBUG_FLAG_PREFIX, DEBUG_MAX_ARGS, DEBUG_PREFIX_READ,
    MAX_MAP_KEYS, MAX_STRLEN, PROC_CMDLINE, PROC_STAT,
};

pub(crate) const TEMPLATE: &str = include_str!("../agent-watch.bt");
const COMM_MAX_LEN: usize = 15; // TASK_COMM_LEN less the closing zero

/// A /proc/<pid> with the stat and cmdline files the seed scan reads.
pub(crate) fn fake_process(proc: &Path, pid: i64, comm: &str, ppid: i64, argv: &[&str]) {
    let entry = proc.join(pid.to_string());
    fs::create_dir(&entry).unwrap();
    fs::write(
        entry.join(PROC_STAT),
        format!("{pid} ({comm}) S {ppid} 1 1 0 -1\n"),
    )
    .unwrap();
    fs::write(entry.join(PROC_CMDLINE), argv.join("\0") + "\0").unwrap();
}

fn proc_dir() -> TempDir {
    let tmp = TempDir::new().unwrap();
    fs::create_dir(tmp.path().join("self")).unwrap(); // not a pid: skipped
    tmp
}

#[test]
fn names_fit_in_comm() {
    assert!(AGENT_NAMES.iter().all(|name| name.len() <= COMM_MAX_LEN));
}

#[test]
fn fill_leaves_no_placeholder() {
    let text = fill(TEMPLATE, &Seeds::from([(12, 10)]));
    assert!(!text.contains("@@"));
    assert!(text.contains(&format!("max_strlen = {MAX_STRLEN};")));
    assert!(text.contains(&format!("max_map_keys = {MAX_MAP_KEYS};")));
    assert!(text.contains(&format!("while ($i < {DEBUG_MAX_ARGS})")));
    assert!(text.contains(&format!("$text = str($arg, {DEBUG_PREFIX_READ});")));
    for prefix in [DEBUG_FLAG_PREFIX, CHILD_TYPE_PREFIX] {
        assert!(text.contains(&format!("strncmp($text, \"{prefix}\", {})", prefix.len())));
    }
}

#[test]
fn fill_matches_every_name() {
    let text = fill(TEMPLATE, &Seeds::new());
    for name in AGENT_NAMES {
        assert!(text.contains(&format!("comm == \"{name}\"")));
    }
}

#[test]
fn seeds_are_integers_only() {
    let block = seed_block(&Seeds::from([(12, 10), (10, 10)]));
    assert_eq!(
        block,
        "  @agent[(uint64)10] = (uint64)10;\n  @agent[(uint64)12] = (uint64)10;"
    );
    assert!(
        block
            .chars()
            .all(|c| " @aegintu[]()0123456789=;\n".contains(c))
    );
}

#[test]
fn filled_program_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let file = super::write_filled("x").unwrap();
    let mode = file.as_file().metadata().unwrap().permissions().mode() & 0o777;
    assert_eq!(
        (mode, fs::read_to_string(file.path()).unwrap().as_str()),
        (0o600, "x")
    );
}

#[test]
fn comm_argv_descendants_and_nearest_root() {
    let tmp = proc_dir();
    let proc = tmp.path();
    fake_process(proc, 1, "systemd", 0, &[]);
    fake_process(proc, 10, "claude", 1, &[]);
    fake_process(proc, 11, "bash", 10, &[]);
    fake_process(proc, 12, "node", 11, &["node", "/usr/bin/pi", "--x"]);
    fake_process(proc, 13, "a (b) c)", 12, &[]);
    fake_process(proc, 14, "MainThread", 1, &["/opt/codex/codex"]);
    fake_process(proc, 15, "vim", 1, &["vim", "notes", "pi"]);
    let (names, seeds) = scan(proc).unwrap();
    let expected: Names = [(10, "claude"), (12, "pi"), (14, "codex")]
        .map(|(p, n)| (p, n.to_string()))
        .into();
    assert_eq!(names, expected);
    assert_eq!(
        seeds,
        Seeds::from([(10, 10), (11, 10), (12, 12), (13, 12), (14, 14)])
    );
}

#[test]
fn vanished_process_and_cycle() {
    let tmp = proc_dir();
    fake_process(tmp.path(), 20, "x", 21, &[]);
    fake_process(tmp.path(), 21, "y", 20, &[]);
    fs::create_dir(tmp.path().join("22")).unwrap(); // ended between listing and reading
    assert_eq!(scan(tmp.path()).unwrap(), (Names::new(), Seeds::new()));
}

#[test]
fn debug_flag_makes_a_root() {
    let tmp = proc_dir();
    let proc = tmp.path();
    let port = format!("{DEBUG_FLAG_PREFIX}port=9222");
    let pipe = format!("{DEBUG_FLAG_PREFIX}pipe");
    fake_process(
        proc,
        30,
        "chrome",
        1,
        &["/opt/chrome", "--no-first-run", &port],
    );
    fake_process(proc, 31, "chrome", 30, &["/opt/chrome", "--type=renderer"]);
    fake_process(proc, 32, "electron", 1, &["electron", &pipe]);
    fake_process(proc, 33, "chromium", 1, &["chromium", "--remote-debugging"]);
    let (names, seeds) = scan(proc).unwrap();
    let expected: Names = [(30, "chrome"), (32, "electron")]
        .map(|(p, n)| (p, n.to_string()))
        .into();
    assert_eq!(names, expected);
    assert_eq!(seeds, Seeds::from([(30, 30), (31, 30), (32, 32)]));
}

#[test]
fn rewritten_cmdline_makes_a_root_and_helpers_stay_under_it() {
    let tmp = proc_dir();
    let proc = tmp.path();
    let title =
        |extra: &str| format!("/opt/chrome --headless=new {DEBUG_FLAG_PREFIX}port=9223{extra}");
    fake_process(proc, 40, "chrome", 1, &[&title("")]);
    fake_process(proc, 41, "chrome", 40, &[&title(" --type=zygote")]);
    fake_process(proc, 42, "chrome", 41, &[&title(" --type=renderer")]);
    fake_process(proc, 43, "MainThread", 1, &["node /usr/bin/pi --x"]);
    let (names, seeds) = scan(proc).unwrap();
    let expected: Names = [(40, "chrome"), (43, "pi")]
        .map(|(p, n)| (p, n.to_string()))
        .into();
    assert_eq!(names, expected);
    assert_eq!(seeds, Seeds::from([(40, 40), (41, 40), (42, 40), (43, 43)]));
}
