# agent-watch

A root systemd service that keeps a log of what AI agents do on this computer. For every agent process and every process it starts, it logs the files they open, the hosts they connect to, the host names they look up and the programs they run. It covers agents in and out of the sandbox, Claude Code included. It uses bpftrace and runs on Linux.

## What is logged

One JSON object per line in `/var/log/agent-watch/agent-watch.jsonl`. Every line has `time` (UTC), `kind`, `agent` (the agent's name), `agent_pid`, `pid` and `comm` (the name of the process that did it). Then, by kind:

- `agent`: an agent started. `end`: it ended.
- `exec`: a program was run; `path`.
- `open`: a file was opened; `path` and `mode` (`read` or `write`).
- `connect`: a connection to a host; `addr` and `port`.
- `dns`: a host name was looked up; `host`.
- `tls`: an encrypted (TLS) connection was opened to a site; `host` (the site name the program sends in clear at the start), `addr` and `port`. The rest of the address (the path of a URL) is encrypted and never seen.
- `alert`: a program opened a credential file that is not its own (see Alerts); `path` and `mode`.
- `lost`: the kernel dropped events, so the log has a gap (`time`, `kind`, `count`; bpftrace sometimes prints the count in hex, which is read as hex).
- `tracer`: a note from agent-watch itself (`time`, `kind`, `message`): started, stopped or a warning.

An agent is a process whose name is on the list in `src/constants/mod.rs` (`claude`, `opencode`, `pi`, `codex`, the -box wrappers and others). Everything it starts counts as that agent too. The same file or program is logged once an hour per agent, so an agent that runs for days still shows when it read something. Lookups, connections and `tls` records are all kept, so each visit shows.

A browser started with the debug flag (`--remote-debugging-port` or `--remote-debugging-pipe`: Chrome, Chromium, Edge, Brave or an Electron app) is logged as an agent too, because a program can drive it. That includes your own browsing in it and every file it opens, which can fill the log faster. A browser started without the flag is not logged.

## Alerts

An `alert` is a credential folder or file in a home (`~/.ssh`, `~/.gnupg`, `~/.config/gh`, `~/.docker`, `~/.password-store`, `~/.aws`, `~/.config/gcloud`, `~/.kube`, `~/.netrc`, `~/.git-credentials`) opened by an agent's program that is not that folder's own tool: `cat` reading `~/.ssh/id_ed25519` is an alert, `ssh` reading it is not. A folder with no listed tool alerts on every reader. The list is `CREDENTIALS` in `src/constants/mod.rs`. Homes are root's and those of accounts with uid 1000 and up. One alert per program, file and agent an hour.

Alerts and `lost` records also go to `/var/log/agent-watch/alerts.jsonl`, which keeps them far longer than the main log: a burst of file opens (a large search) can fill the main log's 13 files in seconds.

The root service shows each alert as a desktop notification itself: at once, then at most one every 30 seconds with a count. It runs `notify-send` as each logged-in user with that user's session bus, so an agent running as you cannot stop it by stopping a program of yours. It can still switch your desktop's notifications off; only an agent in the box or under another account is fenced off from that. `lost` records are not notified: they show in the tray.

The check goes by the program's name only: a copy of any program named `ssh` passes, and so does a tool asked to print its own secret (`gh auth token`). It catches careless reads, not an agent that hides.

## The tray

The tray icon (`tray/`, Python 3.11 or later with the system's GTK, AppIndicator and Notify) is a viewer; it reads the alerts file and the main log every 2 seconds. A shield means watching, a warning sign means unseen alerts, an open shield means agent-watch is not running or you may not read its files (join the `adm` group). Its menu has:

- **Network:** live, the last 15 host names agents looked up, addresses they connected to and sites they opened (a `tls` record replaces the bare address of its connection). An agent's own service is left out (for `claude`: `anthropic.com`, `claude.ai`, `claude.com`, `datadoghq.com` and their addresses; the lists are `OWN_SUFFIXES` and `OWN_ADDRESS_HOSTS` in `tray/constants.py`), and so are connects to port 0, which only probe a route. A web address (URL) is not visible: connections are encrypted, so the host name is all there is.
- **The last 10 alerts and lost-event records,** and "Mark all seen". Seen is kept in memory only, never in a file an agent could write: after a restart every alert in the file shows as unseen again.

It notifies only when agent-watch stops or starts again. The tray runs as you, so an agent can stop it; systemd restarts it after a plain kill, and the root notifications and the alerts file do not depend on it.

## What is not logged

- Agents whose name is not on the list.
- An agent started by naming its interpreter (`node /usr/bin/pi`) after agent-watch starts: the probe matches the program's name, so it is seen only at the next start or restart. One started by its own name (`pi`, through its `#!` line) is seen.
- Work an agent hands to a process that was already running: a daemon, `systemd-run`, a browser.
- Host names looked up by static or Go programs, such as codex's own binary: only the address they connect to.
- UDP sent without a connect.
- What is read, written or sent: the log shows that a file was opened or a host reached, not the contents. No arguments or environment either.
- Processes started or run in the moment between agent-watch reading the running processes and its probes taking hold, at each start or restart.
- An event from a process whose name is not valid UTF-8 (a name cut mid-character, or one the process set itself): it becomes a `tracer` record holding the line, each byte that is not UTF-8 shown as `�`. A name that really holds `�` is treated the same way.
- An event whose bpftrace output holds a lone UTF-16 surrogate escape (`\ud800` and the like): it becomes a `tracer` record holding the raw line.

## Install

You need bpftrace and Rust (cargo). From this folder, build as your own user, never as root, then install as root:

```
cargo build --release
sudo ./install.sh
```

The install copies the built `target/release/agent-watch`, `agent-watch.bt` and the tray to `/usr/local/lib/agent-watch`, owned by root, installs `agent-watch.service` and starts it, and installs `agent-watch-tray.service` as a user service for every desktop user, started now for the user who ran sudo. The service runs that copy, never this folder, because agents can write here. After a change, build and run the install again; it replaces the old copy and restarts the service.

## What runs

`agent-watch` (one Rust program) reads the running processes, fills in the bpftrace program, writes it to a private temporary file and runs `bpftrace -f json` on it. It turns each bpftrace line into a log record. When systemd stops the service, only `agent-watch` gets the signal (`KillMode=mixed`); it passes SIGINT to bpftrace, which detaches its probes, and writes a last `tracer` record. If bpftrace dies on its own, `agent-watch` logs that and exits with an error, and systemd restarts it after 5 seconds.

## Reading the log

The log folder belongs to the `adm` group. The files are not in time order, so sort by `time` when it matters:

```
zcat -f /var/log/agent-watch/agent-watch.jsonl* | jq -c .
zcat -f /var/log/agent-watch/agent-watch.jsonl* | jq -s -c 'sort_by(.time)[]'
zcat -f /var/log/agent-watch/agent-watch.jsonl* | jq -r 'select(.kind == "dns") | .host' | sort -u
zcat -f /var/log/agent-watch/agent-watch.jsonl* | jq -c 'select(.kind == "open" and .mode == "write" and .agent == "pi")'
zcat -f /var/log/agent-watch/agent-watch.jsonl* | jq -c 'select(.kind == "tracer")'
```

The service's own messages are in the journal: `journalctl -u agent-watch`.

## Rotation and disk use

When the log reaches 8 MiB, it is gzipped to `agent-watch.jsonl.1.gz`, older copies move up one number, and copies past 12 are deleted. The log never takes more than about 8 MiB × 13 = 104 MiB, and usually much less, since the copies are compressed. The limits are `MAX_LOG_BYTES` and `KEEP_ROTATED` in `src/constants/mod.rs`. The alerts file rotates the same way at 1 MiB and keeps 4 copies (`MAX_ALERT_BYTES`, `KEEP_ALERTS`).

## Changing the list of agents

Edit `AGENT_NAMES` in `src/constants/mod.rs`, then run `cargo build --release` and `sudo ./install.sh` again. A name is what the process is called when it starts, at most 15 characters: the program's file name, or the script's name for a script.

## Uninstall

```
systemctl --user stop agent-watch-tray
sudo systemctl --global disable agent-watch-tray
sudo rm /etc/systemd/user/agent-watch-tray.service
sudo systemctl disable --now agent-watch
sudo rm /etc/systemd/system/agent-watch.service
sudo rm -r /usr/local/lib/agent-watch
sudo systemctl daemon-reload
```

The logs in `/var/log/agent-watch` stay until you remove them.

## Tests

```
cargo test
python3 -m unittest discover -s tray
```

They need neither root nor bpftrace; the bpftrace program itself is checked by its first root run (`journalctl -u agent-watch` shows any error).
