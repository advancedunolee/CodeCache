"""Map an agent action + its observation to the gold-comparable items it surfaced.

This is the measurement bridge between *what the agent did* and *what entered its
context* — the substrate for Layer-1 scoring (project_overview §5.2). Three cases:

1. **codecache query --format json** (arm A1): the §6.4.2 JSON is authoritative —
   parse it for the exact ranked ``(file, symbol)`` blocks and files.
2. **grep-style output** (``path:line:content``): each line names a file; if the
   matched line is a ``def``/``class`` declaration, attribute that symbol to that
   file precisely.
3. **cat/sed/head of a file**: the file argument(s) that exist in the repo are
   surfaced; ``def``/``class`` names found in the shown text are attributed to them.

This is a **v1 heuristic** (the open measurement-design item flagged in the brief):
A1's JSON path is exact; the grep/cat extraction for A0 is a reasonable best-effort
and is deliberately conservative (it only credits files actually referenced and
symbols actually shown). It is documented so R2/R3 can refine it without surprise.
"""

from __future__ import annotations

import re
from pathlib import Path

from .codecache_tool import parse_query_json

#: A grep ``path:line:content`` whose content declares a def/class → precise block.
_GREP_DEF_RE = re.compile(r"^([^\s:]+):\d+:\s*(?:async\s+)?(?:def|class)\s+([A-Za-z_]\w*)", re.MULTILINE)
#: A def/class declaration anywhere in shown text (for cat/sed of a single file).
_DEF_RE = re.compile(r"^\s*(?:async\s+)?(?:def|class)\s+([A-Za-z_]\w*)", re.MULTILINE)


def is_codecache_query(command: str) -> bool:
    c = command.lower()
    return "codecache" in c and " query" in c and "--format" in c and "json" in c


def _dedup(seq):
    seen, out = set(), []
    for x in seq:
        if x not in seen:
            seen.add(x)
            out.append(x)
    return out


def extract_surfaced(
    command: str,
    observation: str,
    repo_files: set[str],
    repo_dir: Path | None = None,
) -> tuple[list[str], list[tuple[str, str]]]:
    """Return ``(files, blocks)`` surfaced by one action, gold-comparable.

    ``repo_files`` is the set of repo-relative posix paths in the materialised
    corpus (used to credit ``cat <file>`` arguments). ``observation`` is the
    combined stdout/stderr the agent saw.
    """
    cmd = command.replace("\\", "/")

    # 1) codecache query JSON — authoritative.
    if is_codecache_query(command):
        try:
            qr = parse_query_json(observation, "", repo_dir=repo_dir)
            return _dedup(qr.files), list(dict.fromkeys(qr.blocks))
        except Exception:
            pass  # fall through to generic extraction if output wasn't clean JSON

    files: list[str] = []
    blocks: list[tuple[str, str]] = []

    # 2) grep-style `path:line:` lines — precise file + (file, symbol) on def/class lines.
    for m in _GREP_DEF_RE.finditer(observation):
        path = m.group(1).replace("\\", "/")
        blocks.append((path, m.group(2)))
        if path not in files:
            files.append(path)

    # 3) cat/sed/head of a specific file: credit repo files named in the command,
    #    then attribute def/class names found in the shown text to them.
    cmd_files = [f for f in repo_files if f in cmd]
    shown_syms = _DEF_RE.findall(observation)
    for f in cmd_files:
        if f not in files:
            files.append(f)
        for s in shown_syms:
            blocks.append((f, s))

    return _dedup(files), list(dict.fromkeys(blocks))
