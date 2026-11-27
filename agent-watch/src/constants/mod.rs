//! Every name, path, limit and mode agent-watch uses, defined once.

mod network;
pub use network::*;

use std::time::Duration;

/// Process names (comm: at most 15 chars) that make a process an agent root.
pub const AGENT_NAMES: &[&str] = &[
    "claude",
    "opencode",
    "pi",
    "prime-agent",
    "codex",
    "gemini",
    "aider",
    "goose",
    "cursor-agent",
    "crush",
    "qwen",
    "pi-box",
    "opencode-box",
    "prime-agent-box",
    "oc-review",
    "oc-agent",
];
/// argv[0] and argv[1]: a node-run script shows its name in argv[1].
pub const ARGV_NAME_SLOTS: usize = 2;
pub const ARGV_SEP: u8 = 0;
pub const LINE_END_BYTE: u8 = LINE_END as u8;
/// The signals that stop the service: systemd's SIGTERM, and SIGINT by hand.
pub const STOP_SIGNALS: [libc::c_int; 2] = [libc::SIGTERM, libc::SIGINT];

// Paths
pub const PROC_DIR: &str = "/proc";
pub const PROC_STAT: &str = "stat";
pub const PROC_CMDLINE: &str = "cmdline";
pub const STAT_COMM_OPEN: char = '(';
pub const STAT_COMM_CLOSE: char = ')';
pub const STAT_PPID_FIELD: usize = 1; // after comm: state, then ppid
pub const BPFTRACE: &str = "/usr/bin/bpftrace";
pub const INSTALL_DIR: &str = "/usr/local/lib/agent-watch";
pub const PROGRAM_FILE_NAME: &str = "agent-watch.bt";
pub const LOG_DIR: &str = "/var/log/agent-watch";
pub const LOG_FILE_NAME: &str = "agent-watch.jsonl";
pub const ROTATED_SUFFIX: &str = ".gz";
pub const ALERT_FILE_NAME: &str = "alerts.jsonl";
pub const PASSWD_FILE: &str = "/etc/passwd";

// Log ownership and size
pub const LOG_GROUP: &str = "adm";
pub const LOG_FILE_MODE: u32 = 0o640;
pub const LOG_DIR_MODE: u32 = 0o750;
pub const ROOT_UID: u32 = 0;
pub const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;
pub const KEEP_ROTATED: u32 = 12;
/// Alerts are rare: a small file that keeps them far longer than the main log, which a burst fills in seconds.
pub const MAX_ALERT_BYTES: u64 = 1024 * 1024;
pub const KEEP_ALERTS: u32 = 4;
pub const DEDUPE_WINDOW: Duration = Duration::from_secs(3600);
/// Dedupe keys kept before the window starts over early: bounds memory.
pub const SEEN_MAX: usize = 100_000;

// bpftrace program sizes and template placeholders
/// PATH_MAX: long paths are not cut.
pub const MAX_STRLEN: u32 = 4096;
/// Above threads-max on a desktop: one key per live thread.
pub const MAX_MAP_KEYS: u32 = 262_144;
pub const PLACEHOLDER_MATCH: &str = "@@AGENT_MATCH@@";
pub const PLACEHOLDER_SEEDS: &str = "@@SEEDS@@";
pub const PLACEHOLDER_STRLEN: &str = "@@MAX_STRLEN@@";
pub const PLACEHOLDER_MAP_KEYS: &str = "@@MAX_MAP_KEYS@@";
pub const PLACEHOLDER_DEBUG_PREFIX: &str = "@@DEBUG_PREFIX@@";
pub const PLACEHOLDER_DEBUG_LEN: &str = "@@DEBUG_PREFIX_LEN@@";
pub const PLACEHOLDER_DEBUG_READ: &str = "@@DEBUG_PREFIX_READ@@";
pub const PLACEHOLDER_DEBUG_ARGS: &str = "@@DEBUG_MAX_ARGS@@";
pub const PLACEHOLDER_CHILD_TYPE: &str = "@@CHILD_TYPE@@";
pub const PLACEHOLDER_CHILD_TYPE_LEN: &str = "@@CHILD_TYPE_LEN@@";
pub const PLACEHOLDER_ITER_UBUF: &str = "@@ITER_UBUF@@";
pub const PLACEHOLDER_ITER_IOVEC: &str = "@@ITER_IOVEC@@";
pub const PLACEHOLDER_TLS_HANDSHAKE: &str = "@@TLS_HANDSHAKE@@";
pub const PLACEHOLDER_HELLO_TYPE_AT: &str = "@@HELLO_TYPE_AT@@";
pub const PLACEHOLDER_CLIENT_HELLO: &str = "@@CLIENT_HELLO@@";
pub const PLACEHOLDER_TLS_READ: &str = "@@TLS_READ@@";

// Debug browsers
/// An argument starting with this (--remote-debugging-port, --remote-debugging-pipe)
/// makes a browser or Electron app an agent root: something can drive it.
pub const DEBUG_FLAG_PREFIX: &str = "--remote-debugging-";
/// Bytes of each argument read to compare: the prefix and the closing zero str() counts.
pub const DEBUG_PREFIX_READ: usize = DEBUG_FLAG_PREFIX.len() + 1;
/// Arguments searched for the flag, at exec and in /proc: bounds the probe's loop.
/// Automation tools pass about 40; past this a flag is missed.
pub const DEBUG_MAX_ARGS: usize = 128;
/// Chromium rewrites its cmdline (setproctitle) as one string of space-separated arguments.
pub const TITLE_SEP: u8 = b' ';
/// A Chromium helper (zygote, renderer, gpu) carries this; it sits under its browser, not as a root.
pub const CHILD_TYPE_PREFIX: &str = "--type=";
pub const BPFTRACE_ARGS: &[&str] = &["-f", "json", "-B", "line"];

// Record kinds and fields
pub const KIND_AGENT: &str = "agent";
pub const KIND_END: &str = "end";
pub const KIND_EXEC: &str = "exec";
pub const KIND_OPEN: &str = "open";
pub const KIND_CONNECT: &str = "connect";
pub const KIND_DNS: &str = "dns";
pub const KIND_TRACER: &str = "tracer";
pub const KIND_ALERT: &str = "alert";
pub const KIND_LOST: &str = "lost";
pub const KIND_TLS: &str = "tls";
/// Every network contact is kept, so the tray can show each one live.
pub const NEVER_DROPPED: &[&str] = &[KIND_AGENT, KIND_END, KIND_DNS, KIND_CONNECT, KIND_TLS];
/// Records that also go to the alerts file.
pub const ALERT_KINDS: &[&str] = &[KIND_ALERT, KIND_LOST];
pub const MODE_READ: &str = "read";
pub const MODE_WRITE: &str = "write";
/// The open record's f_mode & FMODE_WRITE.
pub const WRITE_BIT_CLEAR: &str = "0";
pub const UNKNOWN_AGENT: &str = "?";
pub const FIELD_SEP: char = '\t';
pub const LINE_END: char = '\n';
/// kind, pid, root pid, comm byte length; then comm, a tab, detail (free text).
pub const HEAD_FIELDS: usize = 4;

// Log record keys
pub const FIELD_TIME: &str = "time";
pub const FIELD_KIND: &str = "kind";
pub const FIELD_AGENT: &str = "agent";
pub const FIELD_AGENT_PID: &str = "agent_pid";
pub const FIELD_PID: &str = "pid";
pub const FIELD_COMM: &str = "comm";
pub const FIELD_PATH: &str = "path";
pub const FIELD_MODE: &str = "mode";
pub const FIELD_ADDR: &str = "addr";
pub const FIELD_PORT: &str = "port";
pub const FIELD_HOST: &str = "host";
pub const FIELD_MESSAGE: &str = "message";
pub const FIELD_COUNT: &str = "count";

// bpftrace -f json output types
pub const JSON_PRINTF: &str = "printf";
pub const JSON_LOST: &str = "lost_events";
pub const JSON_ATTACHED: &str = "attached_probes";
pub const JSON_TYPE: &str = "type";
pub const JSON_DATA: &str = "data";
pub const JSON_COUNT: &str = "count";
/// The other lost-events shape bpftrace 0.25 can print: {"type": "lost_events", "data": {"events": N}}.
pub const JSON_EVENTS: &str = "events";

// Dangers
/// A credential folder or file under a home, and the programs (comm) whose job is to read it;
/// any other program opening it raises an alert, and with no owner every reader does.
/// Owners verified 2026-11-27 by running each tool under a test agent and reading its comm.
pub const CREDENTIALS: &[(&str, &[&str])] = &[
    (".ssh", &["ssh", "ssh-keygen"]),
    (".gnupg", &["gpg"]),
    (".config/gh", &["gh"]),
    (".docker", &["docker", "docker-buildx"]),
    (".password-store", &["tree", "gpg"]), // pass lists with tree, decrypts with gpg
    (".aws", &[]),
    (".config/gcloud", &[]),
    (".kube", &[]),
    (".netrc", &[]),
    (".git-credentials", &[]),
];
pub const PATH_SEP: char = '/';
pub const PASSWD_SEP: char = ':';
pub const PASSWD_UID_FIELD: usize = 2;
pub const PASSWD_GID_FIELD: usize = 3;
pub const PASSWD_HOME_FIELD: usize = 5;
/// Homes checked: root's and those of login accounts, uid 1000 and up.
pub const FIRST_USER_UID: u32 = 1000;

// Time format: struct tm counts years from 1900 and months from 0.
pub const TM_YEAR_BASE: i32 = 1900;
pub const TM_MONTH_BASE: i32 = 1;
pub const MILLIS_PER_SECOND: u64 = 1000;

// Process exit codes
pub const EXIT_OK: u8 = 0;
pub const EXIT_TRACER_DIED: u8 = 1;
