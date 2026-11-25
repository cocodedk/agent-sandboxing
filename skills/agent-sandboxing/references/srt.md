# srt's quirks, and the checks probe.sh does not make

Found on srt 0.0.83. A newer srt may differ: run `bash <skill-dir>/scripts/probe.sh` after every srt update.

## Quirks

- srt cannot nest: a bwrap inside it fails ("nesting depth ... exceeded"). It is the only box.
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
- A writable run under `script`, sent SIGHUP to its foreground process group (as a closing terminal does), leaves no
  placeholder. `timeout ... script` is no test: it kills the shell before its trap runs.
