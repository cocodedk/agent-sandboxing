"""The live network view: DNS lookups and connections from the main log. No gi import."""
import socket
from collections import deque

import alerts

import constants as c


def own_addresses(resolve=socket.getaddrinfo):
    """Addresses of each agent's own hosts, found once; failures are ignored."""
    found = {}
    for agent, hosts in c.OWN_ADDRESS_HOSTS.items():
        found[agent] = set()
        for host in hosts:
            try:
                found[agent] |= {info[4][0] for info in resolve(host, None)}
            except OSError:
                pass
    return found


def is_own_host(agent, host):
    return any(host == s or host.endswith("." + s) for s in c.OWN_SUFFIXES.get(agent, ()))


def line_text(record, own):
    """The menu line of a main-log record, or None when it is hidden."""
    agent = record.get("agent")
    if record.get("kind") == c.KIND_TLS:
        host = str(record.get("host"))
        if is_own_host(agent, host):
            return None
        return c.LINE_TLS.format(agent=agent, host=host, port=record.get("port"))
    if record.get("kind") == c.KIND_DNS:
        host = str(record.get("host")).lower()
        if is_own_host(agent, host):
            return None
        return c.LINE_LOOKUP.format(agent=agent, host=host)
    if record.get("kind") != c.KIND_CONNECT:
        return None
    port = record.get("port")
    addr = str(record.get("addr"))
    if port == c.PORT_PROBE or addr in own.get(agent, ()):
        return None
    if ":" in addr:
        addr = c.ADDR_V6.format(addr=addr)
    return c.LINE_CONNECT.format(agent=agent, addr=addr, port=port)


class Network:
    def __init__(self, path=c.MAIN_LOG_PATH, resolve=socket.getaddrinfo):
        self.follower = alerts.Follower(path, c.NET_NEEDLES)
        self.own = own_addresses(resolve)
        self.items = deque(maxlen=c.NET_ITEMS)  # (time, line, (pid, addr, port)), oldest first

    def poll(self):
        for record in self.follower.poll():
            line = line_text(record, self.own)
            if not line:
                continue
            key = (record.get("pid"), record.get("addr"), record.get("port"))
            if record.get("kind") == c.KIND_TLS:
                # the site name replaces the bare address of the connect just before it
                self.items = deque((i for i in self.items if i[2] != key), maxlen=c.NET_ITEMS)
            self.items.append((record["time"], line, key))
