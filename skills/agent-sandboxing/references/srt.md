# srt's quirks, and the checks probe.sh does not make

Found on srt 0.0.83. A newer srt may differ: run `bash <skill-dir>/scripts/probe.sh` after every srt update.

## Quirks

- srt cannot nest: a bwrap inside it fails ("nesting depth ... exceeded"). It is the only box, so a boxed agent cannot
  start a box with a wider network.
- Its settings refuse a bare `*` host and CIDR ranges ("Overly broad patterns"), so `--internet` and `--network` run
  `scripts/agent-box-net.mjs`, which uses srt as a library: `SandboxManager.initialize` with a callback for each host
  its lists do not decide (only a strict `true` allows). It imports srt's files by path from the folder the `srt`
  command resolves to (`index.js`, `utils/config-loader.js`, `utils/shell-quote.js`, `sandbox/parent-proxy.js`):
  after an srt update, run the network check below again. A missing file stops the run; it never falls back.
- srt checks its own-address list (`deniedResolvedAddresses`) only for a name: an address the callback allows skips it,
  and srt lets `localhost` and `*.localhost` through to whoever allows them. The callback refuses those, every address
  of this machine and the networks of its other interfaces, read with `ip -j addr show` (Node's own list skips an
  interface that is down, as Docker's bridges are while no container runs), once, when the run starts.
- srt's `NO_PROXY` skips the private ranges, so with `--network` the box's `NO_PROXY` keeps only its own loopback and
  a request to the local network goes through the proxy.
- srt reads `-c`, `-h`, `-s`, `-d` and `--version` as its own options even after the command. `agent_box` passes
  `--` before the command; keep it, or `<agent> --version` prints srt's version and a `bash -c` never runs.
- Its seccomp step needs a user namespace inside its own, which AppArmor refuses where
  `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` is 1 (Ubuntu 23.10 and later; "write
  /proc/self/setgroups"). There the box sets `allowAllUnixSockets`, which turns that filter off. The box still sees
  no host socket (no `/run`), and abstract sockets stay behind the network.
- Its proxy arrives as a socket in its temporary folder, which `denyRead: ["/"]` hides ("Proxy CONNECT aborted")
  unless `allowRead` names it. Each run gets a short `mktemp -d` folder of its own: a long path fails with "Failed
  to create bridge sockets", and a shared one would show another srt run's proxy, which may allow more hosts.
- srt sets its proxy in the box's environment (`HTTPS_PROXY`, `GIT_SSH_COMMAND`, `JAVA_TOOL_OPTIONS` and more). An
  agent that ignores `HTTPS_PROXY` reaches no host.
- A `denyRead` folder is an empty writable folder in the box, thrown away at the end. srt has no read-only stand-in.
- A package firewall that connects to registries itself, not through the proxy, fails in the box ("EAI_AGAIN"): keep
  its shims off the box's `PATH`.
- With the repository writable, srt guards names in the working directory (`.bashrc`, `.mcp.json`, `.vscode`,
  `.claude/commands`, …) by making an empty file or folder on the host for each one missing; `git status` shows them
  as untracked. It removes them when the command ends, but not when a closed terminal kills it: `agent_box_sweep`
  does. Compare `BOX_PLACEHOLDERS` with `DANGEROUS_FILES` in srt's `dist/sandbox/sandbox-utils.js` after an update.

## Checks probe.sh does not make

From a repository with a git-ignored `.env`, after sourcing the wrapper:

- `curl http://127.0.0.1:<a local port>` is refused (a local service, a browser's debug port).
- `BOX_MODE=write agent_box <agent> --version` prints the agent's version, not srt's.
- With `BOX_MODE=build`, the project's test suite gives the same result as outside, and `npm view left-pad version`
  fails.
- A folder named in `AGENT_BOX_READ` is readable and read-only.
- No command line holds a key during a real run: `grep -a <part of the key> /proc/*/cmdline` finds nothing.
- Through the proxy (`curl -x "$HTTPS_PROXY" --noproxy ""`), with no flag, `--internet`, `--network` and both:
  `https://example.com` only with `--internet`; the router by address (also as one decimal number, in hex, and as
  `[::ffff:<router>]`) only with `--network`, never by a name that leads to it (`<router>.nip.io`); never `localhost`,
  `a.b.localhost`, `localhost.`, loopback in any spelling (`[::1]`, `0x7f000001`, `[::ffff:127.0.0.1]`,
  `[::127.0.0.1]`), this machine's own address on the network, a Docker bridge or container or a name that leads to
  one, or `169.254.169.254`. `BOX_INTERNET=1 BOX_LAN=1` set outside opens nothing; a box started in a box fails. The
  file, environment and argument checks give the same results under every flag. Tested where AppArmor forces
  `allowAllUnixSockets`; the launcher with srt's seccomp filter on is not tested.
- A writable run under `script`, sent SIGHUP to its foreground process group (as a closing terminal does), leaves no
  placeholder. `timeout ... script` is no test: it kills the shell before its trap runs.
