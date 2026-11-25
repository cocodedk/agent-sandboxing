# The box's settings

A wrapper sources `scripts/agent-box.sh`, then sets these and calls `agent_box`. An unset value is empty.

## Modes (`BOX_MODE`)

| Mode | Repository | Also |
|---|---|---|
| `review` (default) | read-only | — |
| `write` | writable, but not `.git` (hooks included) | srt's placeholder files: call `agent_box_sweep` in the EXIT trap; `BOX_CHANGED` set after the run |
| `build` | as `write` | `BOX_TOOLS` read-only; the dependency folders (`node_modules`, `.venv`, `venv`) shown and writable |

Pick the least the task needs: review for reading and reviewing, write for edits, build only when the project's
checks must run in the box. No package registry is reachable in any mode, so nothing gets installed.

## Values the wrapper sets

| Value | What it is |
|---|---|
| `BOX_DOMAINS` | hosts the agent reaches, through srt's proxy; `*.example.com` covers subdomains |
| `BOX_READ` | the agent's own folders, read-only: its program, its config |
| `BOX_WRITE` | its folders it must write: a config copy, a database copy |
| `BOX_HIDE` | files or folders inside those to hide; a hidden folder is empty and writable in the box, thrown away at the end |
| `BOX_FROZEN` | folders inside `BOX_WRITE` kept read-only |
| `BOX_SHOWN` | name patterns of git-ignored files a script leaves in the repository for the agent (a diff, a brief) |
| `BOX_KEEP` | environment variables passed in, by name, so no value is on a command line |
| `BOX_PATH` | the agent's program folders, searched before the system's |
| `BOX_TERM` | the terminal the agent is told it has; `dumb` by default, `"${TERM:-dumb}"` for an interactive screen |
| `BOX_SKILLS` | skill folders the agent loads, read-only (`"$HOME/.claude/skills"`) |
| `BOX_TOOLS` | build mode: the home's program folders the checks need, read-only |
| `BOX_TOOLS_PATH` | build mode: their program folders, colon-separated, put first on `PATH` |
| `BOX_ALL_SOCKETS` | srt's `allowAllUnixSockets`: empty decides from AppArmor (srt.md), `1` forces it on, `0` off |

`agent_box_tools` fills `BOX_TOOLS` and `BOX_TOOLS_PATH` with node's folder, npm's global prefix and uv's python
and tools folders, links resolved, leaving out any folder that is or holds one in `BOX_TOOLS_REFUSED` (the home,
`~/.local`, `~/.local/share`, `~/.config`, `~/.cache`): those hold other tools' logins. probe.sh fails a
`BOX_TOOLS` that holds one. Never add uv's own folder or any other that holds credentials.

The user running the wrapper may set `AGENT_BOX_READ`: more read-only folders, colon-separated (a test venv kept
elsewhere). A stray colon adds nothing.

## Set by `agent_box`

| Value | What it is |
|---|---|
| `BOX_CHANGED` | after a writable run, a line naming the file that lists every git-ignored file the run made, and every file it changed in a shown dependency folder (by change time, which the agent cannot set back). Print it: those files run outside the box later and no diff shows them |
| `BOX_RUN` | the run's temporary folder; `agent_box` removes it, or `agent_box_sweep` when a signal cut the run short |
| `BOX_CWD`, `BOX_BEFORE` | the folder srt guarded and the guarded names missing there before the run; `agent_box_sweep` reads them |

## Fixed in agent-box.sh

`BOX_SYSTEM` (`/usr`, `/etc`; no `/run`, whose DNS socket would let names out), `BOX_SYSTEM_PATH`, `BOX_DEPS`,
`BOX_SOCKETS` (srt's proxy sockets), `BOX_PLACEHOLDERS` (srt's guarded names), `BOX_USERNS_GUARD`. Change them only
with srt.md open.

## The environment in the box

`HOME`, `PATH`, `LANG=C.UTF-8`, `TERM`, `TMPDIR` (a short folder of the run's own), srt's proxy variables, and
`BOX_KEEP`'s. The rest is unset in a subshell before srt starts, and exported shell functions are dropped.

## What the box sees

The system's `/usr` and `/etc`, the repository (its git-ignored files hidden) and the folders named above. Of
`/tmp` only the repository, if it is there, the run's own folder and srt's own; a script in `/tmp` needs `BOX_READ`. The rest
of the home is not there at all: `~/.ssh` gives "No such file or directory", not "Permission denied". `.git`,
hooks included, is never writable.

## A minimal wrapper

A wrapper may live anywhere; it sources `agent-box.sh` by an absolute path, or by its own folder when it sits beside
it, as the three wrappers do.

```bash
#!/usr/bin/env bash
set -euo pipefail
source "$HOME/.claude/skills/agent-sandboxing/scripts/agent-box.sh"   # where the skill is installed
readonly MY_HOSTS=(api.example.com)
BOX_MODE=write
BOX_DOMAINS=("${MY_HOSTS[@]}")
BOX_READ=("$HOME/.my-agent")   # its program and config, read-only
BOX_KEEP=(MY_AGENT_API_KEY)
if (return 0 2> /dev/null); then return 0; fi
trap agent_box_sweep EXIT
trap 'exit 129' HUP INT TERM
rc=0
agent_box my-agent "$@" || rc=$?
[ -z "$BOX_CHANGED" ] || echo "$BOX_CHANGED" >&2
exit "$rc"
```

## Functions

- `agent_box <command...>` — runs the command in the box, in the current folder. Returns its exit code, or 2
  when the box cannot start (not a repository, the repository holds the home, srt or node missing, a bad mode).
- `agent_box_tools` — see above; call it before `agent_box`, in build mode.
- `agent_box_sweep` — removes srt's placeholders left by a killed run: only those missing before and still empty.
