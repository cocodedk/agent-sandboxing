#!/usr/bin/env node
// agent-box-net.mjs: srt with a wider network than its settings can name, for a box started with --internet or
// --network (agent-box.sh). srt's settings take no "any host"; its library asks a callback about each host no list
// decides, and this one answers. agent_box runs it outside the box in place of the srt command; a run inside a box
// gets no more than that box's own network, as everything it starts is inside it.
//   node agent-box-net.mjs <srt's dist folder> <settings.json> internet|network|both -- <command...>
// internet: any host but a private or local-network address, and no name that leads to one.
// network: the networks this machine's network cards are on (Wi-Fi, Ethernet), by address only, even while a VPN
// holds the default route; not Docker's networks, a VPN's or other private ranges this machine has. A card attached to
// a bridge (br0, as on a VM host) is not seen, as the bridge holds its address: there network reaches no device.
// both: the two; a name never leads into a private range. Never, whatever the flags: localhost, loopback, link-local,
// multicast, cloud metadata, every address of this machine and every network of its other interfaces (Docker's
// bridges, a VPN): srt checks those for a name, not for an address its callback allows, and lets localhost through to
// whoever allows it.
import { existsSync } from 'node:fs';
import { BlockList, isIP } from 'node:net';
import { constants } from 'node:os';
import { execFileSync, spawn } from 'node:child_process';

const PRIVATE = ['10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16', '100.64.0.0/10', 'fc00::/7'];
const NEVER = ['0.0.0.0/8', '127.0.0.0/8', '169.254.0.0/16', '224.0.0.0/4', '255.255.255.255/32', '168.63.129.16/32',
  '192.0.0.192/32', '::/96', '::ffff:0:0:0/96',   // IPv4-translated; canonicalizeHost turns ::ffff:a.b.c.d into IPv4
  '::1/128', 'fe80::/10', 'ff00::/8', '64:ff9b::/96', '2002::/16'];
const REACHES = ['internet', 'network', 'both'];
const NET_CLASS = '/sys/class/net';   // an interface with a device folder here is a network card, not a bridge or tunnel
const EXIT_USAGE = 2;
const EXIT_FAILED = 1;

const [dist, settings, reach, dashes, ...command] = process.argv.slice(2);
if (!dist || !settings || !REACHES.includes(reach) || dashes !== '--' || command.length === 0) {
  console.error('usage: agent-box-net.mjs <srt dist folder> <settings.json> internet|network|both -- <command...>');
  process.exit(EXIT_USAGE);
}
const fail = (what, err) => { console.error(`agent-box-net: ${what}: ${err?.message ?? err}`); process.exit(EXIT_FAILED); };
const { SandboxManager } = await import(`${dist}/index.js`);
const { loadConfig } = await import(`${dist}/utils/config-loader.js`);
const { quote } = await import(`${dist}/utils/shell-quote.js`);
const { canonicalizeHost } = await import(`${dist}/sandbox/parent-proxy.js`);

// Every interface, up or down (Docker's bridges are down while no container runs, and networkInterfaces() skips them).
// The local network is the network cards' subnets; the others' (Docker's bridges, a VPN) are this machine's.
const ifaces = JSON.parse(execFileSync('ip', ['-j', 'addr', 'show'], { encoding: 'utf8' }));
const isCard = i => existsSync(`${NET_CLASS}/${i.ifname}/device`);
const subnets = i => (i.addr_info ?? []).filter(a => a.scope === 'global').map(a => `${a.local}/${a.prefixlen}`);
const lanCidrs = ifaces.filter(isCard).flatMap(subnets);
const machineCidrs = ifaces.filter(i => !isCard(i)).flatMap(subnets);
const blockOf = cidrs => {
  const list = new BlockList();
  for (const c of cidrs) { const [net, bits] = c.split('/'); list.addSubnet(net, Number(bits), isIP(net) === 6 ? 'ipv6' : 'ipv4'); }
  return list;
};
const own = ifaces.flatMap(i => (i.addr_info ?? []).map(a => `${a.local}/${a.family === 'inet6' ? 128 : 32}`));
const never = blockOf([...NEVER, ...own, ...machineCidrs]);
const lan = blockOf(lanCidrs);
const local = blockOf([...PRIVATE, ...lanCidrs]);
// An address the way getaddrinfo reads it (3232235521, 0xC0A80001, [::ffff:192.168.0.1]), or null for a name.
const address = host => { const c = (canonicalizeHost(host) ?? host).replace(/^\[|\]$/g, ''); return isIP(c) ? c : null; };
const inList = (list, a) => list.check(a, isIP(a) === 6 ? 'ipv6' : 'ipv4');
const allows = ({ host }) => {
  const name = host.toLowerCase().replace(/\.+$/, '');
  if (name === 'localhost' || name.endsWith('.localhost')) return false;
  const a = address(host);
  if (a !== null && inList(never, a)) return false;
  const internet = reach !== 'network' && (a === null || !inList(local, a));
  const network = reach !== 'internet' && a !== null && inList(lan, a);
  return internet || network;
};

const loaded = loadConfig(settings);
if (loaded.kind !== 'ok') fail(`settings ${settings}`, loaded.reason ?? loaded.kind);
const config = loaded.config;
if (reach !== 'network') {   // a name leads to no private range, local network or network of this machine
  config.network.deniedResolvedAddresses = [...(config.network.deniedResolvedAddresses ?? []), ...PRIVATE, ...lanCidrs,
    ...machineCidrs];
}
let child;
try {
  await SandboxManager.initialize(config, async q => allows(q) === true);
  child = spawn(await SandboxManager.wrapWithSandbox(quote(command)), { shell: true, stdio: 'inherit' });
} catch (err) { fail('the box did not start', err); }
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(signal, () => child.kill(signal));
child.on('error', err => fail('the command did not start', err));
child.on('exit', (code, signal) => {
  SandboxManager.cleanupAfterCommand();
  process.exit(signal ? 128 + (constants.signals[signal] ?? 0) : code ?? EXIT_FAILED);
});
