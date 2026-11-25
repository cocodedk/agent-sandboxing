#!/usr/bin/env bash
# probe.sh [settings] [host]: proves a box. settings is a file that sets the box (agent-box.sh itself, a file that
# sources it and sets BOX_*, or a wrapper such as pi-box, which only sets its box when sourced), given as a path or as
# the name of a script beside this one or on PATH; agent-box.sh beside this one by default. host is a host the box
# must reach (added to BOX_DOMAINS), api.anthropic.com by default. In a temporary git repository with a git-ignored
# .env and node_modules, it runs agent_box in each mode and checks: the home lists only the folders the box names,
# .env is refused, .git is not writable, the repository is writable only in write and build, the host answers and
# https://example.com does not, only kept variables are in the environment, a git-ignored file the run makes is in
# BOX_CHANGED (write, build), so is a file planted in node_modules with an old modification time (build), and BOX_TOOLS
# holds no folder of other tools' state (build). Prints PASS or FAIL per check and the totals; exits 1 on any FAIL.
set -euo pipefail
readonly PROBE_DIR="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"
readonly PROBE_DEFAULT_SETTINGS="$PROBE_DIR/agent-box.sh"
readonly PROBE_DEFAULT_HOST=api.anthropic.com
readonly PROBE_DENIED_URL=https://example.com
readonly PROBE_MODES=(review write build)
readonly PROBE_TIMEOUT=15            # seconds curl waits for a host
readonly PROBE_NO_ANSWER=000         # curl's status code when no HTTP answer came
readonly PROBE_OLD_TIME=2020-01-01   # the planted file's modification time, before any run
readonly PROBE_SECRET=PROBE_CANARY   # a variable set outside, which the box must not see
readonly PROBE_TOUCHED=probe-touched
readonly PROBE_ENV=.env PROBE_DEPS=node_modules   # the git-ignored file the box must hide, and a dependency folder
readonly PROBE_PLANTED="$PROBE_DEPS/probe-planted.js"
readonly PROBE_NEW_DIR=.venv                     # an ignored folder missing before the run
readonly PROBE_NEW="$PROBE_NEW_DIR/probe-new"     # and the file the run makes in it
readonly PROBE_YES=PROBE-YES PROBE_NO=PROBE-NO     # what the box prints when a step works, or is refused
# The variables every box has: the five agent_box sets, bash's own, and srt's proxy settings.
readonly PROBE_BASE_VARS=(HOME PATH LANG TERM TMPDIR PWD SHLVL OLDPWD _)
readonly PROBE_SRT_VARS='^([A-Za-z_]*_PROXY[A-Z_]*|[a-z_]*_proxy|CLAUDE_CODE_HOST_[A-Z_]+|GIT_CONFIG_PARAMETERS|GIT_SSH_COMMAND|JAVA_TOOL_OPTIONS|SANDBOX_RUNTIME)$'

settings="${1:-$PROBE_DEFAULT_SETTINGS}"
host="${2:-$PROBE_DEFAULT_HOST}"
if [ ! -f "$settings" ]; then
  if [ -f "$PROBE_DIR/$settings" ]; then settings="$PROBE_DIR/$settings"
  else settings="$(command -v "$settings")" || { echo "probe.sh: no settings file or wrapper $1" >&2; exit 2; }; fi
fi
# shellcheck source=agent-box.sh
source "$settings"
declare -F agent_box > /dev/null || { echo "probe.sh: $settings does not set up agent_box" >&2; exit 2; }
[[ " ${BOX_DOMAINS[*]-} " == *" $host "* ]] || BOX_DOMAINS+=("$host")

pass=0 fail=0
check() {   # check <mode> <name> <true|false>
  if "$3"; then echo "PASS $1: $2"; pass=$((pass + 1)); else echo "FAIL $1: $2"; fail=$((fail + 1)); fi
}
inbox() { agent_box bash -c "$1" 2> /dev/null; }   # inbox <script>: its output; fails as the script does
# ok and refused <script>: the box ran and the script worked, or was refused. A box that did not run fails both.
said() { inbox "( $1 ) > /dev/null 2>&1 && echo $PROBE_YES || echo $PROBE_NO" || true; }
ok() { [ "$(said "$1")" = "$PROBE_YES" ] && echo true || echo false; }
refused() { [ "$(said "$1")" = "$PROBE_NO" ] && echo true || echo false; }
listed() {   # listed <path>: true when the repository's <path> is in the file BOX_CHANGED names
  [ -n "$BOX_CHANGED" ] && grep -qxF "$repo/$1" "${BOX_CHANGED##*: }" && echo true || echo false
}
under_home() {   # under_home <path...>: the first folder under the home of each path there
  local p
  for p in "$@"; do [[ "$p" != "$HOME/"* ]] || { p="${p#"$HOME/"}"; echo "${p%%/*}"; }; done
}

repo="$(mktemp -d)"
trap 'agent_box_sweep; cd /; rm -rf "$repo"' EXIT
cd "$repo"
git init -q
printf '%s\n' "$PROBE_ENV" "$PROBE_DEPS/" "$PROBE_NEW_DIR/" > .gitignore
echo "PROBE_KEY=secret" > "$PROBE_ENV"
mkdir "$PROBE_DEPS"
export "$PROBE_SECRET=1"

for mode in "${PROBE_MODES[@]}"; do
  BOX_MODE=$mode
  tools=()
  [ "$mode" != build ] || tools=(${BOX_TOOLS[@]+"${BOX_TOOLS[@]}"})
  srt="$(command -v srt)"
  allowed="$(under_home ${BOX_READ[@]+"${BOX_READ[@]}"} ${BOX_WRITE[@]+"${BOX_WRITE[@]}"} \
    ${BOX_HIDE[@]+"${BOX_HIDE[@]}"} ${BOX_SKILLS[@]+"${BOX_SKILLS[@]}"} ${tools[@]+"${tools[@]}"} \
    "$srt" "$(readlink -f "$srt")" | sort -u)"
  if listed="$(inbox 'ls -A ~')"; then
    extra="$(comm -23 <(sort -u <<< "$listed" | sed '/^$/d') <(printf '%s\n' "$allowed" | sed '/^$/d'))"
    check "$mode" "the home lists only allowed folders${extra:+ (also: $(tr '\n' ' ' <<< "$extra"))}" \
      "$([ -z "$extra" ] && echo true || echo false)"
  else check "$mode" "the home lists only allowed folders (the box did not run)" false; fi
  check "$mode" "$PROBE_ENV is refused" "$(refused "cat $PROBE_ENV")"
  check "$mode" ".git is not writable" "$(refused 'touch .git/probe')"
  if [ "$mode" = review ]; then check "$mode" "the repository is read-only" "$(refused "touch $PROBE_TOUCHED")"
  else check "$mode" "the repository is writable" "$(ok "touch $PROBE_TOUCHED")"; fi
  rm -f "$PROBE_TOUCHED"
  code="$(inbox "curl -sS -o /dev/null -w '%{http_code}' --max-time $PROBE_TIMEOUT https://$host" || true)"
  check "$mode" "$host answers (${code:-none})" "$([ -n "$code" ] && [ "$code" != "$PROBE_NO_ANSWER" ] && echo true || echo false)"
  check "$mode" "$PROBE_DENIED_URL is refused" "$(refused "curl -sS -o /dev/null --max-time $PROBE_TIMEOUT $PROBE_DENIED_URL")"
  names="$(inbox 'compgen -e' || true)"
  stray=""
  for n in $names; do
    [[ " ${PROBE_BASE_VARS[*]} ${BOX_KEEP[*]-} " == *" $n "* ]] || [[ "$n" =~ $PROBE_SRT_VARS ]] || stray+="$n "
  done
  check "$mode" "only kept variables are in the environment${stray:+ (also: $stray)}" \
    "$([ -n "$names" ] && [ -z "$stray" ] && echo true || echo false)"
  if [ "$mode" != review ]; then
    script="mkdir -p $PROBE_NEW_DIR && echo planted > $PROBE_NEW"
    [ "$mode" != build ] || script+=" && echo planted > $PROBE_PLANTED && touch -d $PROBE_OLD_TIME $PROBE_PLANTED"
    inbox "$script" > /dev/null || true
    changed="${BOX_CHANGED##*: }"
    check "$mode" "a git-ignored file the run made is in BOX_CHANGED" "$(listed "$PROBE_NEW")"
    [ "$mode" != build ] || check "$mode" "a file planted in $PROBE_DEPS with an old time is in BOX_CHANGED" \
      "$(listed "$PROBE_PLANTED")"
    [ -z "$BOX_CHANGED" ] || rm -f "$changed"
    rm -rf "$PROBE_NEW_DIR"
  fi
  if [ "$mode" = build ]; then
    bad=""
    for t in ${BOX_TOOLS[@]+"${BOX_TOOLS[@]}"}; do ! agent_box_refused "$t" || bad+="$t "; done
    check "$mode" "BOX_TOOLS holds no folder of other tools' state${bad:+ (has: $bad)}" \
      "$([ -z "$bad" ] && echo true || echo false)"
  fi
done

echo "probe.sh: $settings: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
