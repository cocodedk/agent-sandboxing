#!/usr/bin/env bash
# Installs or updates agent-watch as a root systemd service. Safe to run again.
# Build first, as your own user, never as root: cargo build --release
set -euo pipefail

INSTALL_DIR=/usr/local/lib/agent-watch
UNIT_DIR=/etc/systemd/system
UNIT=agent-watch.service
TRAY_UNIT=agent-watch-tray.service
USER_UNIT_DIR=/etc/systemd/user
LOG_DIR=/var/log/agent-watch
BPFTRACE=/usr/bin/bpftrace
SOURCE=$(cd "$(dirname "$0")" && pwd)
BUILT="$SOURCE/target/release/agent-watch"
PREVIOUS=.previous
# Long enough for bpftrace to compile and attach, or to fail and be restarted once.
SETTLE_SECONDS=20

if [ "$(id -u)" -ne 0 ]; then
  echo "agent-watch: run as root, for example: sudo $0" >&2
  exit 1
fi
if [ ! -x "$BPFTRACE" ]; then
  echo "agent-watch: $BPFTRACE is missing; install it first" >&2
  exit 1
fi
if [ ! -x "$BUILT" ]; then
  echo "agent-watch: $BUILT is missing; build it first as your user: cargo build --release" >&2
  exit 1
fi

# install replaces each file by a new one, so the running copy is untouched until the restart below.
install -d -o root -g root -m 0755 "$INSTALL_DIR"
# Keep the running version: if bpftrace refuses the new program, it is put back.
for file in agent-watch agent-watch.bt; do
  if [ -e "$INSTALL_DIR/$file" ]; then cp -p "$INSTALL_DIR/$file" "$INSTALL_DIR/$file$PREVIOUS"; fi
done
install -o root -g root -m 0755 "$BUILT" "$INSTALL_DIR/agent-watch"
install -o root -g root -m 0644 "$SOURCE/agent-watch.bt" "$INSTALL_DIR/agent-watch.bt"

install -o root -g root -m 0644 "$SOURCE/$UNIT" "$UNIT_DIR/$UNIT"
systemctl daemon-reload
systemctl enable "$UNIT"
# A unit that hit its start limit refuses a restart until its failed state is cleared.
systemctl reset-failed "$UNIT" 2> /dev/null || true
# restart starts it when stopped and replaces the running copy when running.
systemctl restart "$UNIT"
sleep "$SETTLE_SECONDS"
# An explicit restart sets NRestarts to 0: any count above it means the new version failed to start.
if ! systemctl is-active --quiet "$UNIT" || [ "$(systemctl show -p NRestarts --value "$UNIT")" != 0 ]; then
  echo "agent-watch: the new version did not start; its last messages:" >&2
  journalctl -u "$UNIT" -n 20 --no-pager >&2
  if [ -e "$INSTALL_DIR/agent-watch$PREVIOUS" ]; then
    for file in agent-watch agent-watch.bt; do mv "$INSTALL_DIR/$file$PREVIOUS" "$INSTALL_DIR/$file"; done
    systemctl reset-failed "$UNIT" 2> /dev/null || true
    systemctl restart "$UNIT"
    echo "agent-watch: the previous version is running again" >&2
  fi
  exit 1
fi

# The tray, root-owned like the service, for every desktop user at login.
install -d -o root -g root -m 0755 "$INSTALL_DIR/tray"
install -o root -g root -m 0644 "$SOURCE"/tray/{constants,alerts,network,agent_watch_tray}.py "$INSTALL_DIR/tray/"
install -o root -g root -m 0644 "$SOURCE/$TRAY_UNIT" "$USER_UNIT_DIR/$TRAY_UNIT"
systemctl --global enable "$TRAY_UNIT"
# Start it now for the user who ran sudo; other users get it at their next login.
if [ -n "${SUDO_USER:-}" ]; then
  systemctl --user -M "$SUDO_USER@" daemon-reload && systemctl --user -M "$SUDO_USER@" restart "$TRAY_UNIT" ||
    echo "agent-watch: the tray starts at your next desktop login" >&2
fi

echo "agent-watch is running. The log is $LOG_DIR/agent-watch.jsonl (older parts gzipped beside it)."
echo "Read it with: zcat -f $LOG_DIR/agent-watch.jsonl* | jq ."
echo "Alerts are kept longer in $LOG_DIR/alerts.jsonl; the tray icon shows them."
