//! Times as ISO 8601 UTC with milliseconds, without a date crate.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::{MILLIS_PER_SECOND, TM_MONTH_BASE, TM_YEAR_BASE};

/// A time in milliseconds since the epoch, ISO 8601 UTC with milliseconds.
pub fn iso(millis: u64) -> String {
    let seconds = (millis / MILLIS_PER_SECOND) as libc::time_t;
    // SAFETY: gmtime_r writes only into the struct it is given; zeroed is a valid tm.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::gmtime_r(&seconds, &mut tm) };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        tm.tm_year + TM_YEAR_BASE,
        tm.tm_mon + TM_MONTH_BASE,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        millis % MILLIS_PER_SECOND
    )
}

/// The current time, ISO 8601 UTC with milliseconds.
pub fn now_iso() -> String {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    iso(since.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::{iso, now_iso};
    #[test]
    fn time_is_iso_utc_millis() {
        let shape: String = now_iso()
            .chars()
            .map(|c| if c.is_ascii_digit() { 'd' } else { c })
            .collect();
        assert_eq!(shape, "dddd-dd-ddTdd:dd:dd.dddZ");
        assert_eq!(iso(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
    }
}
