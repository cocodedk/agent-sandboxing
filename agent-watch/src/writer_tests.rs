//! Appending, rotation, gzip, keep N and file modes.

use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde_json::{Value, json};
use tempfile::TempDir;

use super::{Writer, log_owner};
use crate::constants::{KEEP_ROTATED, LOG_DIR_MODE, LOG_FILE_MODE, MAX_LOG_BYTES, ROOT_UID};
use crate::record::Record;

const OTHER_UID: u32 = 1000;

/// n stays one digit: every line has the same length.
fn record(n: u32) -> Record {
    json!({"kind": "dns", "n": n, "host": "x".repeat(40)})
        .as_object()
        .unwrap()
        .clone()
}

fn line_bytes() -> u64 {
    serde_json::to_string(&record(0)).unwrap().len() as u64 + 1
}

fn setup() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("log").join("w.jsonl");
    (tmp, path)
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn numbers(text: &str) -> Vec<u64> {
    text.lines()
        .map(|line| {
            serde_json::from_str::<Value>(line).unwrap()["n"]
                .as_u64()
                .unwrap()
        })
        .collect()
}

fn writer(path: &Path) -> Writer {
    Writer::new(path, MAX_LOG_BYTES, KEEP_ROTATED, None).unwrap()
}

#[test]
fn append_and_modes() {
    let (_tmp, path) = setup();
    writer(&path).write(&record(0)).unwrap();
    writer(&path)
        .write(json!({"kind": "end"}).as_object().unwrap())
        .unwrap();
    let text = fs::read_to_string(&path).unwrap();
    let kinds: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["kind"].clone())
        .collect();
    assert_eq!(kinds, [json!("dns"), json!("end")]);
    assert_eq!(
        (mode(&path), mode(path.parent().unwrap())),
        (LOG_FILE_MODE, LOG_DIR_MODE)
    );
}

#[test]
fn rotate_gzip_round_trip_and_keep() {
    let (_tmp, path) = setup();
    let mut writer = Writer::new(&path, 2 * line_bytes(), 2, None).unwrap();
    for n in 0..7 {
        writer.write(&record(n)).unwrap();
    }
    let mut names: Vec<String> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["w.jsonl", "w.jsonl.1.gz", "w.jsonl.2.gz"]);
    let gunzip = |number| {
        let mut text = String::new();
        GzDecoder::new(fs::File::open(writer.rotated(number)).unwrap())
            .read_to_string(&mut text)
            .unwrap();
        text
    };
    assert_eq!(
        (numbers(&gunzip(1)), numbers(&gunzip(2))),
        (vec![4, 5], vec![2, 3])
    );
    assert_eq!(numbers(&fs::read_to_string(&path).unwrap()), [6]);
    assert_eq!(mode(&writer.rotated(1)), LOG_FILE_MODE);
}

#[test]
fn record_larger_than_cap_is_still_written() {
    let (_tmp, path) = setup();
    let mut writer = Writer::new(&path, 1, KEEP_ROTATED, None).unwrap();
    writer.write(&record(0)).unwrap();
    assert!(!writer.rotated(1).exists());
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[test]
fn owner_is_set_only_as_root() {
    assert_eq!(log_owner(OTHER_UID).unwrap(), None);
    let Some((ROOT_UID, log_gid)) = log_owner(ROOT_UID).unwrap() else {
        panic!("root gets the log group")
    };
    let (tmp, path) = setup();
    let me = fs::metadata(tmp.path()).unwrap();
    writer(&path); // no owner: the files keep the creator's group
    assert_eq!(fs::metadata(&path).unwrap().gid(), me.gid());
    // As root the uid is root's; here our own uid, and the log group we belong to.
    Writer::new(
        &path,
        MAX_LOG_BYTES,
        KEEP_ROTATED,
        Some((me.uid(), log_gid)),
    )
    .unwrap();
    for owned in [&path, path.parent().unwrap()] {
        let meta = fs::metadata(owned).unwrap();
        assert_eq!((meta.uid(), meta.gid()), (me.uid(), log_gid));
    }
}
