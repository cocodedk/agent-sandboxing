# What the box does not protect

The box keeps an agent from the rest of the home, the network and the environment. It does not make the agent's
work safe. Say these to the user when wiring a box in.

- **The project folder.** In write and build mode the agent can change or delete any tracked file. Git can bring
  them back, but only what was committed; commit or stash before a writable run. `.git` itself stays read-only.
- **Code run later.** Whatever the agent writes runs outside the box afterwards: a test, a build script, a git hook
  path in a config file, a `package.json` script. Read the diff before running anything it touched.
- **Git-ignored files.** No diff shows them. In build mode `node_modules`, `.venv` and `venv` are writable, and in
  every writable mode the agent can make new ignored files (a fresh `.venv/bin/pip`, a `dist/`). `BOX_CHANGED`
  lists both; read those files before the next run outside. A new nested git repository is the gap: the ignored
  files inside it are not listed.
- **Allowed hosts.** Anything the agent reads can go to the hosts in `BOX_DOMAINS`: the repository's code, and every
  key kept in `BOX_KEEP`, which a model could send to another allowed provider. An attacker needs no server of their
  own: injected text can have the agent send data to an allowed host under the attacker's own key, where it lands in
  the attacker's account; a domain list cannot tell the two keys apart. Keep both lists short.
- **Git history.** `.git` is readable, so git works. Any secret ever committed, even if since deleted and git-ignored,
  and any token in a remote URL in `.git/config`, can be read in the box and sent to an allowed host. Tell the user to
  remove it from the history (for example `git filter-repo`) and rotate the key.
- **The kernel.** bubblewrap uses Linux namespaces. A kernel bug can break out of them, and where AppArmor forces
  `allowAllUnixSockets`, srt's seccomp filter is off. Keep the kernel patched; it is not a VM.
- **The agent's own folders.** What is writable in `BOX_WRITE` persists unless it is a copy thrown away; a
  read-only program folder is what stops a planted program.
- **A killed run.** A SIGKILL skips the EXIT trap: a config or database copy (mode 700) stays in the temporary
  folder, and srt's empty placeholders stay in the repository until the next `agent_box_sweep`.
- **Services outside.** A background service or daemon the agent starts outside the box is not boxed at all.
