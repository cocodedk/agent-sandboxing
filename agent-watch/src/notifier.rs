//! Desktop notifications for alerts, sent by the root service: agents running as the user cannot stop it.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::constants::{
    BUS_ADDRESS_PREFIX, BUS_ENV, FIELD_AGENT, FIELD_COMM, FIELD_MODE, FIELD_PATH, MARKUP_ESCAPES,
    MODE_WRITE, NOTIFY_ARGS, NOTIFY_LINES, SESSION_BUS_FILE, UNKNOWN_AGENT,
};
use crate::danger::Account;
use crate::record::Record;

/// The notify-send to run, the folder of per-user runtime folders, the accounts, the pause between notifications.
pub struct Notifier {
    pub command: PathBuf,
    pub run_dir: PathBuf,
    pub accounts: Vec<Account>,
    pub interval: Duration,
}

impl Notifier {
    /// Start the sender thread; sending an alert to the channel never blocks.
    pub fn spawn(self) -> Sender<Record> {
        let (sender, receiver) = channel();
        thread::spawn(move || self.serve(receiver));
        sender
    }

    /// Notify at once, then at most once per interval with what came meanwhile.
    fn serve(&self, alerts: Receiver<Record>) {
        while let Ok(first) = alerts.recv() {
            let batch: Vec<Record> = std::iter::once(first).chain(alerts.try_iter()).collect();
            self.notify(&title(batch.len()), &body(&batch));
            thread::sleep(self.interval);
        }
    }

    /// One notification in the desktop session of every login account that has one.
    fn notify(&self, title: &str, body: &str) {
        for account in &self.accounts {
            let bus = self
                .run_dir
                .join(account.uid.to_string())
                .join(SESSION_BUS_FILE);
            if !bus.exists() {
                continue;
            }
            let status = Command::new(&self.command)
                .args(NOTIFY_ARGS)
                .args([title, body])
                .uid(account.uid)
                .gid(account.gid)
                .env_clear()
                .env(BUS_ENV, format!("{BUS_ADDRESS_PREFIX}{}", bus.display()))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if !status.as_ref().is_ok_and(|status| status.success()) {
                eprintln!(
                    "agent-watch: no notification for uid {}: {status:?}",
                    account.uid
                );
            }
        }
    }
}

fn title(count: usize) -> String {
    match count {
        1 => "agent-watch: 1 new alert".to_string(),
        _ => format!("agent-watch: {count} new alerts"),
    }
}

/// Up to NOTIFY_LINES alerts, then how many more; markup escaped, since comm and path are the agent's.
fn body(batch: &[Record]) -> String {
    let mut lines: Vec<String> = batch.iter().take(NOTIFY_LINES).map(line).collect();
    if batch.len() > NOTIFY_LINES {
        lines.push(format!("and {} more", batch.len() - NOTIFY_LINES));
    }
    let text = lines.join("\n");
    MARKUP_ESCAPES
        .iter()
        .fold(text, |text, (from, to)| text.replace(from, to))
}

fn line(record: &Record) -> String {
    let field = |key: &str| {
        record
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(UNKNOWN_AGENT)
    };
    let verb = if field(FIELD_MODE) == MODE_WRITE {
        "wrote"
    } else {
        "read"
    };
    let (comm, path, agent) = (field(FIELD_COMM), field(FIELD_PATH), field(FIELD_AGENT));
    format!("{comm} {verb} {path} (agent {agent})")
}

#[cfg(test)]
#[path = "notifier_tests.rs"]
mod tests;
