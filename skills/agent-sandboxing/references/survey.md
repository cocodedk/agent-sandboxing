# Surveying an agent

Goal: a list of what the agent touches, each item marked read, write, hide or leave out. Run the agent once outside
the box first, on a throwaway repository, so you know what a working run looks like.

## What to find

- **Program.** `readlink -f "$(command -v <agent>)"`. Its install folder goes in `BOX_READ` and its `bin` in
  `BOX_PATH`. Programs it keeps elsewhere (a bundled `rg`, language servers) too, read-only: they run outside the box
  later, and a writable one could be replaced.
- **Config and state.** Look in `~/.<agent>`, `~/.config/<agent>`, `~/.local/share/<agent>`, `~/.cache/<agent>`, and
  the variables its docs name (`*_DIR`, `*_DB`, `*_HOME`). Mark each file: config it reads, keys, sessions of other
  projects, logs. Sessions and logs of other projects are hidden; they hold other work.
- **Login.** An API key in the environment is kept by name in `BOX_KEEP`. A key in a file, a login database or a
  keychain needs its own answer: a copy with only the login (opencode), or a config copy (pi).
- **Writes.** Before and after a run:
  `marker="$(mktemp)"; <agent> <one prompt>; find "$HOME" -newer "$marker" -type f 2> /dev/null; rm -f "$marker"`.
  A write into its config folder means that folder cannot be read-only: give it a copy.
- **Hosts.** The provider's API host, the login host, a model-list host. Start with the documented ones; a boxed
  run that stalls or says "connection refused" on a host shows the one missing.
- **Background services.** After a run, `ps -ef | grep -i <agent>` and `ss -lxp | grep -i <agent>`. A service or
  daemon the CLI hands its work to runs outside the box, with the user's full rights: the box must make the CLI run
  the work itself, or that mode cannot be boxed.
- **Ways out.** A password or socket path for that service, an MCP server list (it starts programs), a plugin
  folder, a hook. Hide them or leave them out.
- **Interactive and one-shot.** Try both in the box; one may need a daemon that cannot start there.

## Checks that found the known pitfalls

| Check | Found |
|---|---|
| `ls -A ~` asked of the model in a boxed run | opencode's shell listed the whole home: the work ran in its background service, outside |
| `ps -ef` during a run | `opencode serve --service`; fixed with `--standalone` |
| a boxed run without the store | "API key has expired": opencode's login is in its database, not `auth.json` |
| `opencode session list` in and out of the box | it read the real database through the service |
| the `find -newer` above | pi writes a lock beside `trust.json`, so its config folder needs a copy |
| an interactive start in the box | prime-agent's daemon cannot start: one-shot only |
| a run with its config read-only | prime-agent cannot save a session: `--no-session` |
| `ls` of each config subfolder | prime-agent's keys, sessions, logs, command journals and telemetry: hide all but a named list |
| `cat /proc/*/cmdline` during a run | whether a key is on any command line |

When a check finds something new, add the agent to agents.md with the check that found it.
