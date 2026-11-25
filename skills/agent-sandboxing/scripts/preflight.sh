#!/usr/bin/env bash
# preflight.sh: read-only check of the repository an agent is about to work in, for what the box will not cover there.
# Run it from inside that repository, with no arguments. Prints one line per check, OK or WARN, and exits 0 when all
# are OK, 1 when any is WARN, 2 outside a git repository. It changes nothing and fetches nothing. Checks: uncommitted
# work (the box does not protect the project folder), a remote URL with credentials in it, and key-shaped strings
# anywhere in the history (all refs and reflogs, stashes included), with gitleaks when it is installed, otherwise a
# scan of git log -p (added lines and commit messages) for KEY_PATTERNS.
# A hit is shown as commit and file only, never its value.
set -euo pipefail
export GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1   # git status must not rewrite .git/index; no fetch of missing blobs
readonly EXIT_NOT_REPO=2
readonly MSG_FILE='(commit message)'   # file shown for a hit in a commit message
readonly MAX_HITS=10     # hit lines shown before the count
readonly SHORT_ID=12     # characters of a commit id shown
readonly OK=OK WARN=WARN
readonly CRED_URL='://([^/@]+:[^/@]+|[^/@:]{20,})@'   # user:token@host or token@host in a remote URL
# Common key shapes (grep -E). Only lines a commit adds, and commit messages, are scanned.
readonly KEY_PATTERNS=(
  '-----BEGIN [A-Z ]*PRIVATE KEY-----'     # private key header
  'AKIA[0-9A-Z]{16}'                       # AWS access key id
  '(ghp|gho)_[A-Za-z0-9]{30,}'             # GitHub token
  'github_pat_[A-Za-z0-9_]{20,}'           # GitHub fine-grained token
  'sk-[A-Za-z0-9_-]{20,}'                  # OpenAI / Anthropic style key
  'xox[abprs]-[A-Za-z0-9-]{10,}'           # Slack token
  # generic: name = value, the value a whole token (not a call such as get_password())
  '(KEY|TOKEN|SECRET|PASSWORD|[Kk]ey|[Tt]oken|[Ss]ecret|[Pp]assword)["'"'"']?[[:space:]]*[:=][[:space:]]*["'"'"']?[A-Za-z0-9/+_.-]{16,}(["'"'"'[:space:],;]|$)'
)

status=0
report() {   # report OK|WARN text
  printf '%-4s %s\n' "$1" "$2"
  [ "$1" = "$OK" ] || status=1
}

# Fallback scan: prints "commit<TAB>file" for each added line or message line matching KEY_PATTERNS, once per pair.
scan_log() {
  local joined
  joined="$(IFS='|'; echo "${KEY_PATTERNS[*]}")"
  # -U0: no context lines; the file comes from "diff --git", as an added line can look like a "+++ " header.
  git log -p -U0 --all --reflog --diff-merges=first-parent --no-color --no-ext-diff --no-textconv \
    --format='commit %H%n%w(0,4,4)%B' \
    | { LC_ALL=C grep -a -E -- "^commit |^diff --git |^(\+|    ).*($joined)" || true; } \
    | LC_ALL=C awk -v n="$SHORT_ID" -v m="$MSG_FILE" '
        /^commit / { c = substr($2, 1, n); f = ""; next }
        /^diff --git / { f = $0; sub(/.* b\//, "", f); next }
        { k = c "\t" (/^    / ? m : f); if (!(k in seen)) { seen[k] = 1; print k } }'
}

# gitleaks scan: the same "commit<TAB>file" lines, from its JSON report.
scan_gitleaks() {
  local out sub=git
  out="$(mktemp)"
  trap 'rm -f "$out"' RETURN
  gitleaks git --help > /dev/null 2>&1 || sub=detect
  gitleaks "$sub" --no-banner --redact --log-opts="--all --reflog --no-textconv --no-ext-diff" --exit-code 0 \
    --report-format json --report-path "$out" > /dev/null 2>&1 || return 1
  python3 -c 'import json, sys
seen = set()
for h in json.load(open(sys.argv[1])) or []:
    k = (h.get("Commit", "")[:int(sys.argv[2])], h.get("File", ""))
    if k not in seen:
        seen.add(k); print("\t".join(k))' "$out" "$SHORT_ID" 2> /dev/null
}

if [ "$(git rev-parse --is-inside-work-tree 2> /dev/null)" != true ]; then   # false inside .git or a bare repository
  echo "preflight.sh: not a git repository: run this from the repository the agent will work in" >&2
  exit "$EXIT_NOT_REPO"
fi

# 1. Uncommitted work.
changed="$(git -c core.fsmonitor=false status --porcelain | wc -l)"   # no fsmonitor command from the config
if [ "$changed" -eq 0 ]; then
  report "$OK" "uncommitted work: none"
else
  report "$WARN" "uncommitted work: $changed changed or untracked files; the box does not protect the project folder, commit or stash first"
fi

# 2a. Credentials in a remote URL (the remote's name only, never the URL).
creds="$(git config --get-regexp '^remote\..*\.(url|pushurl)$' 2> /dev/null \
  | grep -E -- "$CRED_URL" | cut -d' ' -f1 | sed 's/^remote\.//; s/\.[a-z]*$//' | sort -u || true)"
if [ -z "$creds" ]; then
  report "$OK" "remote URLs: no credentials"
else
  report "$WARN" "remote URLs: credentials in the URL of remote $(echo "$creds" | paste -sd, -); .git is readable in the box, so they can be read there; remove them from the URL and replace the token"
fi

# 2b. Key-shaped strings in the history.
tool=gitleaks
if ! command -v gitleaks > /dev/null 2>&1 || ! hits="$(scan_gitleaks)"; then tool="git log scan"; hits="$(scan_log)"; fi
count="$(printf '%s' "$hits" | grep -c . || true)"
if [ "$count" -eq 0 ]; then
  report "$OK" "history ($tool): no key-shaped strings"
else
  report "$WARN" "history ($tool): key-shaped strings in $count commit/file pair(s); .git is readable in the box, so these can be read there; remove them from the history (for example git filter-repo) and replace the keys"
  printf '%s\n' "$hits" | sed -n "1,${MAX_HITS}s/^/     /p"
  [ "$count" -le "$MAX_HITS" ] || echo "     ... and $((count - MAX_HITS)) more"
fi
exit "$status"
