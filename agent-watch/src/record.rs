//! Build log records: the head every record starts with and the per-kind fields.

use serde_json::{Map, Value, json};

use crate::clock::now_iso;
use crate::constants::{
    FIELD_ADDR, FIELD_COUNT, FIELD_HOST, FIELD_KIND, FIELD_MESSAGE, FIELD_MODE, FIELD_PATH,
    FIELD_PORT, FIELD_SEP, FIELD_TIME, KIND_CONNECT, KIND_DNS, KIND_EXEC, KIND_LOST, KIND_OPEN,
    KIND_TLS, KIND_TRACER, MODE_READ, MODE_WRITE, WRITE_BIT_CLEAR,
};
use crate::sni::{from_hex, server_name};

/// One log line: keys keep the order they were inserted in.
pub type Record = Map<String, Value>;

/// The fields every record starts with: the time and the kind.
pub fn head(kind: &str) -> Record {
    let mut record = Record::new();
    record.insert(FIELD_TIME.into(), json!(now_iso()));
    record.insert(FIELD_KIND.into(), json!(kind));
    record
}

/// A record about the tracer itself: start, stop, warnings.
pub fn tracer(message: &str) -> Record {
    let mut record = head(KIND_TRACER);
    record.insert(FIELD_MESSAGE.into(), json!(message));
    record
}

/// A record of events the kernel dropped: the log has a gap.
pub fn lost(count: Value) -> Record {
    let mut record = head(KIND_LOST);
    record.insert(FIELD_COUNT.into(), count);
    record
}

/// A record's per-kind fields.
pub type Details = Vec<(&'static str, Value)>;

/// The per-kind fields; the free-text part is always last, so it may hold tabs.
pub fn details(kind: &str, detail: &str) -> Option<Details> {
    let (head, tail) = detail.split_once(FIELD_SEP).unwrap_or((detail, ""));
    Some(match kind {
        KIND_OPEN => {
            let mode = if head == WRITE_BIT_CLEAR {
                MODE_READ
            } else {
                MODE_WRITE
            };
            vec![(FIELD_PATH, json!(tail)), (FIELD_MODE, json!(mode))]
        }
        KIND_EXEC => vec![(FIELD_PATH, json!(detail))],
        KIND_CONNECT => vec![
            (FIELD_ADDR, json!(tail)),
            (FIELD_PORT, json!(head.parse::<i64>().ok()?)),
        ],
        KIND_DNS => vec![(FIELD_HOST, json!(detail))],
        KIND_TLS => {
            let (addr, hello) = tail.split_once(FIELD_SEP)?;
            vec![
                (FIELD_HOST, json!(server_name(&from_hex(hello)?)?)),
                (FIELD_ADDR, json!(addr)),
                (FIELD_PORT, json!(head.parse::<i64>().ok()?)),
            ]
        }
        _ => vec![],
    })
}

/// comm and the detail after it, comm cut by its byte length; a replaced byte breaks that length.
pub fn split_comm(rest: &str, comm_len: usize) -> Option<(&str, &str)> {
    let comm = rest.get(..comm_len)?;
    let detail = rest.get(comm_len..)?.strip_prefix(FIELD_SEP)?;
    (!comm.contains(char::REPLACEMENT_CHARACTER)).then_some((comm, detail))
}
