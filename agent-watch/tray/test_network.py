import json
import os
import tempfile
import unittest

import network

import constants as c

OWN = {"claude": {"160.79.104.10"}}


def dns(host, agent="claude"):
    return {"kind": "dns", "agent": agent, "host": host, "time": "t"}


def conn(addr, port=443, agent="claude"):
    return {"kind": "connect", "agent": agent, "addr": addr, "port": port, "time": "t", "pid": 5}


def tls(host, addr="140.82.121.5", agent="claude"):
    return {"kind": "tls", "agent": agent, "host": host, "addr": addr, "port": 443, "time": "t",
            "pid": 5}


class LineTest(unittest.TestCase):
    def text(self, record):
        return network.line_text(record, OWN)

    def test_lookup_and_connect_lines(self):
        self.assertEqual(self.text(dns("api.github.com")), "claude looked up api.github.com")
        self.assertEqual(self.text(conn("140.82.121.5")), "claude connected to 140.82.121.5:443")

    def test_ipv6_in_brackets(self):
        self.assertEqual(self.text(conn("2607:6bc0::10")),
                         "claude connected to [2607:6bc0::10]:443")

    def test_port_zero_hidden(self):
        self.assertIsNone(self.text(conn("2607:6bc0::10", 0)))

    def test_own_suffix_hidden_with_subdomain(self):
        self.assertIsNone(self.text(dns("anthropic.com")))
        self.assertIsNone(self.text(dns("api.Anthropic.com")))

    def test_lookalike_suffix_shown(self):
        self.assertIsNotNone(self.text(dns("notanthropic.com")))

    def test_own_address_hidden(self):
        self.assertIsNone(self.text(conn("160.79.104.10")))

    def test_other_agents_shown(self):
        self.assertIsNotNone(self.text(dns("api.anthropic.com", "codex")))
        self.assertIsNotNone(self.text(conn("160.79.104.10", agent="codex")))


    def test_tls_line_names_the_site(self):
        self.assertEqual(self.text(tls("github.com")), "claude opened github.com:443")

    def test_own_tls_site_hidden(self):
        self.assertIsNone(self.text(tls("api.anthropic.com")))


class OwnAddressTest(unittest.TestCase):
    def test_resolves_hosts_and_ignores_failures(self):
        def resolve(host, _port):
            if host == c.OWN_ADDRESS_HOSTS["claude"][0]:
                return [(0, 0, 0, "", ("1.2.3.4", 0)), (0, 0, 0, "", ("::1", 0, 0, 0))]
            raise OSError("no such host")
        self.assertEqual(network.own_addresses(resolve), {"claude": {"1.2.3.4", "::1"}})


class NetworkTest(unittest.TestCase):
    def test_follows_main_log_keeps_last_items(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "main.jsonl")
            lines = ['{"time":"t","kind":"open","agent":"claude"}']
            lines += [json.dumps(dns(f"h{i}.example"), separators=(",", ":"))
                      for i in range(c.NET_ITEMS + 2)]
            with open(path, "w") as f:
                f.write("\n".join(lines) + "\n")
            net = network.Network(path, resolve=lambda *_: [])
            net.poll()
        self.assertEqual(len(net.items), c.NET_ITEMS)
        self.assertEqual(net.items[-1][1], "claude looked up h16.example")
        self.assertEqual(net.items[0][1], "claude looked up h2.example")

    def test_site_name_replaces_its_connect(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "main.jsonl")
            records = [conn("140.82.121.5"), conn("1.1.1.1"), tls("github.com")]
            with open(path, "w") as f:
                f.write("".join(json.dumps(r, separators=(",", ":")) + "\n" for r in records))
            net = network.Network(path, resolve=lambda *_: [])
            net.poll()
        lines = [item[1] for item in net.items]
        self.assertEqual(lines, ["claude connected to 1.1.1.1:443", "claude opened github.com:443"])


if __name__ == "__main__":
    unittest.main()
