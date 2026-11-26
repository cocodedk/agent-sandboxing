# shellcheck shell=bash
# agent-box: an agent CLI (opencode, pi, prime-agent or any other) in srt, Anthropic's sandbox-runtime on bubblewrap,
# run from a git repository on Linux. The box holds the repository, the system's programs and the folders its caller
# names, and no other folder of the home; the repository's git-ignored files (.env, keys, local data) are hidden. Its
# network is the hosts its caller names, through srt's proxy outside the box: no other host and none of this machine's
# services. A wrapper started with --internet widens it to any host outside the local network, with --network to the
# local network's devices by address (agent_box_net_flag); never to this machine's own services. Its environment is
# HOME, PATH, LANG, TERM, TMPDIR and the variables its caller keeps. Sourced, not run.
# Needs git, python3, bwrap, ip, and srt (npm install -g @anthropic-ai/sandbox-runtime) with socat and ripgrep.

# Set by the caller after sourcing this file, before agent_box:
BOX_MODE=review   # review: the repository read-only; write: writable but its .git; build: that, plus BOX_TOOLS and
                  # the dependency folders, so the project's checks run
BOX_DOMAINS=()    # the hosts the agent reaches
BOX_READ=()       # its own folders, read-only
BOX_WRITE=()      # its own folders, writable
BOX_HIDE=()       # files or folders in those to hide; a hidden folder is an empty one, thrown away at the end
BOX_FROZEN=()     # folders in BOX_WRITE kept read-only
BOX_SHOWN=()      # name patterns of git-ignored files a script leaves in the repository for the agent, shown anyway
BOX_KEEP=()       # the environment variables passed in, by name, so no value is on a command line
BOX_PATH=""       # the agent's own program folders, searched before the system's
BOX_TERM=dumb     # the terminal the box tells the agent it has; an interactive command passes the real one
BOX_SKILLS=()     # skill folders the agent loads, read-only (for example "$HOME/.claude/skills")
# Build mode only: the home's programs the project's checks need, read-only. Without --internet no package registry
# is reachable, so nothing gets installed there. Fill them with agent_box_tools, which resolves links and refuses
# folders that hold other tools' state (BOX_TOOLS_REFUSED); by hand, resolve with readlink -f and check with
# agent_box_refused. Leave out a folder holding credentials (uv's own does: name only its python and tools folders).
BOX_TOOLS=()      # read-only folders
BOX_TOOLS_PATH="" # colon-separated program folders, put before the rest of PATH
BOX_ALL_SOCKETS="" # allowAllUnixSockets: empty decides from AppArmor (BOX_USERNS_GUARD); 1 forces it on, 0 off
# Set by agent_box_net_flag from the words the wrapper was started with, never by hand or from the environment:
BOX_INTERNET=""   # --internet: any host outside the local network
BOX_LAN=""        # --network: the local network's devices, by address
# AGENT_BOX_READ, set by whoever runs the agent: more read-only folders, colon-separated (a test venv elsewhere).
# Set by agent_box:
BOX_CHANGED=""    # the line naming the git-ignored files a writable run changed or made, for the caller to print
BOX_CWD=""        # the folder srt guarded in a writable run, and the guarded names missing there before (the sweep)
BOX_BEFORE=()
BOX_RUN=""        # the run's temporary folder (settings, proxy sockets), removed by agent_box or the sweep

# No /run: its systemd-resolved socket would let the box send names out as DNS queries; the proxy resolves outside.
readonly BOX_SYSTEM=(/usr /etc)
readonly BOX_SYSTEM_PATH="/usr/local/bin:/usr/bin:/bin"
# Reads 1 where AppArmor stops unprivileged user namespaces (Ubuntu 23.10 and later). srt's seccomp step needs one
# inside its own and fails there, so the box then runs with allowAllUnixSockets, which turns srt's seccomp filter off.
# The box still sees no socket of the host's (no /run), and the host's abstract sockets stay behind the network.
readonly BOX_USERNS_GUARD=/proc/sys/kernel/apparmor_restrict_unprivileged_userns
# The ignored folders a build's checks need are shown and writable (test runners write into node_modules); every file
# the box changed in them is listed after the run, as they run outside the box later and no diff shows them. By change
# time (ctime), which the agent cannot set back as it can a modification time.
readonly BOX_DEPS=(node_modules .venv venv)
# srt's proxy comes into the box as a socket in srt's temporary folder, which the box would hide. Each run gives srt a
# short folder of its own (TMPDIR), so the box sees its own proxy's socket and no other srt run's.
readonly BOX_SOCKETS=("claude-http-*.sock" "claude-socks-*.sock")
# What srt guards in the working directory when the repository is writable (srt 0.0.83's DANGEROUS_FILES), making an
# empty file or folder on the host for each one missing; children before parents.
readonly BOX_PLACEHOLDERS=(.gitconfig .gitmodules .bashrc .bash_profile .zshrc .zprofile .profile .ripgreprc
  .mcp.json .vscode .idea .claude/commands .claude/agents .claude)
# Folders of the home that hold other tools' state and logins: no BOX_TOOLS folder may be one or hold one.
readonly BOX_TOOLS_REFUSED=("$HOME" "$HOME/.local" "$HOME/.local/share" "$HOME/.config" "$HOME/.cache")
# Common credential folders and files of the home: no folder the box names may be one, be inside one or hold one.
readonly BOX_CREDENTIALS=(.ssh .aws .gnupg .config/gh .config/gcloud .docker .kube .netrc .git-credentials
  .password-store)
# With --network, only the box's own loopback skips srt's proxy (srt's NO_PROXY skips the private ranges too).
readonly BOX_NO_PROXY="localhost,127.0.0.1,::1"
BOX_NET_RUNNER="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/agent-box-net.mjs"   # srt with a wider network
readonly BOX_NET_RUNNER

# agent_box_net_flag <word>: true, and the box's network widened, when the word is --internet (any host outside the
# local network: web pages, package registries, any API) or --network (the local network's devices, by address). A
# wrapper passes it the words of its own command line, so only whoever starts the wrapper widens the box.
agent_box_net_flag() {
  case "$1" in
    --internet) BOX_INTERNET=1 ;;
    --network) BOX_LAN=1 ;;
    *) return 1 ;;
  esac
}

# agent_box_refused <folder>: true when the folder is, or holds, one in BOX_TOOLS_REFUSED, or is, is inside or holds
# one in BOX_CREDENTIALS. Any other folder is the caller's judgement.
agent_box_refused() {
  local r p="${1%/}/"
  for r in "${BOX_TOOLS_REFUSED[@]}"; do [[ "$r/" != "$p"* ]] || return 0; done
  for r in "${BOX_CREDENTIALS[@]/#/$HOME/}"; do [[ "$r/" != "$p"* && "$p" != "$r/"* ]] || return 0; done
  return 1
}

# agent_box_tools: adds to BOX_TOOLS and BOX_TOOLS_PATH what this machine has of node's folder, npm's global prefix,
# and uv's python and tools folders, with links resolved. A folder agent_box_refused names is left out.
agent_box_tools() {
  local bin dir
  _box_tool() { [ -n "$1" ] && [ -d "$1" ] && ! agent_box_refused "$1" && BOX_TOOLS+=("$1"); }
  if bin="$(command -v node)" && bin="$(readlink -f "$bin")"; then
    bin="${bin%/*}"
    ! _box_tool "${bin%/*}" || BOX_TOOLS_PATH="${BOX_TOOLS_PATH:+$BOX_TOOLS_PATH:}$bin"
  fi
  if command -v npm > /dev/null && dir="$(npm prefix -g 2> /dev/null)" && dir="$(readlink -f "$dir")"; then
    ! _box_tool "$dir" || BOX_TOOLS_PATH="${BOX_TOOLS_PATH:+$BOX_TOOLS_PATH:}$dir/bin"
  fi
  if command -v uv > /dev/null; then
    for dir in "$(uv python dir 2> /dev/null)" "$(uv tool dir 2> /dev/null)"; do _box_tool "$dir" || true; done
  fi
  unset -f _box_tool
}

# agent_box <command...>: the command in the box, in this directory.
agent_box() {
  local top git srt node run write="" path reads=() changed home=$HOME rc=0 p sockets reach
  BOX_CHANGED="" BOX_CWD="" BOX_BEFORE=()
  top="$(git rev-parse --show-toplevel)" && git="$(git rev-parse --path-format=absolute --git-common-dir)" || return 2
  [[ "$HOME/" != "${top%/}/"* ]] || { echo "${0##*/}: $top holds the home; run it from a repository" >&2; return 2; }
  srt="$(command -v srt)" \
    || { echo "${0##*/}: srt is missing: npm install -g @anthropic-ai/sandbox-runtime" >&2; return 2; }
  node="$(command -v node)" || { echo "${0##*/}: node is missing, and srt runs on it" >&2; return 2; }
  case "$BOX_MODE" in
    review) ;;
    write | build) write="$top" BOX_CWD="$PWD"
      for p in "${BOX_PLACEHOLDERS[@]}"; do [ -e "$p" ] || BOX_BEFORE+=("$p"); done ;;
    *) echo "${0##*/}: BOX_MODE is review, write or build, not $BOX_MODE" >&2; return 2 ;;
  esac
  path="${BOX_PATH:+$BOX_PATH:}$BOX_SYSTEM_PATH"
  if [ -n "$BOX_INTERNET" ] && [ -n "$BOX_LAN" ]; then reach=both
  else reach="${BOX_INTERNET:+internet}${BOX_LAN:+network}"; fi
  if [ "$BOX_MODE" = build ]; then
    reads=(${BOX_TOOLS[@]+"${BOX_TOOLS[@]}"}) path="${BOX_TOOLS_PATH:+$BOX_TOOLS_PATH:}$path"
  fi
  if [ -n "${AGENT_BOX_READ:-}" ]; then
    IFS=: read -ra p <<< "$AGENT_BOX_READ"
    for p in "${p[@]}"; do [ -z "$p" ] || reads+=("$p"); done   # a stray colon names no folder
  fi
  sockets="$BOX_ALL_SOCKETS"
  [ -n "$sockets" ] || { [ "$(cat "$BOX_USERNS_GUARD" 2> /dev/null)" = 1 ] && sockets=1 || sockets=0; }
  run="$(mktemp -d)" BOX_RUN="$run"
  git -C "$top" ls-files -z --others --ignored --exclude-standard --directory | tee "$run/ignored" | python3 -c '
import fnmatch, json, os, sys
top, git, write, build, keep, deps, hidden, reads, writes, frozen, domains, sockets, shown_file = sys.argv[1:]
lines = lambda s: [x for x in s.split("\n") if x]
hide, shown = [], []
for p in sys.stdin.read().split("\0"):
    name, folder = os.path.basename(p.rstrip("/")), p.endswith("/")
    if not p or (not folder and any(fnmatch.fnmatch(name, k) for k in lines(keep))):
        continue
    (shown if build and folder and name in lines(deps) else hide).append(os.path.join(top, p.rstrip("/")))
open(shown_file, "w").write("".join(s + "\n" for s in shown))
print(json.dumps({
    "network": {"allowedDomains": lines(domains), "deniedDomains": [], "allowAllUnixSockets": sockets == "1"},
    "filesystem": {"denyRead": ["/"] + lines(hidden) + hide, "allowRead": lines(reads) + [top, git],
                   "allowWrite": lines(writes) + lines(write),
                   "denyWrite": lines(frozen) + ([os.path.join(top, ".git"), git] if write else [])}}))' \
    "$top" "$git" "$write" "$([ "$BOX_MODE" != build ] || echo 1)" "$(printf '%s\n' ${BOX_SHOWN[@]+"${BOX_SHOWN[@]}"})" \
    "$(printf '%s\n' "${BOX_DEPS[@]}")" "$(printf '%s\n' ${BOX_HIDE[@]+"${BOX_HIDE[@]}"})" \
    "$(printf '%s\n' "${BOX_SYSTEM[@]}" ${BOX_SKILLS[@]+"${BOX_SKILLS[@]}"} ${BOX_READ[@]+"${BOX_READ[@]}"} \
      ${BOX_WRITE[@]+"${BOX_WRITE[@]}"} ${reads[@]+"${reads[@]}"} "${BOX_SOCKETS[@]/#/$run/}")" \
    "$(printf '%s\n' ${BOX_WRITE[@]+"${BOX_WRITE[@]}"})" "$(printf '%s\n' ${BOX_FROZEN[@]+"${BOX_FROZEN[@]}"})" \
    "$(printf '%s\n' ${BOX_DOMAINS[@]+"${BOX_DOMAINS[@]}"})" "$sockets" "$run/shown" > "$run/settings.json" \
    || { rm -rf "$run"; BOX_RUN=""; return 2; }
  # The environment is emptied in a subshell, not with env -i NAME=value, so a kept key is on no command line. srt gives
  # the command a TMPDIR inside the box that does not exist yet; tools that make temporary folders need it. The --
  # keeps srt from reading the command's own -c, -h or --version as its options.
  (
    for p in $(compgen -e); do [[ " ${BOX_KEEP[*]-} " == *" $p "* ]] || unset "$p" 2> /dev/null || true; done
    while read -r _ _ p; do export -fn "$p"; done < <(declare -Fx)
    export HOME="$home" PATH="$path:${node%/*}" LANG=C.UTF-8 TERM="$BOX_TERM" TMPDIR="$run"
    [ -n "$reach" ] || exec "$srt" -s "$run/settings.json" -- bash -c 'mkdir -p "$TMPDIR" && exec "$@"' agent-box "$@"
    exec "$node" "$BOX_NET_RUNNER" "$(dirname "$(readlink -f "$srt")")" "$run/settings.json" "$reach" -- bash -c \
      'mkdir -p "$TMPDIR" && { [ -z "$1" ] || export NO_PROXY="$1" no_proxy="$1"; } && shift && exec "$@"' \
      agent-box "${BOX_LAN:+$BOX_NO_PROXY}" "$@"
  ) || rc=$?
  if [ -n "$write" ]; then   # no diff shows git-ignored files: list those the run changed in the shown dependency
    changed="$run.changed"   # folders (by ctime) and every ignored path it made; beside the run's folder, to outlive it
    { [ ! -s "$run/shown" ] || xargs -d '\n' -I{} find {} -cnewer "$run/settings.json" ! -type d < "$run/shown"
      git -C "$top" ls-files -z --others --ignored --exclude-standard --directory > "$run/ignored.after"
      comm -z -13 <(sort -z "$run/ignored") <(sort -z "$run/ignored.after") | sed -z 's,/$,,' \
        | xargs -0 -r -I{} find "$top/{}" ! -type d
    } 2> /dev/null | sort -u > "$changed" || true
    if [ -s "$changed" ]; then
      BOX_CHANGED="$(printf '%s: the box changed or made %s git-ignored files; read them before running them: %s' \
        "${0##*/}" "$(wc -l < "$changed")" "$changed")"
    else rm -f "$changed"; fi
  fi
  rm -rf "$run"
  BOX_RUN=""
  return "$rc"
}

# agent_box_sweep: the placeholders srt made in the working directory, which it leaves when killed (a closed terminal, a
# stopped job). Only those missing before agent_box began and still empty go. Also the run's folder, if a signal ended
# agent_box early. Call it from the caller's EXIT trap, with trap 'exit 129' HUP INT TERM.
agent_box_sweep() {
  local p
  [ -z "$BOX_RUN" ] || rm -rf "$BOX_RUN"
  [ -n "$BOX_CWD" ] || return 0
  for p in ${BOX_BEFORE[@]+"${BOX_BEFORE[@]}"}; do
    p="$BOX_CWD/$p"
    if [ -f "$p" ] && [ ! -s "$p" ]; then rm -f "$p"; elif [ -d "$p" ]; then rmdir "$p" 2> /dev/null || true; fi
  done
}
