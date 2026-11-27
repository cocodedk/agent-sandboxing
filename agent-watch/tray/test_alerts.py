import json
import os
import tempfile
import unittest
from unittest import mock

import alerts

import constants as c

EXPECTED_TESTS = 31
HOME = "/home/u"
ALERT = {"time": "2026-11-27T10:10:34.228Z", "kind": "alert", "agent": "claude",
         "pid": 12, "comm": "cat", "path": HOME + "/.ssh/id_ed25519", "mode": "read"}
LOST = {"time": "2026-11-27T10:10:35.001Z", "kind": "lost", "count": 3}


def dump(*records):
    return "".join(json.dumps(r) + "\n" for r in records)


class FollowerTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.path = os.path.join(tmp.name, "a.jsonl")
        self.follower = alerts.Follower(self.path)

    def put(self, text, mode="w"):
        with open(self.path, mode) as f:
            f.write(text)

    def test_missing_file_is_empty(self):
        self.assertEqual(self.follower.poll(), [])
        self.assertFalse(self.follower.denied)

    def test_unreadable_file_is_denied(self):
        self.put(dump(ALERT))
        os.chmod(self.path, 0)
        self.assertEqual(self.follower.poll(), [])
        self.assertTrue(self.follower.denied)
        os.chmod(self.path, 0o600)
        self.assertEqual(self.follower.poll(), [ALERT])
        self.assertFalse(self.follower.denied)
        os.chmod(self.path, 0)  # a passing error does not read the file twice
        self.follower.poll()
        os.chmod(self.path, 0o600)
        self.assertEqual(self.follower.poll(), [])

    def test_reads_then_only_new(self):
        self.put(dump(ALERT))
        self.assertEqual(self.follower.poll(), [ALERT])
        self.assertEqual(self.follower.poll(), [])
        self.put(dump(LOST), "a")
        self.assertEqual(self.follower.poll(), [LOST])

    def test_partial_line_waits(self):
        text = dump(ALERT)
        self.put(text[:20])
        self.assertEqual(self.follower.poll(), [])
        self.put(text[20:], "a")
        self.assertEqual(self.follower.poll(), [ALERT])

    def test_bad_json_skipped(self):
        self.put("nope\n[1]\n" + '{"x":1}\n' + dump(ALERT))
        self.assertEqual(self.follower.poll(), [ALERT])

    def test_rotation_by_inode(self):
        self.put(dump(ALERT))
        self.follower.poll()
        os.remove(self.path)
        self.put(dump(LOST, LOST))
        self.assertEqual(self.follower.poll(), [LOST, LOST])

    def test_rotation_by_smaller_size(self):
        self.put(dump(ALERT, ALERT))
        self.follower.poll()
        self.put(dump(LOST))
        self.assertEqual(self.follower.poll(), [LOST])

    def test_needles_skip_before_parsing(self):
        follower = alerts.Follower(self.path, (b"keep",))
        self.put('{"time":"a","x":"drop"}\n{"time":"b","x":"keep"}\n')
        with mock.patch.object(alerts.json, "loads", wraps=json.loads) as loads:
            self.assertEqual(follower.poll(), [{"time": "b", "x": "keep"}])
        self.assertEqual(loads.call_count, 1)

    def test_file_removed_then_back(self):
        self.put(dump(ALERT))
        self.follower.poll()
        os.remove(self.path)
        self.assertEqual(self.follower.poll(), [])
        self.put(dump(ALERT))
        self.assertEqual(self.follower.poll(), [ALERT])


class TextTest(unittest.TestCase):
    def test_unseen_filter(self):
        self.assertEqual(alerts.unseen([ALERT, LOST], ALERT["time"]), [LOST])
        self.assertEqual(alerts.unseen([ALERT, LOST], ""), [ALERT, LOST])

    def test_alert_line_home_and_read(self):
        self.assertEqual(alerts.line_text(ALERT, HOME),
                         "cat read ~/.ssh/id_ed25519 (agent claude)")

    def test_write_and_outside_home(self):
        rec = dict(ALERT, mode="write", path="/home/ux/f")
        self.assertEqual(alerts.line_text(rec, HOME), "cat wrote /home/ux/f (agent claude)")

    def test_home_itself(self):
        self.assertIn(" ~ ", alerts.line_text(dict(ALERT, path=HOME), HOME))

    def test_lost_line(self):
        self.assertEqual(alerts.line_text(LOST, HOME),
                         "missed 3 events: the watcher was overloaded")

    def test_icon_priority(self):
        self.assertEqual(alerts.icon_name(False, 5), c.ICON_STOPPED)
        self.assertEqual(alerts.icon_name(True, 1), c.ICON_UNSEEN)
        self.assertEqual(alerts.icon_name(True, 0), c.ICON_OK)

    def test_status_text(self):
        self.assertEqual(alerts.status_text(False, 2), c.STATUS_STOPPED)
        self.assertEqual(alerts.status_text(True, 2), "2 unseen alerts")
        self.assertEqual(alerts.status_text(True, 0), "Watching")
        self.assertEqual(alerts.status_text(True, 0, denied=True), c.STATUS_DENIED)

    def test_local_time(self):
        self.assertRegex(alerts.local_time(ALERT["time"]), r"^\d\d:\d\d$")
        self.assertRegex(alerts.local_time(ALERT["time"], c.TIME_FORMAT_SECONDS),
                         r"^\d\d:\d\d:\d\d$")

    def test_lost_line_without_count(self):
        self.assertEqual(alerts.line_text(dict(LOST, count=None), HOME),
                         "missed some events: the watcher was overloaded")

    def test_suite_size(self):
        suite = unittest.defaultTestLoader.discover(os.path.dirname(__file__))
        self.assertEqual(suite.countTestCases(), EXPECTED_TESTS)


if __name__ == "__main__":
    unittest.main()
