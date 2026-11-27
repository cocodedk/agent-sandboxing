"""GTK, AppIndicator and Notify glue; the logic lives in alerts.py."""
import os
import subprocess

import gi  # type: ignore[import-not-found]

gi.require_version("Gtk", "3.0")
gi.require_version("AyatanaAppIndicator3", "0.1")
gi.require_version("Notify", "0.7")
import alerts
import network
from gi.repository import (  # type: ignore[import-not-found]
    AyatanaAppIndicator3 as AppIndicator,
)
from gi.repository import GLib, Gtk, Notify

import constants as c

HOME = os.path.expanduser("~")


class Tray:
    def __init__(self):
        self.follower = alerts.Follower()
        self.network = network.Network()
        self.seen = c.UNSEEN_START
        self.records = []
        self.running = None
        self.shown = None
        self.indicator = AppIndicator.Indicator.new(
            c.APP_ID, c.ICON_OK, AppIndicator.IndicatorCategory.APPLICATION_STATUS)
        self.indicator.set_status(AppIndicator.IndicatorStatus.ACTIVE)

    def poll(self):
        self.records = (self.records + self.follower.poll())[-c.RECORDS_KEPT:]
        self.network.poll()
        was_running = self.running
        self.running = subprocess.call(c.SYSTEMCTL_CMD) == 0
        if was_running is not None and was_running != self.running:
            self.notify(c.NOTE_STARTED if self.running else c.NOTE_STOPPED, "")
        self.refresh()
        return True

    def notify(self, title, body):
        Notify.Notification.new(title, body, c.ICON_UNSEEN).show()

    def refresh(self):
        count = len(alerts.unseen(self.records, self.seen))
        # an unreadable log is as blind as a stopped service
        denied = self.follower.denied or self.network.follower.denied
        state = (self.running, count, len(self.records), denied, tuple(self.network.items))
        if state == self.shown:
            return
        self.shown = state
        icon = alerts.icon_name(self.running and not denied, count)
        self.indicator.set_icon_full(icon, c.APP_NAME)
        self.indicator.set_menu(self.build_menu(alerts.status_text(self.running, count, denied)))

    def build_menu(self, status):
        menu = Gtk.Menu()
        menu.append(self.item(status))
        menu.append(self.network_item())
        for record in reversed(self.records[-c.MENU_RECORDS:]):
            text = c.MENU_RECORD.format(
                time=alerts.local_time(record["time"]), line=alerts.line_text(record, HOME))
            menu.append(self.item(text))
        menu.append(Gtk.SeparatorMenuItem())
        mark = self.item(c.MENU_MARK_SEEN, sensitive=True)
        mark.connect("activate", self.mark_seen)
        menu.append(mark)
        menu.show_all()
        return menu

    def network_item(self):
        sub = Gtk.Menu()
        for time, line, _key in reversed(self.network.items):
            text = c.MENU_RECORD.format(
                time=alerts.local_time(time, c.TIME_FORMAT_SECONDS), line=line)
            sub.append(self.item(text))
        if not self.network.items:
            sub.append(self.item(c.MENU_NETWORK_EMPTY))
        item = self.item(c.MENU_NETWORK, sensitive=True)
        item.set_submenu(sub)
        return item

    @staticmethod
    def item(label, sensitive=False):
        item = Gtk.MenuItem(label=label)
        item.set_sensitive(sensitive)
        return item

    def mark_seen(self, _item):
        if self.records:
            self.seen = max(r["time"] for r in self.records)
        self.refresh()


def main():
    Notify.init(c.APP_NAME)
    tray = Tray()
    tray.poll()
    GLib.timeout_add_seconds(c.POLL_SECONDS, tray.poll)
    Gtk.main()


if __name__ == "__main__":
    main()
