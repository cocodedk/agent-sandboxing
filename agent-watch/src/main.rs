//! agent-watch: run bpftrace, turn its output into log records, and stop cleanly on a signal.

mod clock;
mod constants;
mod danger;
mod events;
mod notifier;
mod program;
mod record;
mod sni;
mod writer;

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};
use std::thread;

use constants::{
    ALERT_FILE_NAME, ALERT_KINDS, BPFTRACE, BPFTRACE_ARGS, EXIT_OK, EXIT_TRACER_DIED, FIELD_KIND,
    INSTALL_DIR, KEEP_ALERTS, KEEP_ROTATED, KIND_ALERT, LINE_END_BYTE, LOG_DIR, LOG_FILE_NAME,
    MAX_ALERT_BYTES, MAX_LOG_BYTES, NOTIFY_INTERVAL, NOTIFY_SEND, PASSWD_FILE, PROC_DIR,
    PROGRAM_FILE_NAME, RUN_USER_DIR, STOP_SIGNALS,
};
use danger::{Credentials, accounts, homes};
use events::Events;
use notifier::Notifier;
use record::{Record, tracer};
use serde_json::Value;
use writer::{Writer, euid, log_owner};

/// The tracer's pid while it runs, for the signal handler; 0 when there is none.
static CHILD: AtomicI32 = AtomicI32::new(0);
/// Set when a signal asked the service to stop.
static STOPPING: AtomicBool = AtomicBool::new(false);

/// Remember the stop and pass SIGINT to bpftrace, which detaches its probes on SIGINT.
extern "C" fn stop(_signum: libc::c_int) {
    STOPPING.store(true, Ordering::SeqCst);
    forward_stop();
}

fn forward_stop() {
    let pid = CHILD.load(Ordering::SeqCst);
    if pid > 0 && STOPPING.load(Ordering::SeqCst) {
        // SAFETY: kill is async-signal-safe; pid > 0 never names a process group.
        unsafe { libc::kill(pid, libc::SIGINT) };
    }
}

/// The main log, the alerts file that keeps alerts far longer, and the desktop notifier.
struct Logs {
    all: Mutex<Writer>,
    alerts: Mutex<Writer>,
    notify: Sender<Record>,
}

fn lock(writer: &Mutex<Writer>) -> MutexGuard<'_, Writer> {
    writer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One tracer record: a note about bpftrace itself.
fn note(logs: &Logs, message: &str) -> io::Result<()> {
    log(logs, &[tracer(message)])
}

/// Every record goes to the main log; alerts and lost events go to the alerts file too.
fn log(logs: &Logs, records: &[Record]) -> io::Result<()> {
    let mut all = lock(&logs.all); // always locked before alerts: no deadlock
    for record in records {
        all.write(record)?;
        let kind = record.get(FIELD_KIND).and_then(Value::as_str);
        if kind.is_some_and(|kind| ALERT_KINDS.contains(&kind)) {
            lock(&logs.alerts).write(record)?;
        }
        if kind == Some(KIND_ALERT) {
            let _ = logs.notify.send(record.clone()); // fails only if the notifier thread died
        }
    }
    Ok(())
}

/// Each line, bytes that are not UTF-8 replaced; a line keeps its newline.
fn each_line(stream: impl Read, mut each: impl FnMut(&str) -> io::Result<()>) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    while reader.read_until(LINE_END_BYTE, &mut line)? > 0 {
        each(&String::from_utf8_lossy(&line))?;
        line.clear();
    }
    Ok(())
}

/// Each stderr line goes to the journal and into the log as a tracer record.
fn pass_stderr(stream: impl Read, logs: &Logs) -> io::Result<()> {
    each_line(stream, |line| {
        io::stderr().write_all(line.as_bytes())?;
        note(logs, &format!("bpftrace stderr: {}", line.trim_end()))
    })
}

/// Run the tracer until it ends; EXIT_OK only when a signal asked it to stop.
fn run(command: &[&str], logs: &Logs, events: &mut Events) -> io::Result<u8> {
    let mut start = vec![tracer(&format!("starting: {}", command.join(" ")))];
    start.extend(events.seed_records());
    log(logs, &start)?;
    for signum in STOP_SIGNALS {
        // SAFETY: the handler only stores atomics and calls kill, both async-signal-safe.
        unsafe { libc::signal(signum, stop as *const () as libc::sighandler_t) };
    }
    let mut child = Command::new(command[0])
        .args(&command[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    CHILD.store(child.id() as i32, Ordering::SeqCst);
    forward_stop(); // a signal that came before the pid was known
    let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
    thread::scope(|scope| {
        let errors = scope.spawn(|| stderr.map_or(Ok(()), |stream| pass_stderr(stream, logs)));
        let read = stdout.map_or(Ok(()), |stream| {
            each_line(stream, |line| log(logs, &events.handle(line)))
        });
        errors
            .join()
            .unwrap_or_else(|_| Err(io::Error::other("stderr reader panicked")))?;
        read
    })?;
    let status = child.wait()?;
    CHILD.store(0, Ordering::SeqCst);
    let code = status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or_default());
    note(logs, &format!("bpftrace stopped, exit code {code}"))?;
    Ok(if STOPPING.load(Ordering::SeqCst) {
        EXIT_OK
    } else {
        EXIT_TRACER_DIED
    })
}

/// The service: seed from /proc, fill the program, trace into the log.
fn serve(
    proc_dir: &Path,
    program_file: &Path,
    log_file: &Path,
    tracer_command: &[&str],
) -> io::Result<u8> {
    let (names, seeds) = program::scan(proc_dir)?;
    let text = program::fill(&fs::read_to_string(program_file)?, &seeds);
    let owner = log_owner(euid())?;
    let alert_file = log_file.with_file_name(ALERT_FILE_NAME);
    let accounts = accounts(&fs::read_to_string(PASSWD_FILE)?);
    let credentials = Credentials::new(&homes(&accounts));
    let notifier = Notifier {
        command: NOTIFY_SEND.into(),
        run_dir: RUN_USER_DIR.into(),
        accounts,
        interval: NOTIFY_INTERVAL,
    };
    let logs = Logs {
        all: Mutex::new(Writer::new(log_file, MAX_LOG_BYTES, KEEP_ROTATED, owner)?),
        alerts: Mutex::new(Writer::new(
            &alert_file,
            MAX_ALERT_BYTES,
            KEEP_ALERTS,
            owner,
        )?),
        notify: notifier.spawn(),
    };
    let filled = program::write_filled(&text)?;
    let path = filled.path().to_string_lossy();
    let command = [tracer_command, &[path.as_ref()]].concat();
    run(&command, &logs, &mut Events::new(names, credentials))
}

fn main() -> io::Result<ExitCode> {
    let program_file = Path::new(INSTALL_DIR).join(PROGRAM_FILE_NAME);
    let log_file = Path::new(LOG_DIR).join(LOG_FILE_NAME);
    let command: Vec<&str> = [BPFTRACE].iter().chain(BPFTRACE_ARGS).copied().collect();
    let code = serve(Path::new(PROC_DIR), &program_file, &log_file, &command)?;
    Ok(ExitCode::from(code))
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "files_tests.rs"]
mod files_tests;
