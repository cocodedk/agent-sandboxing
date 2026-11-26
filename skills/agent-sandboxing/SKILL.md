---
name: agent-sandboxing
description: Boxes an agent CLI (opencode, pi, prime-agent or another) in srt, with only the repository and the folders, hosts and variables it needs, its git-ignored secrets hidden. Use to sandbox or box an agent CLI, change a box, or fix a boxed run missing a file, host or variable. Not for containers, VMs or Claude Code's own sandbox settings (update-config).
compatibility: Linux with bubblewrap (bwrap), socat, ripgrep, git, python3 (standard library), Node.js, and srt (npm install -g @anthropic-ai/sandbox-runtime).
---

# agent-sandboxing

The box is `scripts/agent-box.sh`, sourced, not run. A wrapper sources it, sets the agent's `BOX_*` values and
calls `agent_box <agent> <args...>` from inside a git repository. The box then holds that repository, the system's
programs and the folders the wrapper names; nothing else of the home, no git-ignored file, no host but the agent's
own, and no environment variable but the ones kept by name. Why that matters, for telling the user: why.md.

Paths below are under this skill's folder (`<skill-dir>`), not the user's repository.

## Workflow

1. **Check requirements.** `for c in bwrap socat rg git python3 node srt; do command -v "$c" > /dev/null || echo "missing: $c"; done`
   prints nothing when all are there; a missing srt is `npm install -g @anthropic-ai/sandbox-runtime`. Then run
   `bash <skill-dir>/scripts/probe.sh`; it makes its own throwaway repository. Every line PASS means srt works here.
   A FAIL names its check; srt.md explains most of them.
2. **Survey the agent.** If it is opencode, pi or prime-agent, read agents.md and use its wrapper. Otherwise find
   its program, config, state, login, hosts, background services and the writes it makes, per survey.md.
3. **Check the repository.** From the repository the agent will work in, run `bash <skill-dir>/scripts/preflight.sh`.
   Before the first boxed run there, tell the user each WARN in plain words: uncommitted work the box will not protect,
   or a key in the history or a remote URL that the box can read. A WARN does not block; the user decides.
4. **Write the wrapper.** Start from the minimal wrapper in settings.md, or copy the nearest one (pi-box for an agent with an API key, prime-agent-box for one with
   read-only config, opencode-box for one with a login database) and set each value per settings.md. These lines
   carry secrets or cleanup; keep them exactly as the templates have them:
   - keys by name in `BOX_KEEP`, never `NAME=value` on a command line;
   - a config the agent writes is a `mktemp -d` copy, removed in the EXIT trap;
   - `trap '<cleanup>; agent_box_sweep' EXIT` and `trap 'exit 129' HUP INT TERM` in every writable mode;
   - `if (return 0 2> /dev/null); then return 0; fi` after the settings, so the probe can source the wrapper;
   - no folder that holds credentials in `BOX_READ`, `BOX_WRITE` or `BOX_TOOLS`; hide it with `BOX_HIDE`;
   - `[ -z "$BOX_CHANGED" ] || echo "$BOX_CHANGED" >&2` after a writable run.
5. **Probe it.** `bash <skill-dir>/scripts/probe.sh <wrapper> <the agent's host>` must print no FAIL. It runs all three
   modes, whatever the wrapper's own, fails a folder that `agent_box_refused` refuses (the broad roots and the named
   credential folders; any other folder stays your judgement), and sees only the settings made before the wrapper's `return`: a `BOX_WRITE`
   set after it (a config or database copy) is checked by the real run alone. An HTTP error such as 404 from the host
   is a PASS: the host was reached. Then make the checks probe.sh cannot (srt.md, and the agent's own in agents.md) and one real one-prompt run. A failure goes back to
   step 2 if something the agent needs is missing, to step 4 if the box lets too much in.
6. **Wire it in.** The skill or command that runs the agent calls the wrapper instead of the agent, passes its
   arguments through, and prints `BOX_CHANGED` when set. Tell the user what limits.md says the box leaves open.

## Files

- [references/settings.md](references/settings.md) — every `BOX_*` value, the three modes, the functions. Step 4.
- [references/survey.md](references/survey.md) — how to find what an agent needs, and the checks that found the
  known pitfalls. Step 2.
- [references/agents.md](references/agents.md) — opencode, pi and prime-agent: what each needs and its traps. Steps
  2 and 5.
- [references/srt.md](references/srt.md) — srt's quirks and the checks probe.sh does not make. Steps 1, 5, and any
  change to agent-box.sh.
- [references/limits.md](references/limits.md) — what the box does not protect. Steps 3 and 6.
- [references/why.md](references/why.md) — why sandbox an agent: what the box does, the dangers it closes. Step 6.
- `evals/cases.yaml` — trigger and outcome cases for this skill; run them after changing the description.
- `scripts/agent-box.sh` — the box. Sourced by every wrapper; read its header only when changing it.
- `scripts/probe.sh` — proves a box in a throwaway repository. Run it; don't read it.
- `scripts/preflight.sh` — read-only check of a repository for uncommitted work, a token in its git config and keys
  anywhere in `.git`; OK or WARN per line. Step 3. Run it; don't read it.
- `scripts/preflight_scan.py` — the scanner `preflight.sh` runs on every object in `.git` and the local git config.
- `scripts/opencode-box`, `scripts/pi-box`, `scripts/prime-agent-box` — the three wrappers; run them as the
  agent itself, or copy one as a template. Each header says what is in its box.
