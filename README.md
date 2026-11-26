# agent-sandboxing

A Claude Code skill that runs other AI coding agent CLIs (opencode, pi, prime-agent and others) in a sandbox, plus a page that explains why and how. The sandbox shows the agent only the project folder, the programs it needs and, by default, the few internet hosts it must reach, and clears every environment variable you did not name. It is built on Anthropic's sandbox-runtime (srt) and runs on Linux.

## Website

Why sandbox AI coding agents: the risks, what a sandbox does and does not do, and how to start.

- [English](https://sandbox.cocode.dk/)
- [Dansk](https://sandbox.cocode.dk/da/)
- [Español](https://sandbox.cocode.dk/es/)
- [简体中文](https://sandbox.cocode.dk/zh/)

The skill itself is written in English.

## Install

You need Linux, Claude Code, bubblewrap, socat, ripgrep, git, python3, iproute2, Node.js and srt. The steps use Debian or Ubuntu.

1. Install the system tools:

   ```
   sudo apt install bubblewrap socat ripgrep git python3 iproute2
   ```

   The command finishes without errors.
2. Install Node.js from nodejs.org or your package manager, then run `npm --version`. It should print a version number.
3. Install srt:

   ```
   npm install -g @anthropic-ai/sandbox-runtime
   ```

   Run `srt --version`. It should print a version number.
4. Download the skill:

   ```
   git clone https://github.com/cocodedk/agent-sandboxing
   ```

   A folder named `agent-sandboxing` appears.
5. Copy the `skills/agent-sandboxing` folder from the download into `~/.claude/skills/`:

   ```
   mkdir -p ~/.claude/skills && cp -r agent-sandboxing/skills/agent-sandboxing ~/.claude/skills/
   ```

   You now have `~/.claude/skills/agent-sandboxing`.
6. In Claude Code, ask for a sandbox, for example "sandbox opencode" or "put my agent CLI in a box". For opencode, pi and prime-agent, Claude uses the ready-made wrapper. For any other agent, it surveys what the agent needs and writes a small wrapper script. Before the first run in a repository, it checks for uncommitted work, a token in the git config and secrets in the git history, staged files included, and tells you where they are, never their values. Then it runs the skill's probe test. The test tries each fence: a hidden `.env`, a read-only `.git`, a refused host and cleared variables. It prints PASS or FAIL per check and should end with 0 failed. The test shows that the sandbox keeps to the wrapper's settings and refuses common credential folders such as `~/.ssh`; whether the other folders and hosts in the wrapper are safe to share is still yours to judge.

The skill includes ready-made wrappers: opencode-box, pi-box and prime-agent-box. The skill sandboxes other agent CLIs that Claude Code runs. It does not sandbox Claude Code itself, which has its own sandbox settings.

By default the sandbox reaches only the agent's own hosts. To let an agent reach more, start its wrapper with `--internet` (any host outside your local network: web pages, package registries, any API) or `--network` (the devices on your local network, by address), for example `pi-box --internet`. Neither reaches services on your own computer, and an agent in the sandbox cannot turn either on or start a sandbox with more. With `--internet`, every key in the sandbox can be sent to any host.

## What is in `skills/agent-sandboxing`

- `SKILL.md`: what Claude Code does when you ask for a sandbox.
- `references/`: why sandbox, how srt works, the settings, the ready-made agents, how to survey a new agent, and the limits.
- `scripts/agent-box.sh`: the box, sourced by each wrapper; turns an agent's list of hosts, folders and kept variables into srt's settings.
- `scripts/opencode-box`, `scripts/pi-box`, `scripts/prime-agent-box`: the ready-made wrappers.
- `scripts/agent-box-net.mjs`: runs srt with the wider network that `--internet` and `--network` ask for.
- `scripts/probe.sh`: the probe test that tries each fence and prints PASS or FAIL per check.
- `scripts/preflight.sh`: a read-only check of your repository before a boxed run; it warns about uncommitted work, a token in the git config, and secrets in any object in `.git`: the history, tags, staged files and objects no commit holds any more. `scripts/preflight_scan.py` does the scanning; it needs python3.
- `evals/cases.yaml`: trigger and outcome cases for the skill.

## Limits

- It shares your computer's kernel, like a container; a virtual machine isolates more.
- Allowed hosts, such as the model provider, still receive whatever is inside the sandbox, and a tricked agent can send it there under an attacker's own API key. With `--internet`, that is any host.
- Git history stays readable: a secret that was ever committed, or a token in a remote URL in `.git/config`, is still there; remove it from the history and replace the key before you run an agent. `scripts/preflight.sh` finds such secrets in every commit, tag and staged file, even in objects no commit holds any more, and shows where they are, never their values; it does not read Git LFS files.
- Code the agent writes runs outside the sandbox once you run, push or install it: review the diff first.
- Package registries are blocked unless you start the wrapper with `--internet`, so by default nothing installs inside the sandbox.
- The project folder is not protected: commit or stash first, or use a separate worktree.
- On Ubuntu, srt's extra system-call filter (seccomp) is switched off so the sandbox can start.
- Linux only.

## Credit

The sandbox this project wraps is Anthropic's [sandbox-runtime](https://github.com/anthropics/sandbox-runtime) (npm `@anthropic-ai/sandbox-runtime`), licensed under Apache-2.0. This project only adds the settings for each agent and the wrappers around it.

## Author

**Babak Bandpey**: [https://cocode.dk](https://cocode.dk) | [LinkedIn](https://linkedin.com/in/babakbandpey) | [GitHub](https://github.com/cocodedk)

## License

Apache-2.0 | © 2026 [Cocode](https://cocode.dk) | Created by [Babak Bandpey](https://linkedin.com/in/babakbandpey) | [LICENSE](LICENSE)
