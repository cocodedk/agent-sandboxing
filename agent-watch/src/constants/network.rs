//! Constants for the network side: TLS site names, lost-event parsing and desktop notifications.

use std::time::Duration;

use super::MAX_STRLEN;

// TLS site names (SNI): an agent's ClientHello names the site in clear
/// enum iter_type in the kernel's BTF (7.0): how tcp_sendmsg's msghdr holds the user's bytes.
pub const ITER_UBUF: u8 = 0;
pub const ITER_IOVEC: u8 = 1;
/// Record type 22 (handshake), and at byte 5 handshake type 1 (ClientHello).
pub const TLS_HANDSHAKE: u8 = 0x16;
pub const HELLO_TYPE_AT: usize = 5;
pub const CLIENT_HELLO: u8 = 1;
/// Bytes of a ClientHello passed on: curl's is about 1.6 KB, Chrome's about 2 KB; bpftrace allows up to MAX_STRLEN.
pub const TLS_READ: u32 = MAX_STRLEN;
/// After the 4-byte handshake header: the 2-byte version and the 32-byte random.
pub const HELLO_FIXED: usize = 2 + 32;
pub const RECORD_HEADER: usize = 5;
pub const HANDSHAKE_LENGTH: usize = 3;
pub const EXT_SERVER_NAME: u16 = 0;
pub const NAME_TYPE_HOST: u8 = 0;
pub const NAME_LIST_LENGTH: usize = 2;
pub const HOST_MAX: usize = 253;
pub const HEX_SEP: char = ' ';

// Desktop notifications, sent by root: agents running as the user cannot stop them
pub const NOTIFY_SEND: &str = "/usr/bin/notify-send";
/// Critical: GNOME keeps the banner until it is dismissed. "--" ends the options: comm and paths are the agent's.
pub const NOTIFY_ARGS: &[&str] = &["--app-name=agent-watch", "--urgency=critical", "--"];
pub const RUN_USER_DIR: &str = "/run/user";
pub const SESSION_BUS_FILE: &str = "bus";
pub const BUS_ENV: &str = "DBUS_SESSION_BUS_ADDRESS";
pub const BUS_ADDRESS_PREFIX: &str = "unix:path=";
/// The first alert is shown at once; later ones at most once per interval, with a count.
pub const NOTIFY_INTERVAL: Duration = Duration::from_secs(30);
pub const NOTIFY_LINES: usize = 3;
/// GNOME reads a notification body as markup.
pub const MARKUP_ESCAPES: &[(&str, &str)] = &[("&", "&amp;"), ("<", "&lt;"), (">", "&gt;")];

// bpftrace 0.25 can print a lost-events count in hex without quotes: {"type": "lost_events", "count": 4c, ...}
pub const LOST_MARK: &str = "\"lost_events\"";
pub const LOST_COUNT_KEY: &str = "\"count\": ";
pub const LOST_COUNT_END: [char; 2] = [',', '}'];
pub const HEX_RADIX: u32 = 16;
