#!/usr/bin/env bash
# preflight.sh: read-only check of the repository an agent is about to work in, for what the box will not cover there.
# Run it from inside that repository, with no arguments. Prints one line per check, OK or WARN, and exits 0 when all
# are OK, 1 when any is WARN, 2 outside a git repository. It changes nothing, fetches nothing and obeys no file the
# repository ships. Checks: uncommitted work (the box does not protect the project folder), tokens in the local git
# config (a remote URL, an http header), and key-shaped strings in any object in .git (every commit, file version, tag,
# stash, staged file and unreachable object), read once by preflight_scan.py beside it, which needs python3. A hit is
# shown as where it is (commit and file, a message, an object id), never its value. Not found: a key in a Git LFS file,
# in a pack file with no index, in another worktree's config.worktree, or only in a file name.
set -euo pipefail
export GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1   # git status must not rewrite .git/index; no fetch of missing blobs
readonly EXIT_NOT_REPO=2
readonly MAX_HITS=10     # hit lines shown before the count
readonly OK=OK WARN=WARN
SCAN="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/preflight_scan.py"
readonly SCAN

status=0
report() {   # report OK|WARN text
  printf '%-4s %s\n' "$1" "$2"
  [ "$1" = "$OK" ] || status=1
}

if [ "$(git rev-parse --is-inside-work-tree 2> /dev/null)" != true ]; then   # false inside .git or a bare repository
  echo "preflight.sh: not a git repository: run this from the repository the agent will work in" >&2
  exit "$EXIT_NOT_REPO"
fi

# 1. Uncommitted work. git status runs a clean or process filter the repository's own config defines (a copied .git can
# hold one), so then it is not run; nor is fsmonitor, nor any submodule's.
if ! config="$(git config --list --show-scope --includes 2> /dev/null)"; then
  report "$WARN" "uncommitted work: not checked, as git could not read the repository's config"
elif grep -q -E "^(local|worktree)"$'\t'"filter\." <<< "$config"; then
  report "$WARN" "uncommitted work: not checked, as the repository's git config defines a filter that git status would run; check the config, and commit or stash first"
elif ! changed="$(git -c core.fsmonitor=false status --porcelain --ignore-submodules=all 2> /dev/null)"; then
  report "$WARN" "uncommitted work: not checked, as git status failed"
elif [ -z "$changed" ]; then
  report "$OK" "uncommitted work: none"
else
  report "$WARN" "uncommitted work: $(grep -c . <<< "$changed") changed or untracked files; the box does not protect the project folder, commit or stash first"
fi

if ! command -v python3 > /dev/null 2>&1 || [ ! -f "$SCAN" ]; then
  report "$WARN" "git config and history: not checked, as python3 or $SCAN is missing"
  exit "$status"
fi

# 2. Tokens in the local git config (the entry's section and name only, never its value).
rc=0
names="$(python3 -I "$SCAN" config)" || rc=$?
[ "$rc" -eq 0 ] || report "$WARN" "git config: not all of .git could be read, so a token may be missing from this check"
if [ -n "$names" ]; then
  report "$WARN" "git config: a token in $names; .git is readable in the box, so it can be read there; remove it and replace the token"
elif [ "$rc" -eq 0 ]; then
  report "$OK" "git config: no tokens"
fi

# 3. Key-shaped strings in any object in .git.
rc=0
hits="$(python3 -I "$SCAN" objects)" || rc=$?
count="$(grep -c . <<< "$hits" || true)"
[ "$rc" -eq 0 ] || report "$WARN" "history: not all of .git could be read, so keys may be missing from this check"
if [ "$count" -gt 0 ]; then
  report "$WARN" "history: key-shaped strings in $count place(s) in .git; .git is readable in the box, so these can be read there; remove them from the history (for example git filter-repo), drop unreachable objects (git reflog expire --expire=now --all, then git gc --prune=now), and replace the keys"
  printf '%s\n' "$hits" | sed -n "1,${MAX_HITS}s/^/     /p"
  [ "$count" -le "$MAX_HITS" ] || echo "     ... and $((count - MAX_HITS)) more"
elif [ "$rc" -eq 0 ]; then
  report "$OK" "history: no key-shaped strings in any object in .git"
fi
exit "$status"
