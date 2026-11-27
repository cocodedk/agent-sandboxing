//! Turn bpftrace JSON lines into log records: root names, alerts, dedupe.

use std::collections::HashSet;
use std::hash::{BuildHasher, RandomState};
use std::time::Instant;

use serde_json::{Value, json};

use crate::constants::{
    DEDUPE_WINDOW, FIELD_AGENT, FIELD_AGENT_PID, FIELD_COMM, FIELD_PID, FIELD_SEP, HEAD_FIELDS,
    HEX_RADIX, JSON_ATTACHED, JSON_COUNT, JSON_DATA, JSON_EVENTS, JSON_LOST, JSON_PRINTF,
    JSON_TYPE, KIND_AGENT, KIND_ALERT, KIND_OPEN, KIND_TLS, LINE_END, LOST_COUNT_END,
    LOST_COUNT_KEY, LOST_MARK, NEVER_DROPPED, SEEN_MAX, UNKNOWN_AGENT,
};
use crate::danger::Credentials;
use crate::program::Names;
use crate::record::{Details, Record, details, head, lost, split_comm, tracer};

/// A lost-events line whose count bpftrace printed in hex without quotes, which is not JSON.
fn lost_in_hex(line: &str) -> Option<Vec<Record>> {
    if !line.contains(LOST_MARK) {
        return None;
    }
    let rest = line.split_once(LOST_COUNT_KEY)?.1;
    let digits = rest.split(LOST_COUNT_END).next()?.trim();
    let count = u64::from_str_radix(digits, HEX_RADIX).ok()?;
    Some(vec![lost(json!(count))])
}

/// Keeps root names, the credential paths and the dedupe window; handle() returns the records to write.
pub struct Events {
    names: Names,
    credentials: Credentials,
    window_start: Instant,
    seen: HashSet<u64>,
    keys: RandomState,
}

impl Events {
    pub fn new(names: Names, credentials: Credentials) -> Self {
        Events {
            names,
            credentials,
            window_start: Instant::now(),
            seen: HashSet::new(),
            keys: RandomState::new(),
        }
    }

    /// An agent record for each agent that was running before the tracer started.
    pub fn seed_records(&self) -> Vec<Record> {
        self.names
            .iter()
            .map(|(&pid, name)| self.record(KIND_AGENT, pid, pid, name, Details::new()))
            .collect()
    }

    /// Records for one line of bpftrace output; anything unexpected becomes a tracer record.
    pub fn handle(&mut self, line: &str) -> Vec<Record> {
        self.handle_at(line, Instant::now())
    }

    fn handle_at(&mut self, line: &str, now: Instant) -> Vec<Record> {
        let parsed = self.parse(line, now).or_else(|| lost_in_hex(line));
        parsed.unwrap_or_else(|| {
            vec![tracer(&format!(
                "unreadable bpftrace output: {}",
                line.trim()
            ))]
        })
    }

    fn parse(&mut self, line: &str, now: Instant) -> Option<Vec<Record>> {
        let event: Value = serde_json::from_str(line).ok()?;
        let kind = event.get(JSON_TYPE)?;
        let data = event.get(JSON_DATA);
        let message = match kind.as_str() {
            Some(JSON_PRINTF) => return self.printf(data?.as_str()?, now),
            Some(JSON_LOST) => {
                let count = event.get(JSON_COUNT).or_else(|| data?.get(JSON_EVENTS));
                return Some(vec![lost(count.cloned().unwrap_or(Value::Null))]);
            }
            Some(JSON_ATTACHED) => {
                format!(
                    "attached {} probes",
                    event.get(JSON_COUNT).unwrap_or(&Value::Null)
                )
            }
            name => format!(
                "bpftrace {}: {}",
                name.map_or(kind.to_string(), str::to_string),
                data.unwrap_or(&event)
            ),
        };
        Some(vec![tracer(&message)])
    }

    fn printf(&mut self, data: &str, now: Instant) -> Option<Vec<Record>> {
        let data = data.strip_suffix(LINE_END).unwrap_or(data);
        let mut fields = data.splitn(HEAD_FIELDS + 1, FIELD_SEP);
        let mut next = || fields.next();
        let (kind, pid, root, comm_len, rest) = (next()?, next()?, next()?, next()?, next()?);
        let (comm, detail) = split_comm(rest, comm_len.parse().ok()?)?;
        let (pid, root): (i64, i64) = (pid.parse().ok()?, root.parse().ok()?);
        if kind == KIND_AGENT {
            self.names.insert(root, comm.to_string());
        }
        let mut records = Vec::new();
        // A repeat is dropped before its record is built: most events are repeats.
        if NEVER_DROPPED.contains(&kind) || self.first_in_window(root, kind, detail, now) {
            let Some(details) = details(kind, detail) else {
                // A ClientHello without a site name (an address, or cut short) is no record.
                return (kind == KIND_TLS).then(Vec::new);
            };
            records.push(self.record(kind, pid, root, comm, details));
        }
        if kind == KIND_OPEN {
            records.extend(self.alert(pid, root, comm, detail, now));
        }
        Some(records)
    }

    /// An alert when comm opened a credential path that is not its own; checked before the open
    /// dedupe, which ignores comm, with a repeat key of its own that includes comm.
    fn alert(
        &mut self,
        pid: i64,
        root: i64,
        comm: &str,
        detail: &str,
        now: Instant,
    ) -> Option<Record> {
        let (_, path) = detail.split_once(FIELD_SEP)?;
        let key = || format!("{comm}{FIELD_SEP}{detail}");
        if !self.credentials.foreign(comm, path)
            || !self.first_in_window(root, KIND_ALERT, &key(), now)
        {
            return None;
        }
        Some(self.record(KIND_ALERT, pid, root, comm, details(KIND_OPEN, detail)?))
    }

    fn record(&self, kind: &str, pid: i64, root: i64, comm: &str, details: Details) -> Record {
        let agent = self.names.get(&root).map_or(UNKNOWN_AGENT, String::as_str);
        let mut record = head(kind);
        record.insert(FIELD_AGENT.into(), json!(agent));
        record.insert(FIELD_AGENT_PID.into(), json!(root));
        record.insert(FIELD_PID.into(), json!(pid));
        record.insert(FIELD_COMM.into(), json!(comm));
        for (key, value) in details {
            record.insert(key.into(), value);
        }
        record
    }

    fn first_in_window(&mut self, root: i64, kind: &str, detail: &str, now: Instant) -> bool {
        if now.saturating_duration_since(self.window_start) >= DEDUPE_WINDOW
            || self.seen.len() >= SEEN_MAX
        {
            self.seen.clear();
            self.window_start = now;
        }
        self.seen.insert(self.keys.hash_one((root, kind, detail))) // a u64: bounded size
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "alert_tests.rs"]
mod alert_tests;

#[cfg(test)]
#[path = "network_tests.rs"]
mod network_tests;
