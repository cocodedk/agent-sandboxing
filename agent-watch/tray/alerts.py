"""Logic of the tray: no gi import."""
import json
import os
from datetime import datetime

import constants as c


class Follower:
    """Yields new records of a JSONL file across rotation and partial lines.

    With needles, a line holding none of them is skipped before it is parsed."""

    def __init__(self, path=c.ALERTS_PATH, needles=()):
        self.path = path
        self.needles = needles
        self.denied = False  # the file exists but this user may not read it
        self._reset(None)

    def _reset(self, inode):
        self.inode = inode
        self.offset = 0
        self.partial = b""

    def poll(self):
        try:
            stat = os.stat(self.path)
            if stat.st_ino != self.inode or stat.st_size < self.offset:
                self._reset(stat.st_ino)
            with open(self.path, "rb") as f:
                f.seek(self.offset)
                data = f.read()
        except OSError as error:
            # A missing file was rotated away: start the next one from 0. Any other error keeps the place,
            # so a passing error does not read the same file twice.
            self.denied = isinstance(error, PermissionError)
            if isinstance(error, FileNotFoundError):
                self._reset(None)
            return []
        self.denied = False
        self.offset += len(data)
        *lines, self.partial = (self.partial + data).split(b"\n")
        if self.needles:
            lines = [x for x in lines if any(n in x for n in self.needles)]
        return [r for r in map(_parse, lines) if r]


def _parse(line):
    try:
        record = json.loads(line)
    except ValueError:
        return None
    if isinstance(record, dict) and isinstance(record.get("time"), str):
        return record
    return None


def unseen(records, seen):
    return [r for r in records if r["time"] > seen]


def line_text(record, home):
    if record.get("kind") == c.KIND_LOST:
        count = record.get("count")
        return c.LINE_LOST_UNKNOWN if count is None else c.LINE_LOST.format(count=count)
    path = str(record.get("path"))
    if path == home or path.startswith(home + "/"):
        path = c.HOME_SHORT + path[len(home):]
    verb = c.VERB_WROTE if record.get("mode") == c.MODE_WRITE else c.VERB_READ
    return c.LINE_ALERT.format(
        comm=record.get("comm"), verb=verb, path=path, agent=record.get("agent"))


def icon_name(running, unseen_count):
    if not running:
        return c.ICON_STOPPED
    return c.ICON_UNSEEN if unseen_count else c.ICON_OK


def status_text(running, unseen_count, denied=False):
    if denied:
        return c.STATUS_DENIED
    if not running:
        return c.STATUS_STOPPED
    return c.STATUS_UNSEEN.format(n=unseen_count) if unseen_count else c.STATUS_OK


def local_time(iso_time, fmt=c.TIME_FORMAT):
    return datetime.fromisoformat(iso_time).astimezone().strftime(fmt)

