"""R2.5a — ContextBench-Lite external-corpus loader (pure mapper core).

Maps in-memory ContextBench-Lite records (list of dicts, materialised from parquet)
→ the existing ``SweepQuery`` shape so results drop into ``score_vectors`` / ``run_ab``
/ the R2.4 reporter unchanged (D21 "scorer unchanged").

**No network, no binary, no file I/O** in this module.  All I/O is confined to the thin
fetch entrypoint ``research/r1_harness/fetch_contextbench.py`` which downloads once to a
gitignored cache dir.

Dataset: HF ``Contextbench/ContextBench``, config ``contextbench_verified`` (500-task
Lite subset), Apache-2.0.  arXiv:2602.05892.  Homepage: github.com/EuniAI/ContextBench.

Field mapping (verified against real parquet schema via HF datasets-server API, 2026-06-15):

  corpus_id   = record["repo"]              e.g. "astropy/astropy"
  query_id    = record["instance_id"]        e.g. "SWE-Bench-Verified__python__..."
  query       = record["problem_statement"]  natural-language task description
  query_type  = "keyword"                    no task-type signal in ContextBench schema;
                                             default matches load_suite()
  gold_context = record["gold_context"]      JSON string (or None):
                 list of {"file": str, "start_line": int, "end_line": int, "content": str}
  gold_files  = frozenset of unique "file" values across all gold_context entries
  gold_blocks = frozenset of (file_path, symbol_name) pairs where symbol_name encodes
                the line range: "<file>::L<start_line>-L<end_line>"
                (ContextBench has no symbol names; line range is a stable proxy)

Missing / null / empty gold_context → empty frozensets (documented rule; no crash).
"""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .sweep import SweepQuery


def _parse_gold_context(
    raw: Any,
) -> tuple[frozenset[str], frozenset[tuple[str, str]]]:
    """Parse the ``gold_context`` field into (gold_files, gold_blocks).

    ``raw`` may be:
      - ``None``                        → empty frozensets
      - ``""`` or ``"[]"`` (str)        → empty frozensets
      - a JSON string with entries      → mapped frozensets
      - already a list of dicts         → mapped frozensets (parquet may materialise it)

    Each entry must have "file", "start_line", "end_line".
    symbol_name = "<file>::L<start_line>-L<end_line>"  (stable proxy).
    """
    if raw is None:
        return frozenset(), frozenset()

    if isinstance(raw, str):
        stripped = raw.strip()
        if not stripped or stripped == "null":
            return frozenset(), frozenset()
        try:
            entries = json.loads(stripped)
        except json.JSONDecodeError:
            return frozenset(), frozenset()
    elif isinstance(raw, list):
        entries = raw
    else:
        return frozenset(), frozenset()

    if not entries:
        return frozenset(), frozenset()

    files: list[str] = []
    blocks: list[tuple[str, str]] = []
    for entry in entries:
        # Guard: skip non-dict array elements (e.g. strings, ints, nulls).
        # Documented rule: malformed gold_context entries → skipped, no crash.
        if not isinstance(entry, dict):
            continue
        file_path = entry.get("file", "")
        # Guard: skip entries whose "file" value is not a str (e.g. 123).
        # Documented rule: non-string file values are skipped to preserve frozenset[str] contract.
        if not isinstance(file_path, str) or not file_path:
            continue
        start = entry.get("start_line", 0)
        end = entry.get("end_line", 0)
        symbol_name = f"{file_path}::L{start}-L{end}"
        files.append(file_path)
        blocks.append((file_path, symbol_name))

    return frozenset(files), frozenset(blocks)


def parse_contextbench_records(records: list[dict]) -> list[SweepQuery]:
    """Map a list of in-memory ContextBench-Lite records → list[SweepQuery].

    Parameters
    ----------
    records:
        Already-parsed records (list of dicts) from the ``contextbench_verified``
        parquet split.  Each dict must have at minimum: ``instance_id``, ``repo``,
        ``problem_statement``.  ``gold_context`` may be absent or None.

    Returns
    -------
    list[SweepQuery]
        One SweepQuery per record, in first-seen order (deterministic).
        ``gold_files`` and ``gold_blocks`` are frozensets.
        Records with missing/null gold_context yield empty frozensets.
    """
    # Import here (not at module level) so the module-level import surface stays
    # stdlib-only.  sweep.py is stdlib + dataclasses — no network, no binary.
    from .sweep import SweepQuery

    out: list[SweepQuery] = []
    for rec in records:
        corpus_id: str = rec.get("repo", "")
        query_id: str = rec.get("instance_id", "")
        query: str = rec.get("problem_statement", "")
        query_type: str = "keyword"  # no task-type signal in ContextBench schema
        raw_gold = rec.get("gold_context", None)
        gold_files, gold_blocks = _parse_gold_context(raw_gold)
        out.append(
            SweepQuery(
                corpus_id=corpus_id,
                query_id=query_id,
                query=query,
                query_type=query_type,
                gold_files=gold_files,
                gold_blocks=gold_blocks,
            )
        )
    return out
