#!/usr/bin/env python3
"""preflight_scan: where a git repository's .git holds key-shaped strings, never what they are.

Run by preflight.sh from inside the repository, as python3 -I. It reads the repository's git folder and every
git folder inside it (each submodule's). Two modes:
  objects  reads every blob, commit and tag object once: unreachable, staged, binary and tagged ones too. Prints one
           line per place, "<id>\t<where>": a commit and the file (its oldest commit with a key there), a commit's
           message, a tag's message, a staged file, or an object in no commit.
  config   reads the repository's own git config (local and worktree, includes followed) and prints the section and
           name of each entry whose name or value holds a token, never the subsection (a URL there can hold one).
A submodule's lines start with its folder under .git ("modules/x:"). Exits 0 when it read everything,
EXIT_GIT_FAILED when a git command failed or an object was cut short; the lines printed until then still stand.
It runs no code from the repository and writes nothing.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from typing import IO

SHORT_ID = 12  # characters of an object id shown
CHUNK = 1 << 20  # bytes of an object read at a time
OVERLAP = 4096  # bytes kept from the chunk before, so a key across two chunks still matches
EXIT_USAGE = 2
EXIT_GIT_FAILED = 3
SCANNED = {b"blob", b"commit", b"tag"}
REPO_SCOPES = {b"local", b"worktree"}  # config the repository's own folder holds
STAGED, NOWHERE = "(staged)", "(in no commit)"
WHERE = {b"commit": "(commit message)", b"tag": "(tag message)"}  # where a hit in an object of this type is
GIT_DATA = {"objects", "refs", "logs", "lfs"}  # folders of a git folder that hold no other git folder
# Every git command: no fsmonitor, pager, textconv, external diff or signature check from the repository's config.
GIT = ["git", "-c", "core.fsmonitor=false", "--no-pager"]
LOG = ["log", "--all", "--reflog", "--root", "--raw", "--no-abbrev", "--no-renames", "--no-textconv",
       "--no-ext-diff", "--no-show-signature", "--diff-merges=first-parent", "--format=commit %H"]

CRED_URL = re.compile(rb"://(?:[^/@\s:]+:[^/@\s]+|[^/@:\s]{20,})@")  # user:token@host or token@host
AUTH_HEADER = re.compile(rb"(?i:authorization)[ \t]*[:=]")  # an http.extraheader credential
# Common key shapes, as bytes, so binary objects are read as they are. Each starts with a fixed string, which keeps
# the search fast (2 GB of objects in about 25 s).
KEY_PATTERNS = (CRED_URL, *(re.compile(p) for p in (
    rb"-----BEGIN [A-Z ]*PRIVATE KEY-----",  # private key header
    rb"AKIA[0-9A-Z]{16}",  # AWS access key id
    rb"gh[pousr]_[A-Za-z0-9]{30,}",  # GitHub token
    rb"github_pat_[A-Za-z0-9_]{20,}",  # GitHub fine-grained token
    rb"sk-(?<![A-Za-z0-9_-]sk-)[A-Za-z0-9_-]{20,}",  # OpenAI, Anthropic, OpenRouter (sk-or-), OrcaRouter (sk-orca-), OpenCode; at a word start (not task-...)
    rb"xox[abprs]-[A-Za-z0-9-]{10,}",  # Slack token
    rb"glpat-[A-Za-z0-9_-]{20,}",  # GitLab token
    rb"hf_(?<![A-Za-z0-9_]hf_)[A-Za-z0-9]{30,}",  # Hugging Face token
    rb"AIza[0-9A-Za-z_-]{35}",  # Google API key
    rb"k_live_(?<=[rs]k_live_)[0-9A-Za-z]{20,}",  # Stripe live key (sk_live_, rk_live_)
    rb"npm_(?<![A-Za-z0-9_]npm_)[A-Za-z0-9]{36}",  # npm token
)))
# name = value, the value a whole token (not a call such as get_password()): only in data holding one of NAMES, the
# value is found first, then a name ending where the blanks and quotes before it end, within NAME_REACH bytes.
NAMES = tuple(case(word) for word in (b"key", b"token", b"secret", b"password")
              for case in (bytes.upper, bytes.title, bytes.lower))
VALUE = re.compile(rb"[:=][ \t]*[\"']?[A-Za-z0-9/+_.-]{16,}(?:[\"'\s,;]|$)", re.MULTILINE)
NAME_REACH = 256
QUOTES_AND_BLANKS = b" \t\"'"


def key_in(data: bytes) -> bool:
    """True when data holds a key shape."""
    if any(rx.search(data) for rx in KEY_PATTERNS):
        return True
    return any(name in data for name in NAMES) and any(
        data[max(0, m.start() - NAME_REACH):m.start()].rstrip(QUOTES_AND_BLANKS).endswith(NAMES)
        for m in VALUE.finditer(data))


class GitFailed(Exception):
    """A git command failed, or an object was shorter than git said."""


def git(where: list[str], *args: str) -> bytes:
    """The output of a git command in a git folder (where: [] for this repository); GitFailed when it fails."""
    done = subprocess.run([*GIT, *where, *args], capture_output=True, check=False)
    if done.returncode != 0:
        raise GitFailed
    return done.stdout


def git_dirs() -> list[tuple[str, list[str]]]:
    """(label, git options) for this repository and every git folder inside its .git: each submodule's, wherever it
    is (.git/modules, a linked worktree's modules, a submodule's own)."""
    common = git([], "rev-parse", "--path-format=absolute", "--git-common-dir").decode().strip()
    found: list[tuple[str, list[str]]] = [("", [])]
    for root, dirs, files in os.walk(common):
        is_git = "HEAD" in files and "objects" in dirs and "refs" in dirs
        if is_git and root != common:
            found.append((os.path.relpath(root, common) + ":", ["--git-dir", root]))
        dirs[:] = sorted(d for d in dirs if not (is_git and d in GIT_DATA))
    return found


def has_key(out: IO[bytes], size: int, scan: bool) -> bool:
    """Reads one object of size bytes from a git cat-file --batch stream; true when scanned and it holds a key."""
    found, tail = False, b""
    while size > 0:
        chunk = out.read(min(CHUNK, size))
        if not chunk:
            raise GitFailed
        size -= len(chunk)
        found = found or (scan and key_in(tail + chunk))
        tail = chunk[-OVERLAP:]
    if out.read(1) != b"\n":
        raise GitFailed
    return found


def object_hits(where: list[str]) -> tuple[list[tuple[bytes, bytes]], bool]:
    """The (id, type) of every scanned object holding a key shape, and whether every object was read."""
    hits: list[tuple[bytes, bytes]] = []
    with subprocess.Popen([*GIT, *where, "cat-file", "--batch-all-objects", "--unordered", "--batch"],
                          stdout=subprocess.PIPE, bufsize=CHUNK) as cat:
        assert cat.stdout is not None
        try:
            while header := cat.stdout.readline():
                oid, kind, size = header.split()
                if has_key(cat.stdout, int(size), kind in SCANNED):
                    hits.append((oid, kind))
        except (GitFailed, ValueError):
            cat.kill()
            return hits, False
    return hits, cat.returncode == 0


def blob_places(where: list[str], wanted: set[bytes]) -> tuple[dict[str, str], set[bytes]]:
    """Every path a wanted blob has had, with its oldest commit there (STAGED when only staged), and the blobs found."""
    files: dict[str, str] = {}
    placed: set[bytes] = set()
    for line in git(where, "ls-files", "-s").splitlines():
        info, path = line.split(b"\t", 1)
        if (oid := info.split()[1]) in wanted:
            files[path.decode(errors="replace")] = STAGED
            placed.add(oid)
    commit = ""
    for line in git(where, *LOG).splitlines():  # newest first, so the last commit seen for a path is its oldest
        if line.startswith(b"commit "):
            commit = line[7:].decode()
        elif line.startswith(b":") and b"\t" in line:
            info, path = line.split(b"\t", 1)
            if (oid := info.split()[3]) in wanted:
                files[path.decode(errors="replace")] = commit
                placed.add(oid)
    return files, placed


def objects(label: str, where: list[str]) -> bool:
    """Prints where each hit is, one line per file, message, tag or loose object; true when all was read."""
    hits, complete = object_hits(where)
    blobs = {oid for oid, kind in hits if kind == b"blob"}
    files, placed = blob_places(where, blobs) if blobs else ({}, set())
    found = {f"{oid.decode()[:SHORT_ID]}\t{label}{WHERE.get(kind, NOWHERE)}" for oid, kind in hits if oid not in placed}
    found.update(f"{commit[:SHORT_ID]}\t{label}{path}" for path, commit in files.items())
    for line in sorted(found):
        print(line)
    return complete


def config(label: str, where: list[str]) -> set[str]:
    """section.name of each entry of the repository's own config whose name or value holds a token."""
    names = set()
    fields = git(where, "config", "--list", "--show-scope", "--includes", "-z").split(b"\0")
    for scope, entry in zip(fields[::2], fields[1::2], strict=False):
        if scope in REPO_SCOPES and (key_in(entry.replace(b"\n", b"=", 1)) or AUTH_HEADER.search(entry)):
            key = entry.split(b"\n", 1)[0].decode(errors="replace")
            names.add(f"{label}{key.split('.', 1)[0]}.{key.rsplit('.', 1)[-1]}")
    return names


def main() -> int:
    if sys.argv[1:] not in (["config"], ["objects"]):
        print("usage: preflight_scan.py objects|config", file=sys.stderr)
        return EXIT_USAGE
    complete, names = True, set()
    try:
        for label, where in git_dirs():
            if sys.argv[1] == "config":
                names |= config(label, where)
            else:
                complete = objects(label, where) and complete
    except GitFailed:
        return EXIT_GIT_FAILED
    if names:
        print(", ".join(sorted(names)))
    return 0 if complete else EXIT_GIT_FAILED


if __name__ == "__main__":
    sys.exit(main())
