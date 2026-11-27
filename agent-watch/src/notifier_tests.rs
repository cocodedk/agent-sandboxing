//! The notification text, who gets it, and how notify-send is run.

use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use serde_json::json;
use tempfile::TempDir;

use super::{Notifier, body, title};
use crate::danger::Account;
use crate::record::Record;

const WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(10);

fn alert(comm: &str, mode: &str, path: &str) -> Record {
    let value = json!({"kind": "alert", "agent": "pi", "comm": comm, "mode": mode, "path": path});
    value.as_object().unwrap().clone()
}

#[test]
fn title_counts() {
    assert_eq!(title(1), "agent-watch: 1 new alert");
    assert_eq!(title(4), "agent-watch: 4 new alerts");
}

#[test]
fn body_lists_three_then_counts_and_escapes_markup() {
    let mut batch = vec![alert("cat", "read", "/h/.ssh/a<b>&c")];
    batch.extend((0..4).map(|_| alert("cp", "write", "/h/.aws/x")));
    let lines = body(&batch);
    let lines: Vec<&str> = lines.lines().collect();
    assert_eq!(lines[0], "cat read /h/.ssh/a&lt;b&gt;&amp;c (agent pi)");
    assert_eq!(lines[1], "cp wrote /h/.aws/x (agent pi)");
    assert_eq!(lines[3], "and 2 more");
}

/// A notifier whose notify-send is a script that writes its arguments and bus to a file.
fn fake(tmp: &Path, with_bus: bool) -> (Sender<Record>, std::path::PathBuf) {
    let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
    let out = tmp.join("out");
    let command = tmp.join("notify-send");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" \"$DBUS_SESSION_BUS_ADDRESS\" > {}\n",
        out.display()
    );
    fs::write(&command, script).unwrap();
    fs::set_permissions(
        &command,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    let user_dir = tmp.join("run").join(uid.to_string());
    fs::create_dir_all(&user_dir).unwrap();
    if with_bus {
        fs::write(user_dir.join("bus"), "").unwrap();
    }
    let home = String::new();
    let accounts = vec![Account { uid, gid, home }];
    let notifier = Notifier {
        command,
        run_dir: tmp.join("run"),
        accounts,
        interval: Duration::ZERO,
    };
    (notifier.spawn(), out)
}

#[test]
fn notify_send_runs_as_the_user_with_its_bus() {
    let tmp = TempDir::new().unwrap();
    let (sender, out) = fake(tmp.path(), true);
    sender.send(alert("cat", "read", "/h/.ssh/id")).unwrap();
    let start = Instant::now();
    while !out.exists() && start.elapsed() < WAIT {
        std::thread::sleep(POLL);
    }
    std::thread::sleep(POLL * 10); // the script may still be writing
    let text = fs::read_to_string(&out).unwrap();
    let bus = tmp
        .path()
        .join("run")
        .join(unsafe { libc::getuid() }.to_string())
        .join("bus");
    let expected = format!(
        "--app-name=agent-watch\n--urgency=critical\n--\nagent-watch: 1 new alert\ncat read /h/.ssh/id (agent pi)\nunix:path={}\n",
        bus.display()
    );
    assert_eq!(text, expected);
}

#[test]
fn no_session_bus_means_no_notification() {
    let tmp = TempDir::new().unwrap();
    let (sender, out) = fake(tmp.path(), false);
    sender.send(alert("cat", "read", "/h/.ssh/id")).unwrap();
    drop(sender); // the thread ends once the batch is handled
    std::thread::sleep(POLL * 20);
    assert!(!out.exists());
}
