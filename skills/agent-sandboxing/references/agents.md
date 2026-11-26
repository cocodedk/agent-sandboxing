# The agents already boxed

Each wrapper's header says what is in its box. These are the traps that shaped them.

## opencode (`scripts/opencode-box`, write mode)

- Always `opencode --standalone`. Without it `opencode run` hands the work to its background service
  (`opencode serve --service`), which runs outside the box: a boxed run's shell listed the whole home.
- The login lives in opencode's database, not in `auth.json`, and that one file also holds every project's sessions.
  `oc_login_db` copies it with every table emptied but the login's and opencode's bookkeeping (`OC_DB_KEEP`), and
  `OPENCODE_DB` (an absolute path, opencode 2.4.1 or later) points at the copy, kept by name in `BOX_KEEP`.
- `opencode session list` and `session export` also need `--standalone`, or they read the real database through the
  service, not the copy.
- Hide `~/.config/opencode/service.json`: the service's password is a way out.
- Of its store (`~/.local/share/opencode`) only `bin/` is in the box, read-only. Its snapshots (git repositories it
  later runs git on outside) and logs go into an empty folder thrown away at the end.
- It follows srt's `HTTPS_PROXY` and needs `opencode.ai` and `*.opencode.ai` (the model list is
  `models.opencode.ai`). It runs on its own login: pass it no provider key.
- `--persist` keeps one repository's sessions across runs in a copy of its own, its login renewed each time. It,
  `--internet` and `--network` come first, in any order.
- When scripting `opencode run`: `--file=` with its `=` and `--` before the prompt, no `<` or `>` in the prompt,
  stdin from `/dev/null`, output to a log file, not a pipe.

Checks: `ls -A ~/.local/share/opencode` lists only `bin` and `touch` there is refused in every mode; the real
database's session count is the same after a run; no database copy is left in the temporary folder; under `script`
the wrapper opens opencode's screen; `--persist run` twice, the second with `--continue`, recalls the first.

## pi (`scripts/pi-box`, build mode)

- pi writes its config folder as it runs (a lock beside `trust.json`), so it gets a `mktemp -d` copy (mode 700, it
  holds keys) named in `PI_CODING_AGENT_DIR`, kept in `BOX_KEEP` and removed in the EXIT trap. Sessions and settings
  changes go with it: no resume.
- Copied: `settings.json`, `models.json`, `models-store.json`, `trust.json`, `auth.json`, `AGENTS.md` (with
  `cp -L`: it may link outside the box). `mcp.json` stays out: its servers would start programs.
- Its program, `~/.pi/agent/bin` and `install`, is read-only.
- Hosts are `PI_HOSTS`, keys `PI_KEYS`, by name. Every key in `PI_KEYS` is in the box, and the model could send
  one to another host in `PI_HOSTS` (with `--internet`, to any host): add only the providers you use.

Checks: a one-prompt run in `--mode json` with stdin closed answers; no command line holds a key during the run;
after a SIGHUP the config copy is gone. A SIGKILL leaves it (mode 700) in the temporary folder.

## prime-agent (`scripts/prime-agent-box`, build mode)

- Its config (`~/.prime/agent`) and program (`~/.local/share/prime-agent`) are read-only, so it saves no session:
  run it with `--no-session`.
- Of its config folder only `PRIME_SHOWN` is there (program parts, models, skills, its MCP catalog, settings,
  `auth.json`); every other entry is hidden, a new one too: `keys` (a web-search key), and the sessions, logs,
  command journals (`daemon-workers`, with other projects' paths), ledgers and telemetry of every project.
- One-shot only (`-p`): its interactive screen needs a daemon that cannot start in the box.
- `PRIME_AGENT_KERNEL_PYTHON` names its kernel venv in the read-only config, or it tries to rebuild it there.
- The key is `OPENROUTER_API_KEY` by name: its `auth.json` holds only Anthropic's.

Checks: a one-prompt run answers; `ls` of each hidden folder is empty in the box and each hidden file unreadable (and a misspelled name is
empty too, so also check they are not empty outside); no command line holds a key during the run.
