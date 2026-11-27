//! Fill the bpftrace template and find the agents already running in /proc.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use tempfile::NamedTempFile;

use crate::constants::{
    AGENT_NAMES, ARGV_NAME_SLOTS, ARGV_SEP, CHILD_TYPE_PREFIX, CLIENT_HELLO, DEBUG_FLAG_PREFIX,
    DEBUG_MAX_ARGS, DEBUG_PREFIX_READ, HELLO_TYPE_AT, ITER_IOVEC, ITER_UBUF, MAX_MAP_KEYS,
    MAX_STRLEN, PLACEHOLDER_CHILD_TYPE, PLACEHOLDER_CHILD_TYPE_LEN, PLACEHOLDER_CLIENT_HELLO,
    PLACEHOLDER_DEBUG_ARGS, PLACEHOLDER_DEBUG_LEN, PLACEHOLDER_DEBUG_PREFIX,
    PLACEHOLDER_DEBUG_READ, PLACEHOLDER_HELLO_TYPE_AT, PLACEHOLDER_ITER_IOVEC,
    PLACEHOLDER_ITER_UBUF, PLACEHOLDER_MAP_KEYS, PLACEHOLDER_MATCH, PLACEHOLDER_SEEDS,
    PLACEHOLDER_STRLEN, PLACEHOLDER_TLS_HANDSHAKE, PLACEHOLDER_TLS_READ, PROC_CMDLINE, PROC_STAT,
    PROGRAM_FILE_NAME, STAT_COMM_CLOSE, STAT_COMM_OPEN, STAT_PPID_FIELD, TITLE_SEP, TLS_HANDSHAKE,
    TLS_READ,
};

/// Agent roots: pid -> name.
pub type Names = BTreeMap<i64, String>;
/// Every marked pid -> its nearest root pid.
pub type Seeds = BTreeMap<i64, i64>;

/// The bpftrace test that comm is one of the agent names.
fn match_condition() -> String {
    let tests: Vec<String> = AGENT_NAMES
        .iter()
        .map(|name| format!("comm == \"{name}\""))
        .collect();
    tests.join(" || ")
}

/// One map assignment per pid; the types keep anything but a number out of the program.
fn seed_block(seeds: &Seeds) -> String {
    let lines: Vec<String> = seeds
        .iter()
        .map(|(pid, root)| format!("  @agent[(uint64){pid}] = (uint64){root};"))
        .collect();
    lines.join("\n")
}

/// The runnable program: the template with every placeholder replaced.
pub fn fill(template: &str, seeds: &Seeds) -> String {
    template
        .replace(PLACEHOLDER_MATCH, &match_condition())
        .replace(PLACEHOLDER_SEEDS, &seed_block(seeds))
        .replace(PLACEHOLDER_STRLEN, &MAX_STRLEN.to_string())
        .replace(PLACEHOLDER_MAP_KEYS, &MAX_MAP_KEYS.to_string())
        .replace(PLACEHOLDER_DEBUG_PREFIX, DEBUG_FLAG_PREFIX)
        .replace(PLACEHOLDER_DEBUG_LEN, &DEBUG_FLAG_PREFIX.len().to_string())
        .replace(PLACEHOLDER_DEBUG_READ, &DEBUG_PREFIX_READ.to_string())
        .replace(PLACEHOLDER_DEBUG_ARGS, &DEBUG_MAX_ARGS.to_string())
        .replace(PLACEHOLDER_CHILD_TYPE, CHILD_TYPE_PREFIX)
        .replace(
            PLACEHOLDER_CHILD_TYPE_LEN,
            &CHILD_TYPE_PREFIX.len().to_string(),
        )
        .replace(PLACEHOLDER_ITER_UBUF, &ITER_UBUF.to_string())
        .replace(PLACEHOLDER_ITER_IOVEC, &ITER_IOVEC.to_string())
        .replace(PLACEHOLDER_TLS_HANDSHAKE, &TLS_HANDSHAKE.to_string())
        .replace(PLACEHOLDER_HELLO_TYPE_AT, &HELLO_TYPE_AT.to_string())
        .replace(PLACEHOLDER_CLIENT_HELLO, &CLIENT_HELLO.to_string())
        .replace(PLACEHOLDER_TLS_READ, &TLS_READ.to_string())
}

/// The filled program in a temporary file, deleted when dropped; tempfile makes it mode 0600.
pub fn write_filled(text: &str) -> io::Result<NamedTempFile> {
    let mut file = NamedTempFile::with_suffix(PROGRAM_FILE_NAME)?;
    file.write_all(text.as_bytes())?;
    Ok(file)
}

/// comm and ppid from /proc/<pid>/stat; comm may hold spaces and parentheses.
fn read_stat(proc_pid: &Path) -> Option<(String, i64)> {
    let bytes = fs::read(proc_pid.join(PROC_STAT)).ok()?;
    let stat = String::from_utf8_lossy(&bytes);
    let (head, tail) = stat.rsplit_once(STAT_COMM_CLOSE)?;
    let comm = head.split_once(STAT_COMM_OPEN)?.1;
    let ppid = tail.split_whitespace().nth(STAT_PPID_FIELD)?.parse().ok()?;
    Some((comm.to_string(), ppid))
}

/// The agent name this process runs, from comm or the argv[0]/argv[1] basenames;
/// a process started with the debug flag is a root under its comm, unless it is a helper.
fn agent_name(comm: &str, cmdline: &[u8]) -> Option<String> {
    if AGENT_NAMES.contains(&comm) {
        return Some(comm.to_string());
    }
    let inner = cmdline.strip_suffix(&[ARGV_SEP]).unwrap_or(cmdline);
    let sep = if inner.contains(&ARGV_SEP) {
        ARGV_SEP
    } else {
        TITLE_SEP
    };
    let args: Vec<&[u8]> = inner
        .split(|byte| *byte == sep)
        .take(DEBUG_MAX_ARGS)
        .collect();
    for arg in args.iter().take(ARGV_NAME_SLOTS) {
        let arg = String::from_utf8_lossy(arg);
        let name = Path::new(arg.as_ref())
            .file_name()
            .and_then(|name| name.to_str());
        if let Some(name) = name.filter(|name| AGENT_NAMES.contains(name)) {
            return Some(name.to_string());
        }
    }
    let has = |prefix: &str| args.iter().any(|arg| arg.starts_with(prefix.as_bytes()));
    let debug = has(DEBUG_FLAG_PREFIX) && !has(CHILD_TYPE_PREFIX);
    debug.then(|| comm.to_string())
}

/// One process: its pid, ppid and agent name; None if it ended while read or is not a pid.
fn read_process(entry: &Path) -> Option<(i64, i64, Option<String>)> {
    let file_name = entry.file_name()?.to_str()?;
    if !file_name.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let pid = file_name.parse().ok()?;
    let (comm, ppid) = read_stat(entry)?;
    let cmdline = fs::read(entry.join(PROC_CMDLINE)).ok()?;
    let name = agent_name(&comm, &cmdline);
    Some((pid, ppid, name))
}

/// Agent roots (pid -> name) and every marked pid (pid -> nearest root pid).
pub fn scan(proc_dir: &Path) -> io::Result<(Names, Seeds)> {
    let mut parents = HashMap::new();
    let mut names = Names::new();
    for entry in fs::read_dir(proc_dir)? {
        let Some((pid, ppid, name)) = read_process(&entry?.path()) else {
            continue;
        };
        parents.insert(pid, ppid);
        if let Some(name) = name {
            names.insert(pid, name);
        }
    }
    let mut seeds = Seeds::new();
    for &pid in parents.keys() {
        let (mut ancestor, mut visited) = (pid, HashSet::new());
        while !names.contains_key(&ancestor) && visited.insert(ancestor) {
            let Some(&parent) = parents.get(&ancestor) else {
                break;
            };
            ancestor = parent;
        }
        if names.contains_key(&ancestor) {
            seeds.insert(pid, ancestor);
        }
    }
    Ok((names, seeds))
}

#[cfg(test)]
#[path = "program_tests.rs"]
pub(crate) mod tests;
