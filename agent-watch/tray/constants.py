"""Every path, interval, icon name, count and text template of the tray."""

ALERTS_PATH = "/var/log/agent-watch/alerts.jsonl"
SERVICE_NAME = "agent-watch"
SYSTEMCTL_CMD = ["systemctl", "is-active", "--quiet", SERVICE_NAME]
MAIN_LOG_PATH = "/var/log/agent-watch/agent-watch.jsonl"
NET_NEEDLES = (b'"kind":"dns"', b'"kind":"connect"', b'"kind":"tls"')  # cheap byte test before json.loads

POLL_SECONDS = 2
MENU_RECORDS = 10
RECORDS_KEPT = 1000  # bounds the tray's memory; the alerts file keeps them all
NET_ITEMS = 15
TIME_FORMAT = "%H:%M"
TIME_FORMAT_SECONDS = "%H:%M:%S"
UNSEEN_START = ""  # sorts before every ISO time: all records start unseen

KIND_ALERT = "alert"
KIND_LOST = "lost"
KIND_DNS = "dns"
KIND_CONNECT = "connect"
KIND_TLS = "tls"
PORT_PROBE = 0  # a connect with this port is a routing probe, not a connection
OWN_SUFFIXES = {"claude": ("anthropic.com", "claude.ai", "claude.com", "datadoghq.com")}
OWN_ADDRESS_HOSTS = {"claude": ("api.anthropic.com", "downloads.claude.ai",
                                "http-intake.logs.us5.datadoghq.com")}
MODE_WRITE = "write"
HOME_SHORT = "~"

ICON_STOPPED = "security-low-symbolic"
ICON_UNSEEN = "dialog-warning-symbolic"
ICON_OK = "security-high-symbolic"

APP_ID = "agent-watch-tray"
APP_NAME = "agent-watch"

LINE_ALERT = "{comm} {verb} {path} (agent {agent})"
LINE_LOST = "missed {count} events: the watcher was overloaded"
LINE_LOST_UNKNOWN = "missed some events: the watcher was overloaded"
LINE_LOOKUP = "{agent} looked up {host}"
LINE_CONNECT = "{agent} connected to {addr}:{port}"
LINE_TLS = "{agent} opened {host}:{port}"
ADDR_V6 = "[{addr}]"
VERB_READ = "read"
VERB_WROTE = "wrote"
NOTE_STOPPED = "agent-watch stopped: agents are not being watched"
NOTE_STARTED = "agent-watch is watching again"
STATUS_OK = "Watching"
STATUS_UNSEEN = "{n} unseen alerts"
STATUS_STOPPED = "agent-watch is not running"
STATUS_DENIED = "Cannot read the alerts file: add yourself to the adm group"
MENU_MARK_SEEN = "Mark all seen"
MENU_RECORD = "{time} {line}"
MENU_NETWORK = "Network"
MENU_NETWORK_EMPTY = "No network contacts yet"
