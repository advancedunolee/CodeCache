"""RED tests for R2.5a — ContextBench-Lite external-corpus loader.

Pure-logic, binary-free, network-free.  Covers:

  1. happy (THE core proof): a small inline ContextBench-Lite record (faithful to
     the real parquet schema, verified from HF API) maps to a SweepQuery with
     exact, hand-specified field values.
  2. happy: multiple records → multiple SweepQuery in deterministic order;
     frozenset shapes are correct for score_vectors consumption.
  3. edge: a record with multiple gold files / multiple gold blocks → frozensets
     contain all of them (no dedup loss, no ordering dependence — frozenset equality).
  4. edge: missing/empty gold_context (None or "[]" or missing key) → handled per
     documented rule (empty frozensets) with no crash.
  5. error/hermeticity: the mapper core imports nothing that does network or binary
     I/O (import-surface assertion, mirroring R2.4 purity test).
  6. fetch-entrypoint robustness: missing cache dir → stderr instruction + nonzero
     exit (unit test with subprocess; no live network call).

The production module ``r1harness/contextbench.py`` does NOT exist yet — every
import here will fail with ImportError.  That is the correct RED state.

Public API expected (engineering lead must implement):

    from r1harness.contextbench import parse_contextbench_records

    parse_contextbench_records(records: list[dict]) -> list[SweepQuery]

Field-mapping rules (hand-derived from the real parquet schema):

  corpus_id   = record["repo"]              e.g. "astropy/astropy"
  query_id    = record["instance_id"]        e.g. "SWE-Bench-Verified__python__..."
  query       = record["problem_statement"]  the natural-language task description
  query_type  = "keyword"                    no task-type signal in ContextBench schema;
                                             default "keyword" matches load_suite()
  gold_context = record["gold_context"]      JSON string: list of
                 {"file": str, "start_line": int, "end_line": int, "content": str}
  gold_files  = frozenset of unique "file" values across all gold_context entries
  gold_blocks = frozenset of (file_path, symbol_name) pairs where symbol_name is
                derived as "<file>::L<start_line>-L<end_line>" (no symbol names in
                ContextBench; we encode line range as a stable proxy symbol)
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

# ---------------------------------------------------------------------------
# Inline fixture records (faithful to the real ContextBench parquet schema,
# verified from HF datasets-server API row 0, 2026-06-15).
# ---------------------------------------------------------------------------

RECORD_SINGLE = {
    "instance_id": "SWE-Bench-Verified__python__maintenance__bugfix__deb49033",
    "original_inst_id": "astropy__astropy-13398",
    "repo": "astropy/astropy",
    "repo_url": "https://github.com/astropy/astropy.git",
    "language": "python",
    "base_commit": "6500928dc0e57be8f06d1162eacc3ba5e2eff692",
    "gold_context": (
        '[{"file": "astropy/coordinates/attributes.py", "start_line": 344, "end_line": 396, '
        '"content": "class EarthLocationAttribute(Attribute):\\n    pass"}]'
    ),
    "patch": "diff ...",
    "test_patch": "diff ...",
    "problem_statement": ("A direct approach to ITRS to Observed transformations that stays within the ITRS."),
    "f2p": "[]",
    "p2p": "[]",
    "source": "Verified",
}

# Second record — different repo, different instance_id.
RECORD_B = {
    "instance_id": "SWE-Bench-Verified__python__maintenance__bugfix__aabbccdd",
    "original_inst_id": "django__django-99999",
    "repo": "django/django",
    "repo_url": "https://github.com/django/django.git",
    "language": "python",
    "base_commit": "deadbeef",
    "gold_context": (
        '[{"file": "django/db/models/query.py", "start_line": 10, "end_line": 20, '
        '"content": "class QuerySet:\\n    pass"}]'
    ),
    "patch": "",
    "test_patch": "",
    "problem_statement": "Fix the ORM queryset evaluation bug.",
    "f2p": "[]",
    "p2p": "[]",
    "source": "Verified",
}

# Record with multiple gold_context entries (multiple files, multiple blocks).
RECORD_MULTI = {
    "instance_id": "SWE-Bench-Verified__python__multi__aabbcc11",
    "original_inst_id": "lib__lib-1234",
    "repo": "myorg/mylib",
    "repo_url": "https://github.com/myorg/mylib.git",
    "language": "python",
    "base_commit": "abc123",
    "gold_context": (
        '[{"file": "src/foo.py", "start_line": 1, "end_line": 10, "content": "def foo(): pass"},'
        ' {"file": "src/bar.py", "start_line": 5, "end_line": 15, "content": "def bar(): pass"},'
        ' {"file": "src/foo.py", "start_line": 50, "end_line": 60, "content": "def baz(): pass"}]'
    ),
    "patch": "",
    "test_patch": "",
    "problem_statement": "Fix multiple files.",
    "f2p": "[]",
    "p2p": "[]",
    "source": "Verified",
}

# Record with null / empty gold_context.
RECORD_NO_GOLD_NULL = {
    "instance_id": "SWE-Bench-Verified__python__no_gold__00000000",
    "original_inst_id": "repo__repo-0",
    "repo": "someorg/somerepo",
    "repo_url": "https://github.com/someorg/somerepo.git",
    "language": "python",
    "base_commit": "cafebabe",
    "gold_context": None,
    "patch": "",
    "test_patch": "",
    "problem_statement": "A task with no gold context.",
    "f2p": "[]",
    "p2p": "[]",
    "source": "Verified",
}

RECORD_NO_GOLD_EMPTY_LIST = dict(RECORD_NO_GOLD_NULL, gold_context="[]", instance_id="id-empty-list")
RECORD_NO_GOLD_MISSING_KEY = {k: v for k, v in RECORD_NO_GOLD_NULL.items() if k != "gold_context"}
RECORD_NO_GOLD_MISSING_KEY["instance_id"] = "id-missing-key"

# ---------------------------------------------------------------------------
# Production import (will fail ImportError — correct RED state)
# ---------------------------------------------------------------------------
from r1harness.contextbench import parse_contextbench_records  # type: ignore[import]  # noqa: E402


# ---------------------------------------------------------------------------
# 1. Happy — single record, exact field values (THE core proof)
# ---------------------------------------------------------------------------


def test_single_record_maps_to_sweep_query():
    """A faithful ContextBench-Lite record maps to a SweepQuery with exact field values."""
    from r1harness.sweep import SweepQuery

    results = parse_contextbench_records([RECORD_SINGLE])

    assert len(results) == 1
    sq = results[0]
    assert isinstance(sq, SweepQuery)

    # corpus_id = record["repo"]
    assert sq.corpus_id == "astropy/astropy"

    # query_id = record["instance_id"]
    assert sq.query_id == "SWE-Bench-Verified__python__maintenance__bugfix__deb49033"

    # query = record["problem_statement"]
    assert sq.query == ("A direct approach to ITRS to Observed transformations that stays within the ITRS.")

    # query_type defaults to "keyword" (no signal in ContextBench schema)
    assert sq.query_type == "keyword"

    # gold_files: frozenset of file paths
    assert sq.gold_files == frozenset({"astropy/coordinates/attributes.py"})

    # gold_blocks: frozenset of (file_path, symbol_name) pairs.
    # symbol_name = "<file>::L<start>-L<end>" (stable proxy; no real symbol names in CB)
    assert sq.gold_blocks == frozenset(
        {("astropy/coordinates/attributes.py", "astropy/coordinates/attributes.py::L344-L396")}
    )


# ---------------------------------------------------------------------------
# 2. Happy — multiple records, deterministic order, frozenset shapes correct
# ---------------------------------------------------------------------------


def test_multiple_records_deterministic_order():
    """Multiple records → multiple SweepQuery in first-seen order."""
    from r1harness.sweep import SweepQuery

    results = parse_contextbench_records([RECORD_SINGLE, RECORD_B])

    assert len(results) == 2
    assert all(isinstance(sq, SweepQuery) for sq in results)

    # First record stays first (deterministic first-seen order).
    assert results[0].query_id == "SWE-Bench-Verified__python__maintenance__bugfix__deb49033"
    assert results[1].query_id == "SWE-Bench-Verified__python__maintenance__bugfix__aabbccdd"

    # Both have frozenset types.
    for sq in results:
        assert isinstance(sq.gold_files, frozenset)
        assert isinstance(sq.gold_blocks, frozenset)
        for pair in sq.gold_blocks:
            assert isinstance(pair, tuple) and len(pair) == 2


def test_multiple_records_consumable_by_score_vectors():
    """Results are directly consumable by score_vectors-shaped code (frozenset + tuple shapes)."""
    results = parse_contextbench_records([RECORD_SINGLE, RECORD_B])
    for sq in results:
        # score_vectors iterates gold_blocks as (file_path, symbol_name) pairs.
        for file_path, symbol_name in sq.gold_blocks:
            assert isinstance(file_path, str)
            assert isinstance(symbol_name, str)
        for f in sq.gold_files:
            assert isinstance(f, str)


# ---------------------------------------------------------------------------
# 3. Edge — multiple gold files + multiple gold blocks in one record
# ---------------------------------------------------------------------------


def test_multi_gold_record_all_files_and_blocks_captured():
    """A record with 3 gold_context entries across 2 files → all captured; frozenset equality."""
    results = parse_contextbench_records([RECORD_MULTI])

    assert len(results) == 1
    sq = results[0]

    # Two distinct files.
    assert sq.gold_files == frozenset({"src/foo.py", "src/bar.py"})

    # Three blocks (2 from foo.py, 1 from bar.py); frozenset equality is order-independent.
    expected_blocks = frozenset(
        {
            ("src/foo.py", "src/foo.py::L1-L10"),
            ("src/bar.py", "src/bar.py::L5-L15"),
            ("src/foo.py", "src/foo.py::L50-L60"),
        }
    )
    assert sq.gold_blocks == expected_blocks


# ---------------------------------------------------------------------------
# 4. Edge — missing / empty gold_context → empty frozensets, no crash
# ---------------------------------------------------------------------------


def test_null_gold_context_yields_empty_frozensets():
    """gold_context=None → empty gold_files and gold_blocks; no crash."""
    results = parse_contextbench_records([RECORD_NO_GOLD_NULL])
    assert len(results) == 1
    sq = results[0]
    assert sq.gold_files == frozenset()
    assert sq.gold_blocks == frozenset()


def test_empty_list_gold_context_yields_empty_frozensets():
    """gold_context='[]' → empty frozensets."""
    results = parse_contextbench_records([RECORD_NO_GOLD_EMPTY_LIST])
    assert len(results) == 1
    sq = results[0]
    assert sq.gold_files == frozenset()
    assert sq.gold_blocks == frozenset()


def test_missing_gold_context_key_yields_empty_frozensets():
    """gold_context key absent → empty frozensets; no KeyError crash."""
    results = parse_contextbench_records([RECORD_NO_GOLD_MISSING_KEY])
    assert len(results) == 1
    sq = results[0]
    assert sq.gold_files == frozenset()
    assert sq.gold_blocks == frozenset()


def test_empty_records_list_returns_empty():
    """parse_contextbench_records([]) → []."""
    assert parse_contextbench_records([]) == []


# ---------------------------------------------------------------------------
# 5. Hermeticity — mapper core imports no network/binary/file I/O
# ---------------------------------------------------------------------------


def test_mapper_core_import_surface_is_pure():
    """r1harness.contextbench imports only stdlib + r1harness; no datasets, no subprocess."""
    import r1harness.contextbench as cb_mod

    # Collect transitive imports by inspecting the module's globals.
    # We don't walk the whole import graph (too expensive); instead assert
    # that the heavy HF libs are NOT present in the module's own namespace.
    disallowed = {"datasets", "huggingface_hub", "subprocess", "urllib", "requests", "socket"}
    module_globals = set(cb_mod.__dict__.keys())
    overlap = disallowed & module_globals
    assert overlap == set(), f"Mapper module leaks disallowed imports into its namespace: {overlap}"


def test_mapper_module_does_not_import_datasets_at_module_level():
    """Running 'import r1harness.contextbench' in a fresh interpreter must not import datasets."""
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            (
                "import sys; "
                "import r1harness.contextbench; "
                "assert 'datasets' not in sys.modules, "
                "'datasets was imported at module level in contextbench.py'"
            ),
        ],
        capture_output=True,
        text=True,
        cwd=str(Path(__file__).resolve().parents[1]),
    )
    assert result.returncode == 0, f"Purity check failed:\nstdout={result.stdout}\nstderr={result.stderr}"


# ---------------------------------------------------------------------------
# 6. Fetch-entrypoint robustness — missing cache → stderr + nonzero exit
# ---------------------------------------------------------------------------


def test_fetch_entrypoint_missing_cache_exits_nonzero(tmp_path):
    """fetch_contextbench.py default (no --force) with a missing cache → nonzero exit + cache-not-found
    instruction text.  The default path must NOT attempt a download; it must print the cache-not-found
    instruction and exit nonzero without importing or calling datasets.

    The fix: main() without --force + missing cache must instruct-and-exit (mirror run_report.py
    precedent).  Only --force triggers fetch_and_cache().
    """
    fetch_script = Path(__file__).resolve().parents[1] / "fetch_contextbench.py"
    if not fetch_script.exists():
        pytest.skip("fetch_contextbench.py not yet written (GREEN step)")

    result = subprocess.run(
        [
            sys.executable,
            str(fetch_script),
            "--cache-dir",
            str(tmp_path / "nonexistent_cache"),
        ],
        capture_output=True,
        text=True,
        env={**__import__("os").environ, "CONTEXTBENCH_CACHE": str(tmp_path / "nonexistent_cache")},
    )
    # Must exit nonzero.
    assert result.returncode != 0, "fetch_contextbench.py should exit nonzero when cache is missing"
    # Must print the cache-not-found instruction — tightened assertion requires the script name.
    combined_err = result.stderr + result.stdout
    assert "fetch_contextbench.py" in combined_err, (
        f"Cache-not-found instruction must mention 'fetch_contextbench.py' but got:\n{combined_err}"
    )


def test_fetch_entrypoint_missing_cache_never_calls_load_dataset(tmp_path):
    """The default missing-cache path (no --force) must NEVER import or call datasets.load_dataset.

    Proves hermetic no-download behaviour even when datasets IS importable.
    We inject a poisoned datasets stub on PYTHONPATH that raises RuntimeError
    if load_dataset is called; assert the default (no --force) path exits without triggering it.
    """
    import os
    import textwrap

    fetch_script = Path(__file__).resolve().parents[1] / "fetch_contextbench.py"
    if not fetch_script.exists():
        pytest.skip("fetch_contextbench.py not yet written (GREEN step)")

    # Write a poisoned datasets stub into tmp_path.
    stub_pkg = tmp_path / "datasets"
    stub_pkg.mkdir()
    (stub_pkg / "__init__.py").write_text(
        textwrap.dedent("""
            def load_dataset(*args, **kwargs):
                raise RuntimeError(
                    "POISON: datasets.load_dataset was called on the missing-cache read path"
                )
        """),
        encoding="utf-8",
    )

    # Prepend the stub dir to PYTHONPATH so 'import datasets' finds our poison.
    env = dict(os.environ)
    existing_pp = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = str(tmp_path) + (os.pathsep + existing_pp if existing_pp else "")

    result = subprocess.run(
        [
            sys.executable,
            str(fetch_script),
            "--cache-dir",
            str(tmp_path / "nonexistent_cache"),
        ],
        capture_output=True,
        text=True,
        env=env,
    )

    # Must exit nonzero (cache missing).
    assert result.returncode != 0, (
        f"Expected nonzero exit when cache missing; got 0.\nstdout={result.stdout}\nstderr={result.stderr}"
    )
    # Must NOT have triggered the poison.
    combined = result.stdout + result.stderr
    assert "POISON" not in combined, f"datasets.load_dataset was called on the missing-cache read path!\n{combined}"


# ---------------------------------------------------------------------------
# 7. Edge — non-dict entries in gold_context array → empty frozensets, no crash
# ---------------------------------------------------------------------------


def test_nondict_gold_context_entries_yield_empty_frozensets():
    """gold_context is a valid JSON array but elements are non-dicts (e.g. strings).

    Documented contract: malformed gold_context → empty frozensets, no crash.
    Must NOT raise AttributeError when entry.get() is called on a str.
    """
    record = dict(RECORD_NO_GOLD_NULL, gold_context='["juststring", 42, null]', instance_id="id-nondict")
    results = parse_contextbench_records([record])
    assert len(results) == 1
    sq = results[0]
    assert sq.gold_files == frozenset(), "Expected empty gold_files for non-dict entries"
    assert sq.gold_blocks == frozenset(), "Expected empty gold_blocks for non-dict entries"


def test_nonstring_file_value_skipped_or_coerced():
    """A non-string 'file' value (e.g. 123) must not produce a non-str member in gold_files.

    The frozenset[str] contract must hold.  Implementation MUST either skip the entry or
    coerce to str — whichever it does, it must document it and must not crash.
    """
    record = dict(
        RECORD_NO_GOLD_NULL,
        gold_context='[{"file": 123, "start_line": 1, "end_line": 5, "content": "x"}]',
        instance_id="id-nonstring-file",
    )
    results = parse_contextbench_records([record])
    assert len(results) == 1
    sq = results[0]
    # All members of gold_files must be str.
    for f in sq.gold_files:
        assert isinstance(f, str), f"gold_files member is not str: {f!r}"
    # All members of gold_blocks must be (str, str) tuples.
    for file_path, symbol_name in sq.gold_blocks:
        assert isinstance(file_path, str), f"gold_blocks file_path is not str: {file_path!r}"
        assert isinstance(symbol_name, str), f"gold_blocks symbol_name is not str: {symbol_name!r}"
