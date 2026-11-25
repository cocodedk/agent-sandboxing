# Why sandbox an agent

An agent CLI runs a language model's commands with the user's own rights. Without a box, one bad prompt, a poisoned
file in the repository or a confused model can do whatever the user can.

## What a box does

It runs the agent as a normal process that sees only part of the machine: the repository, the agent's own folders, the system's programs,
the hosts the agent needs and the environment variables kept by name. Everything else is not there.

## The dangers it closes

- **Secrets.** Without a box the agent can read `.env` files, SSH and cloud keys, browser data and every other
  tool's login in the home. In the box the repository's git-ignored files are hidden and the rest of the home is gone.
- **Sending data out.** Without a box the agent can send what it reads to any host. In the box it reaches only the
  hosts named for it, though those hosts can still carry data out (limits.md).
- **Writes outside the task.** Without a box it can change shell start-up files, git hooks or other projects. In the
  box it writes only the repository (never `.git`) and the folders named for it.
- **Ways around the box.** A background service the agent hands its work to runs with full rights; the box makes the
  agent do the work itself, and hides the service's password.

## The benefit

The user can let an agent work on its own, for longer, on code they did not write, and know the worst it can reach.
A box does not make the agent's work safe; limits.md says what is left open.
