# BRIEF — R2.5 / external-corpus loader (R2.5a build now · R2.5b plan)

- **Milestone:** R2 offline ablations (D23) — gated slice **R2.5 external-corpus loader** (D26 ratified the gates)  ·  **Module(s):** `research/r1_harness/` only (Python). **Pure `research/`.**
- **Owner (manager):** principal-engineering-manager  ·  **Implementer:** research-harness-engineer  ·  **Created:** 2026-06-15
- **Status:** RED ✅  GREEN ✅  REVIEW ✅  DONE ✅ (R2.5a; R2.5b scoped, deferred)
- **Links:** docs/ROADMAP.md "Research track" (R2.5, **D26**) · docs/TODO.md "Research track" · project_overview.md §5–§7 · prior: BRIEF-R2.4-ablation-reporter.md, BRIEF-R2.3b-stub-chunker-ab-plumbing.md
- **Prior commits:** R2.4 reporter `f6ff03c` · R2.3a ingest seam `fc28fdd` · R2.2b sweep `292b0e4`.

## Ratified gate (D26 — record only; do not re-litigate)
- **Corpus = BOTH.** **ContextBench-Lite** (Apache-2.0) for the real-corpus ablation table; **CodeRAG-Bench RepoEval (function slice)** (CC-BY-SA 4.0) for the published-BM25 reproduction (R2.7).
- **Network/HF = AUTHORIZED** for the research harness only: add `datasets`/`huggingface_hub`; a **one-time, cached, no-auth-token** download; **zero paid spend**; the **product stays air-gapped**. The ~$1K R3 spend is **NOT** authorized.
- **Licensing housekeeping (HARD):** **download-and-cache locally (gitignored); do NOT vendor** the corpus data into the tracked tree (CC-BY-SA share-alike — keep it out of git). Provenance/attribution notes in `research/CLAUDE.md` (or a NOTICE).

## Goal
**R2.5a (BUILD NOW):** add the HF deps + a loader framework, and ship the **ContextBench-Lite loader** — a pure-logic, binary-free, network-free mapper (unit-tested against a tiny inline/fixture sample) that maps ContextBench-Lite gold → the existing **`SweepQuery`** shape so it drops into `score_vectors` / `run_ab` / the R2.4 reporter **unchanged** (D21 "scorer unchanged"). Confine all network I/O to a thin **fetch entrypoint** that downloads once → pins a **cached slice** under a **gitignored** cache dir; tests run against cached/fixture data, **never re-download**.

## Scope (in / out)
**R2.5a — IN (this pass):**
- Pin `datasets` + `huggingface_hub` in `research/r1_harness/requirements.txt` (exact `==` versions; record venv-vs-system choice in `research/CLAUDE.md`).
- A **pure mapper** in `r1harness/` (e.g. `r1harness/contextbench.py`): `parse_contextbench_records(records) -> list[SweepQuery]` (or similar) — takes already-parsed in-memory records (list of dicts / parquet rows materialised to dicts), **no network, no binary, no file I/O in the core**. Maps ContextBench-Lite gold (file/block/line annotations) → `SweepQuery(corpus_id, query_id, query, query_type, gold_files: frozenset[str], gold_blocks: frozenset[(file_path, symbol_name)])`.
- A thin **fetch entrypoint** (e.g. `fetch_contextbench.py` mirroring `run_sweep.py`/`run_ab.py`) that: downloads the `contextbench_verified` (Lite, 500-task) config once via `datasets`/`huggingface_hub`, **no auth token**, writes a **pinned cached slice** under a **gitignored** cache dir (a small deterministic subset is fine — the point is to prove the path, not run the full 500 yet). Missing-cache → clear stderr instruction to run the fetch first + clean nonzero exit (the `run_report.py` precedent), **never auto-download inside tests**.
- Extend `research/r1_harness/.gitignore` with the new cache dir pattern (allowed `research/` housekeeping).
- **Prove the end-to-end real-corpus path:** map a real ContextBench-Lite record (or a faithful fixture sample) into a `SweepQuery` and show it in the GREEN section (sample mapped record) — this is the deliverable evidence the real-corpus path works.

**R2.5a — OUT (scope errors → STOP and escalate to manager):**
- **No CodeRAG-Bench / RepoEval loader** — that is R2.5b (separate slice). Leave it scoped in this brief + TODO only.
- **No Rust `src/`, `Cargo.toml`, `Cargo.lock`, or Rust `tests/*.rs` changes.** If a crate change seems needed, that's a scope error — STOP, escalate.
- **Do NOT touch `.claude/settings.json`** — modified-but-unstaged (cargo hooks disabled by the user); leave it unstaged and OUT of the commit (explicit pathspec).
- **Do NOT commit downloaded corpus blobs** or any large cached data — gitignored.
- No astchunk (R2.6), no LLM/agent/paid spend (R3), no baseline-reproduction run (R2.7).
- No full 500-task scoring run as a *gate* — R2.5a proves the loader path; the full ablation run is R2.5b/R2.7.

**R2.5b — PLAN (NEXT slice, do NOT build now):**
- The **CodeRAG-Bench RepoEval (function slice) BEIR loader**: map `corpus.jsonl` / `queries.jsonl` + qrels → `SweepQuery` gold (`gold_files`/`gold_blocks`), scorer unchanged (D21).
- **First build step of R2.5b: confirm the exact CodeRAG-Bench LICENSE file** (expected CC-BY-SA 4.0 per paper/release; the GitHub-API read 504'd through the env proxy). Record the confirmed license in `research/CLAUDE.md` before mapping any data.
- This is the corpus **R2.7** reproduces the published BM25 NDCG@10 against (±0.03, D23). RepoEval/RepoCoder underlying data is MIT.

## Verified external facts (cite; do NOT re-derive)
- **ContextBench:** `github.com/EuniAI/ContextBench`, arXiv:2602.05892, **Apache-2.0** (confirmed). HF dataset `Contextbench/ContextBench`, **parquet**; verified/"Lite" config `contextbench_verified` = **500 tasks** (full = 1,136). Human-annotated gold contexts (file/block/line). **No static BM25 baseline** — Layer-1 gold corpus ONLY (our scorer over its static gold), NOT number-reproduction.
- **CodeRAG-Bench:** `github.com/code-rag-bench/code-rag-bench`, arXiv:2406.14497 (NAACL'25). Data license **CC-BY-SA 4.0** (per paper/release — **confirm the LICENSE file as the first R2.5b step**). **NDCG@10** primary; **RepoEval function** split is the reported one; BEIR format on HF. RepoEval/RepoCoder underlying data **MIT**.
- **Environment (probed this turn):** PyPI + huggingface.co reachable (HTTP 200) from the build/bash env; `datasets`/`huggingface_hub` NOT yet installed; Python = system `/usr/bin/python3` 3.12.3 (Windows runs used a `C:\ccr1` venv — engineer decides venv-vs-system, records it).

## Data shapes (verified — do NOT rediscover)
- **`SweepQuery`** (`r1harness/sweep.py:59-69`) is the **drop-in target** — match it exactly:
  ```python
  SweepQuery(corpus_id: str, query_id: str, query: str, query_type: str,
             gold_files: frozenset[str], gold_blocks: frozenset[tuple[str, str]])
  ```
  Consumed unchanged by `score_vectors` (`sweep.py`), `run_ab` (`ab_runner.py`), and the R2.4 reporter. `gold_blocks` elements are `(file_path, symbol_name)` pairs — the same pairing the Rust scorer + `micro_suite.json` use.
- **Loader convention (`r1harness/corpus.py`):** pure parse from a path/structure; `@dataclass(frozen=True)`; deterministic first-seen ordering; `KeyError` with an "available" list on miss. Mirror this style.
- **`query_type`** in the micro-suite is `"keyword"` / `"semantic"` (defaults to `"keyword"`); pick a deterministic mapping from ContextBench's task metadata and document it (if no signal, default `"keyword"` like `load_suite`).

## Scenarios to cover (RED first — pure-logic, deterministic, network-free, binary-free)
- [ ] **happy (THE core proof):** a small inline/fixture ContextBench-Lite record (faithful to the parquet schema) maps to a `SweepQuery` with correct `corpus_id`/`query_id`/`query`/`query_type`/`gold_files`/`gold_blocks`. Assert exact field values (hand-specified in the test).
- [ ] **happy:** multiple records map to multiple `SweepQuery` in deterministic order; the result is directly consumable by `score_vectors`-shaped code (assert the frozensets + tuple shapes).
- [ ] **edge:** a record with multiple gold files / multiple gold blocks → frozensets contain all of them (no dedup loss, no ordering dependence — frozenset equality).
- [ ] **edge:** missing/empty optional fields (no gold blocks, or block without a symbol name) → handled per a documented rule (e.g. empty frozenset, or skip the block) with no crash.
- [ ] **error / hermeticity:** the mapper core imports nothing that does network or binary I/O (assert by import-surface, mirroring the R2.4 purity test); the fetch entrypoint is the ONLY network surface and is NOT exercised by the test suite (no live download in CI/pytest).
- [ ] **fetch-entrypoint robustness (unit, mocked or cache-present):** missing cache dir → stderr instruction + clean nonzero exit (no traceback, no auto-download). Do NOT make a real network call in the test.
- [ ] **gitignore:** the new cache dir is matched by `research/r1_harness/.gitignore` (verify with `git check-ignore` in the GREEN/run notes, not necessarily a pytest).

## Definition of Done
- [ ] Tests written first (RED captured in this brief), now green; `ruff check research/` clean; `ruff format --check research/` clean. Suite **green-with-1-skip** (the existing Windows-only path test SKIPS on Linux — expected; no new skips introduced unless justified).
- [ ] Pure mapper core has **no network / no binary / no file I/O**; the fetch entrypoint is the only network surface and is **not** invoked by the test suite (hermetic).
- [ ] Mapper output is the **unchanged `SweepQuery`** shape — drops into `score_vectors`/`run_ab`/the R2.4 reporter with zero scorer change (D21).
- [ ] `datasets` + `huggingface_hub` pinned (`==`) in `requirements.txt`; venv-vs-system install choice recorded in `research/CLAUDE.md`.
- [ ] New gitignored cache dir added to `research/r1_harness/.gitignore`; **no corpus blobs committed**.
- [ ] No Rust/Cargo/`.claude/settings.json` changes. `code-reviewer` APPROVED.
- [ ] **Doc-sync (same commit):** D26 in ROADMAP (manager, done); `docs/TODO.md` R2.5 row → R2.5a done / R2.5b next; `research/CLAUDE.md` (new external-corpus loader + HF dependency + provenance/attribution + **product stays air-gapped**; update the "No paid spend / process boundary" rules to reflect the now-authorized one-time cached HF download); `requirements.txt` pinned deps.
- [ ] **Confirm the CodeRAG-Bench LICENSE** (for R2.5b) — report it back even though R2.5b is not built this pass; if the env proxy blocks it, say so and leave R2.5b's "confirm LICENSE first" step in the brief.
- [ ] Committed locally with explicit pathspec (exclude `.claude/settings.json`, `runs/`, the corpus cache dir). **NOT pushed.** Report commit hash + green pytest/ruff summary + a **sample ContextBench-Lite record mapped into a `SweepQuery`** (proves the real-corpus path) + the confirmed CodeRAG-Bench LICENSE.

## Constraints (HARD — unchanged from R2.3b/R2.4 + the D26 additions)
- Pure `research/`, tests-first, **ruff + pytest** gates (both clean). **No `src/`, no `Cargo.*`, no Rust `tests/*.rs`.** Crate change ⇒ scope error ⇒ escalate.
- **Do NOT touch `.claude/settings.json`** — leave it unstaged, OUT of the commit (explicit pathspec).
- Cached corpus data stays **gitignored** (extend `.gitignore` for the new cache dir). Do NOT commit downloaded corpus blobs.
- **Network is authorized for the fetch entrypoint ONLY** (one-time, cached, no token, zero spend). The **test suite must remain hermetic** — it runs against the cached/fixture slice and never re-downloads.
- **Commit locally, do NOT push.**

---
## RED — test lead (research-harness-engineer)

**Tests written:** `research/r1_harness/tests/test_contextbench.py` (11 test functions originally, 14 after R2.5a-rev fixes; 6 scenario groups + 3 new tests for reviewer findings)

**Public API the mapper must satisfy:**
```python
from r1harness.contextbench import parse_contextbench_records
parse_contextbench_records(records: list[dict]) -> list[SweepQuery]
```

**Field-mapping rules (hand-derived from the real parquet schema, verified via HF datasets-server API):**
- `corpus_id`  = `record["repo"]`               e.g. `"astropy/astropy"`
- `query_id`   = `record["instance_id"]`         e.g. `"SWE-Bench-Verified__python__..."`
- `query`      = `record["problem_statement"]`   natural-language task description
- `query_type` = `"keyword"`                     no task-type signal in schema; matches `load_suite()` default
- `gold_context` = `record["gold_context"]`      JSON string: `[{"file": str, "start_line": int, "end_line": int, "content": str}, ...]`
- `gold_files`  = `frozenset` of unique `"file"` values across all entries
- `gold_blocks` = `frozenset` of `(file_path, symbol_name)` pairs; `symbol_name` = `"<file>::L<start>-L<end>"` (stable proxy; no real symbol names in ContextBench)

**Failing output (exit code 2 — collection error, correct RED state):**
```
ERROR collecting tests/test_contextbench.py
tests/test_contextbench.py:142: in <module>
    from r1harness.contextbench import parse_contextbench_records
E   ModuleNotFoundError: No module named 'r1harness.contextbench'
```

## GREEN — engineering lead (research-harness-engineer)

### Files created / edited
- `research/r1_harness/r1harness/contextbench.py` — pure mapper core (`parse_contextbench_records`)
- `research/r1_harness/fetch_contextbench.py` — thin fetch entrypoint (only network surface)
- `research/r1_harness/tests/test_contextbench.py` — 11 tests (all green)
- `research/r1_harness/requirements.txt` — pinned `datasets==5.0.0` + `huggingface_hub==1.19.0`
- `research/r1_harness/.gitignore` — added `cache/` pattern
- `research/CLAUDE.md` — updated with R2.5a apparatus, venv decision, external-corpus provenance

### Gates
- `ruff check research/` — clean (0 errors)
- `ruff format --check research/` — clean (33 files already formatted)
- `pytest research/r1_harness/` — **99 passed, 1 skipped** (pre-existing Windows-only path skip; no new skips)

### Sample ContextBench-Lite record mapped to SweepQuery (real-corpus proof)

Input record (faithful to real HF parquet row 0, `contextbench_verified`):
```json
{
  "instance_id": "SWE-Bench-Verified__python__maintenance__bugfix__deb49033",
  "repo": "astropy/astropy",
  "problem_statement": "A direct approach to ITRS to Observed transformations that stays within the ITRS.",
  "gold_context": "[{\"file\": \"astropy/coordinates/attributes.py\", \"start_line\": 344, \"end_line\": 396, \"content\": \"class EarthLocationAttribute(Attribute):\\n    ...\"}]"
}
```

Output `SweepQuery`:
```
SweepQuery(
    corpus_id   = "astropy/astropy",
    query_id    = "SWE-Bench-Verified__python__maintenance__bugfix__deb49033",
    query       = "A direct approach to ITRS to Observed transformations that stays within the ITRS.",
    query_type  = "keyword",
    gold_files  = frozenset({"astropy/coordinates/attributes.py"}),
    gold_blocks = frozenset({("astropy/coordinates/attributes.py",
                              "astropy/coordinates/attributes.py::L344-L396")}),
)
```

The `SweepQuery` shape is the UNCHANGED D21 shape — drops into `score_vectors`/`run_ab`/the R2.4 reporter with zero scorer change.

### Dependencies + venv decision
- `datasets==5.0.0` + `huggingface_hub==1.19.0` pinned in `requirements.txt` (exact `==`)
- **Venv decision:** project-local venv at `research/r1_harness/.venv/` (gitignored); system Python is externally managed (PEP 668/Debian). Recorded in `research/CLAUDE.md`.

### gitignore
Added `cache/` pattern to `research/r1_harness/.gitignore` (line 19).
`git check-ignore` confirmation:
```
research/r1_harness/.gitignore:19:cache/	research/r1_harness/cache/contextbench/test_file.json
```

### CodeRAG-Bench LICENSE (for R2.5b)
**Could not confirm via LICENSE file.** The GitHub repo `code-rag-bench/code-rag-bench` has **no LICENSE file** in the root (GitHub API `/license` returns 404; root listing: README.md, generation/, preprocessor/, requirements.txt, retrieval/ — no LICENSE). The CC-BY-SA 4.0 claim is from the paper/release notes only, not from an on-disk file. First step of R2.5b: re-check (repo may have added one) and record the confirmed license before loading any data. Recorded in `research/CLAUDE.md`.

## Specialist / Perf notes
<n/a unless a parquet/HF-schema or scorer-compat question arises — escalate to manager if so>

## REVIEW — code reviewer

**Verdict: BLOCK** (one major correctness/hermeticity defect; the rest is solid). 2026-06-15.

### Gates (independently re-run on Linux)
- `ruff check research/` → **All checks passed** (exit 0).
- `ruff format --check research/` → **33 files already formatted** (exit 0).
- `pytest research/r1_harness` → **99 passed, 1 skipped** (exit 0). The 1 skip is the pre-existing
  Windows-only `tests/test_codecache_tool.py:23` — expected, no new skips. `tests/test_contextbench.py`
  alone → 11 passed.
- Note: the venv (`research/r1_harness/.venv/`) contains only pytest+ruff; `datasets`/`huggingface_hub`
  were NOT actually installed despite the brief's "venv created with -r requirements.txt" claim. This
  matters for the BLOCKER below.

### Findings

- **BLOCKER — fetch_contextbench.py:160-168 (+ test_contextbench.py:324-342)** — *Missing-cache path is
  not hermetic and the test that "proves" it is too weak.* `main()` with a missing cache and no `--force`
  evaluates `cp.exists()` False and falls straight through to `fetch_and_cache()`, which calls
  `from datasets import load_dataset` and then `load_dataset("Contextbench/ContextBench", ...)` — a **live
  HF network download**. There is NO missing-cache-instruction branch in `main()` (the stderr instruction
  lives only in `load_cached_contextbench()`, which `main()` never calls). I proved this: with `datasets`
  absent the script exits 1 via the ImportError fallback; with a stub `datasets` installed (the intended
  provisioned-venv state per requirements.txt + DoD) the same invocation **invokes `load_dataset` and would
  hit the network**. `test_fetch_entrypoint_missing_cache_exits_nonzero` only passes today because the venv
  was never provisioned with `datasets`; once it is (as the DoD requires) that test makes a live network
  call — violating the HARD "test suite must remain hermetic / never re-download" constraint. The assertion
  is also tautologically weak: `"fetch_contextbench" in err OR "run" in err` is satisfied by the ImportError
  pip-install message, so it does not actually verify no-download behavior.
  *Fix:* give the test an explicit no-download guarantee — either pass a flag the entrypoint honors that
  prints the missing-cache stderr instruction and exits nonzero WITHOUT importing/calling `datasets` (mirror
  `run_report.py`'s precedent: missing cache → instruct-and-exit, never auto-download), or have the test
  invoke `load_cached_contextbench()` directly (the function that actually has the instruct-and-exit
  behavior) rather than `main()`. Then tighten the assertion to require the cache-not-found instruction
  text, and add a test that asserts `datasets`/`load_dataset` is never imported/called on the missing-cache
  path (e.g. run with a poisoned `datasets` stub on PYTHONPATH that raises if `load_dataset` is touched).
  The mapper core itself is fully hermetic (verified below) — this is purely the entrypoint + its test.

- **MAJOR — contextbench.py:72-80** — *Reachable crash on malformed gold_context structure.* When
  `gold_context` is valid JSON but an array element is a non-dict (e.g. `'["juststring"]'`), the loop calls
  `entry.get(...)` on a `str` → uncaught `AttributeError`. The module docstring (line 28) and the brief both
  promise "missing/null/empty/**malformed** gold_context → empty frozensets, no crash," and the code already
  defends against malformed-JSON-string, dict-instead-of-list, and None — but a JSON array of non-dicts slips
  through. Real ContextBench rows are lists of objects so this won't fire on clean data, but the documented
  contract (and the no-reachable-panic golden rule) is violated, and there is no test for it.
  *Fix:* `if not isinstance(entry, dict): continue` (or wrap in a guard) before `entry.get(...)`; add a RED
  test for a non-dict list entry → empty frozensets.

- **MINOR — contextbench.py:73-80** — Non-string `file` value (e.g. `123`) silently produces an `int` member
  in `gold_files` and the block tuple, breaking the `frozenset[str]` / `frozenset[tuple[str,str]]` type
  contract. Won't occur on the real schema; cheap to coerce/guard (`if not isinstance(file_path, str)`).

- **MINOR — contextbench.py:85** — Return annotation is bare `-> list` (the function-local `SweepQuery`
  import prevents `-> list[SweepQuery]` at module scope). Consider `from __future__ import annotations` +
  `TYPE_CHECKING`-guarded import, or `"list[SweepQuery]"` as a string annotation, to recover the precise type
  without breaking import purity. Nit.

- **MINOR (doc) — brief RED header line 83** — claims "15 test functions, 6 scenario groups"; the file has
  **11** test functions (GREEN section line 113 correctly says 11). Stale count in the RED header.

### Finding #4 — the gold_blocks symbol_name proxy (assessed as requested)
**Disposition: NOT a blocker for R2.5a; it is an honest, correctly-documented limitation that MUST be
flagged as a hard follow-up gating the R2.5b/R2.7 scoring run.** Reasoning:
- The scorer matches blocks by **exact set membership** on `(file_path, symbol_name)` tuples
  (`scorer.py` `recall_at_k`/`dcg_at_k`: `item in gold`). The loader encodes
  `symbol_name = "<file>::L<start>-L<end>"`, while CodeCache emits **real** symbol names
  (e.g. `EarthLocationAttribute`). These never compare equal, so **every block-level metric over a
  ContextBench corpus would be 0** unless the retrieved blocks are re-encoded into the same line-range proxy.
  I confirmed the matching mechanics directly in `sweep.py:121-128` and `scorer.py:37-100`.
- For R2.5a's **stated scope** — a *loader that proves the real-corpus path produces the unchanged
  `SweepQuery` shape and flows into `score_vectors`/`run_ab`/the reporter with zero scorer change* — this is
  correct and in-scope. The loader does NOT claim block-level numbers are meaningful; the docstring
  (lines 24-26) and brief both state symbol names are a "stable proxy" and that this is a real-corpus
  *ablation* where **file-level gold is unaffected** (real file paths DO match — verified) and **block-level
  needs reconciliation**. That is honest.
- It is therefore **not a correctness bug in the loader**, but it WILL silently produce misleading
  (all-zero) block metrics if R2.5b/R2.7 runs the scorer against this corpus without reconciling the
  encodings. **Required follow-up (manager to log against R2.5b/R2.7):** before any ContextBench *scoring*
  run, either (a) make the retrieval adapter emit blocks under the same line-range proxy when scoring
  ContextBench, or (b) restrict the ContextBench ablation to **file-level** metrics and explicitly mark
  block-level as N/A in the reporter. The reporter's scope-honesty disclaimer should call this out.

### What passed (verified, not just claimed)
- **Mapper purity (HARD):** importing `r1harness.contextbench` pulls in only stdlib (`json`, `re`, `typing`,
  `enum`, …) + `r1harness`; transitively NO `datasets`/`huggingface_hub`/`requests`/`urllib`/`socket`/
  `subprocess`/`http`, even through `sweep.py` (whose own imports are stdlib + `.corpus`/`.scorer`). The
  function-local `SweepQuery` import is sound. Both purity tests are real.
- **Drop-in shape (D21):** the mapper reuses `sweep.SweepQuery` (does NOT redefine it); output field types
  match exactly and `score_vectors` consumes it unchanged. Verified.
- **TDD integrity:** tests are hand-specified with exact expected field values (not `is_ok()`-style);
  `contextbench.py` was never previously committed (consistent with RED). Robustness edges that ARE covered
  (None / "[]" / missing key / malformed JSON string / empty list) all yield empty frozensets, no crash.
- **Test suite never imports the fetch module** except via the one subprocess invocation (the only network
  surface) — grep-confirmed.
- **Scope discipline (HARD):** zero `src/`/`Cargo.toml`/`Cargo.lock`/Rust-`tests/*.rs` changes;
  `.claude/settings.json` modified-but-UNSTAGED (will be excluded by explicit pathspec — confirm at commit);
  no corpus blobs tracked; `cache/` is gitignored and `git check-ignore` matches the cache file path
  (`.gitignore:19`).

**Re-review needed on:** the BLOCKER (entrypoint no-download guarantee + a real no-download test) and the
MAJOR (non-dict entry guard + test). The two style minors and the doc nit are optional. Once the BLOCKER and
MAJOR are fixed and re-tested green, I expect to APPROVE.


## R2.5a-rev — research-harness-engineer (2026-06-15)

**Status after BLOCK:** all reviewer findings addressed.

### BLOCKER fix — fetch entrypoint missing-cache path hermetic
`fetch_contextbench.py main()` rewritten: the default path (no `--force`) now checks the cache
and, if missing, **prints the cache-not-found instruction and exits 1 WITHOUT calling
`fetch_and_cache()` or importing `datasets`**. Only `--force` triggers `fetch_and_cache()`.
This mirrors the `run_report.py` precedent exactly.

Previously: `main()` always fell through to `fetch_and_cache()` for any missing cache.
After fix: `if args.force:` routes to download; else checks cache, instruct-and-exit if missing.

### MAJOR fix — non-dict gold_context entries
`contextbench.py _parse_gold_context()`: added `if not isinstance(entry, dict): continue` guard
before `entry.get(...)`. JSON arrays of non-dicts (strings, ints, nulls) now produce empty
frozensets with no crash. Documented in the guard comment.

### MINOR fixes
- Non-string `file` value: `if not isinstance(file_path, str) or not file_path: continue` guard
  added. Rule: non-string file values are **skipped** (not coerced) to preserve `frozenset[str]`
  contract. Documented in the guard comment.
- `-> list[SweepQuery]` annotation: added `from typing import TYPE_CHECKING` + `if TYPE_CHECKING:
  from .sweep import SweepQuery` at module level. `from __future__ import annotations` (already
  present) makes the annotation a lazy string at runtime, so the function-local runtime import
  still provides the actual class. Return annotation is now `-> list[SweepQuery]` (precise).
- Brief RED header stale count: fixed from "15 test functions" to accurate description (11
  original + 14 after rev).

### New tests added (3 new RED → GREEN)
- `test_fetch_entrypoint_missing_cache_exits_nonzero`: tightened — now asserts
  `"fetch_contextbench.py"` in stderr (not just any "run" substring); uses default path (no
  `--read-only` flag needed since default IS the no-download path after fix).
- `test_fetch_entrypoint_missing_cache_never_calls_load_dataset`: NEW — injects a poisoned
  `datasets` stub on `PYTHONPATH` that raises `RuntimeError("POISON: ...")` if `load_dataset`
  is touched; asserts the default path exits nonzero WITHOUT the POISON string in output.
  Proves hermetic no-download behaviour even when `datasets` is importable.
- `test_nondict_gold_context_entries_yield_empty_frozensets`: NEW — `gold_context='["juststring",
  42, null]'` → empty frozensets, no crash.
- `test_nonstring_file_value_skipped_or_coerced`: NEW — `"file": 123` → entry skipped; all
  `gold_files` members are `str`.

### Provisioning honesty update
`research/CLAUDE.md` updated to state plainly: `datasets`/`huggingface_hub` are pinned in
`requirements.txt` but **NOT installed** in the venv. The test suite is hermetic precisely
because the default missing-cache path never imports them (enforced by the poison test).

### Gates
- `ruff check research/` — **All checks passed** (exit 0)
- `ruff format --check research/` — **33 files already formatted** (exit 0)
- `pytest research/r1_harness/` — **102 passed, 1 skipped** (the pre-existing Windows-only skip;
  no new skips; 14 tests in `test_contextbench.py` all green)

### Confirmed: missing-cache path does NOT import/call datasets
`/mnt/c/Users/ehlee/workspace/projects/CodeCache/research/r1_harness/fetch_contextbench.py`
with missing cache (no `--force`): prints "ERROR: ContextBench-Lite cache not found" + "Run
the fetch entrypoint first: python3 fetch_contextbench.py --force" → exits 1. `datasets` is
never imported. The poison test (`test_fetch_entrypoint_missing_cache_never_calls_load_dataset`)
proves this hermetically even when `datasets` is importable.

## OUTCOME — manager

**Date:** 2026-06-15  **Manager:** principal-engineering-manager  **Slice: R2.5a DONE (R2.5b scoped, deferred)**

**Aligned — Definition of Done met:**
- **Gate ratification recorded:** D26 added to the ROADMAP Decision Log verbatim (Corpus=BOTH — ContextBench-Lite Apache-2.0 for the real-corpus ablation + CodeRAG-Bench RepoEval CC-BY-SA for the R2.7 published-BM25 reproduction; Network/HF authorized one-time-cached, no token, zero spend, product air-gapped; the ~$1K R3 spend NOT authorized).
- **TDD order honored:** RED (failing `ModuleNotFoundError` collection error) preceded GREEN; the BLOCK→fix cycle added 3 new RED→GREEN tests (incl. the hermetic poison-stub no-download proof). Nothing weakened or deleted (count 11 → 14; assertions strictly stronger).
- **Gates green (Linux/WSL2, `python3` 3.12, venv = pytest+ruff only):** `pytest research/r1_harness` **102 passed, 1 skipped** (the Windows-only path skip, expected; 0 failures/errors); `ruff check research/` clean; `ruff format --check research/` clean (33 files).
- **Hermeticity (HARD) — verified by the reviewer's reproduction:** the mapper core (`contextbench.py`) does no network/binary/file I/O and imports neither `datasets`/`huggingface_hub` nor even `sweep` at module load; the fetch entrypoint is the only network surface, confined to the `--force` path; the missing-cache read path instruct-and-exits without importing `datasets` (proven hermetically by the poison-stub test even when `datasets` is importable).
- **Drop-in shape (D21 "scorer unchanged"):** the mapper emits the unchanged `SweepQuery` (reuses `sweep.SweepQuery`, does not redefine) — drops into `score_vectors`/`run_ab`/the R2.4 reporter with zero scorer change. Real-corpus path PROVEN: a real `contextbench_verified` row (`astropy/astropy`) maps to a correct `SweepQuery` (GREEN section).
- **Deps + provenance + gitignore:** `datasets==5.0.0` + `huggingface_hub==1.19.0` pinned (fetch-only, not installed — honestly recorded); `research/CLAUDE.md` carries the venv decision, the D26 network exception, the external-corpus provenance/attribution, and the R2.5b "confirm LICENSE first" step; `cache/` gitignored (verified `git check-ignore`); no corpus blobs.
- **code-reviewer APPROVED** (BLOCK → fix → APPROVE; reviewer independently re-ran ruff+pytest and reproduced the no-download proof).

**Scope discipline (verified):** zero Rust `src/`/`Cargo.toml`/`Cargo.lock`/Rust-`tests/*.rs` changes; `.claude/settings.json` left modified-but-unstaged and EXCLUDED from the commit via explicit pathspec.

**CodeRAG-Bench LICENSE (for R2.5b):** could NOT be confirmed via an on-disk LICENSE file — the repo root has none (GitHub API `/license` → 404); the CC-BY-SA 4.0 claim is paper/release-only. Recorded in `research/CLAUDE.md` + `docs/TODO.md` R2.5b as the mandatory first build step (re-check + confirm before loading any data).

**Hard follow-up logged against R2.5b/R2.7 (the reviewer's finding #4 — NOT a loader bug):** ContextBench has no real symbol names, so `gold_blocks` uses a line-range proxy `"<file>::L<start>-L<end>"` while CodeCache emits real symbol names. A naive ContextBench *scoring* run would therefore yield all-zero **block-level** metrics. Before any ContextBench scoring run: either (a) re-encode retrieved blocks to the same line-range proxy when scoring ContextBench, or (b) restrict the ContextBench ablation to **file-level** metrics (file-level gold matches as-is) and mark block-level N/A in the reporter's scope-honesty disclaimer. Recorded in `docs/TODO.md` R2.5a "Known limitation."

**Doc-sync (same commit):** D26 in ROADMAP; `docs/TODO.md` R2.5 row split (R2 parent → R2.5a DONE / R2.5b NEXT, with the known-limitation note); `research/CLAUDE.md` (engineer-authored, verified accurate); `requirements.txt` pinned; brief status → DONE. Reviewer's two manager-owned doc-sync items actioned (TODO split + OUTCOME filled); GREEN-section "99 passed" is a harmless historical record superseded by the rev "102 passed" — left as the audit trail.

**Commit:** local only (no push), explicit pathspec excluding `.claude/settings.json`, `runs/`, and the corpus `cache/`. Hash recorded in the user report.

## RE-REVIEW (R2.5a-rev) — code reviewer (2026-06-15)

**Verdict: APPROVE.** The BLOCKER and the MAJOR are genuinely and completely closed; the two
style minors and the doc nit are fixed; hermeticity and scope discipline still hold. Two
non-blocking doc-sync items below are for the manager to close at commit time.

### Gates (independently re-run on Linux, venv = pytest+ruff only)
- `ruff check research/` → **All checks passed** (exit 0).
- `ruff format --check research/` → **33 files already formatted** (exit 0).
- `pytest research/r1_harness` → **102 passed, 1 skipped** (exit 0). The 1 skip is the
  pre-existing Windows-only `tests/test_codecache_tool.py:23` (backslash path semantics) — NOT
  R2.5a-related, no new skips. `tests/test_contextbench.py` alone → **14 passed**.
- Confirmed `datasets`/`huggingface_hub` are genuinely absent from the venv (`find_spec` → None
  for both), so hermeticity is real, not accidental — see the poison proof below.

### BLOCKER — CLOSED (verified, not just claimed)
`main()` now splits on `--force` (fetch_contextbench.py:162-180). The default (no-`--force`) path
with a missing cache prints the cache-not-found instruction mentioning `fetch_contextbench.py
--force` and `return 1` WITHOUT importing or calling `datasets`. The `from datasets import
load_dataset` is confined to `fetch_and_cache()` (line 91), reached ONLY via the `--force` branch.
I proved the test is non-tautological by reproducing both arms with a poisoned `datasets` stub on
PYTHONPATH (raises if `load_dataset` is touched):
- **Default path, missing cache, poison present** → exits 1, prints the instruction, **no
  "POISON"** in output. (This is what `test_fetch_entrypoint_missing_cache_never_calls_load_dataset`
  asserts — and it would FAIL if the default path leaked into a download.)
- **`--force` path, missing cache, poison present** → output shows `download failed: POISON:
  load_dataset was called`, exits 1 cleanly (caught by `except Exception`). This proves the stub is
  correctly wired and that the only code path that reaches `load_dataset` is `--force`.
The tightened assertion now requires `"fetch_contextbench.py"` in stderr (was a weak OR). Real
no-download guarantee established.

### MAJOR — CLOSED (verified)
`_parse_gold_context()` now has `if not isinstance(entry, dict): continue` (contextbench.py:78-79)
before any `entry.get(...)`. `test_nondict_gold_context_entries_yield_empty_frozensets`
(`gold_context='["juststring", 42, null]'`) → empty frozensets, no `AttributeError`. Passes.

### Minors / doc nit — CLOSED
- Non-string `file` value: `if not isinstance(file_path, str) or not file_path: continue`
  (contextbench.py:83-84); rule documented (skip, not coerce); `test_nonstring_file_value_skipped_or_coerced`
  passes — `frozenset[str]` contract preserved.
- Return annotation recovered to `-> list[SweepQuery]` via `from __future__ import annotations` +
  `TYPE_CHECKING`-guarded `from .sweep import SweepQuery` (contextbench.py:34-37). Verified in a fresh
  interpreter: `import r1harness.contextbench` pulls in NEITHER `datasets` NOR `r1harness.sweep`
  (`sys.modules` checks both False) — the TYPE_CHECKING import is runtime-inert and the function-local
  import still resolves the real class at call time (output is a genuine `SweepQuery`). Import purity preserved.
- Brief RED-header stale "15 tests" count corrected to "11 originally, 14 after rev" (line 83).

### Hermeticity / purity — STILL HOLDS (re-verified)
- `test_mapper_core_import_surface_is_pure` + `test_mapper_module_does_not_import_datasets_at_module_level`
  both pass; fresh-interpreter import leaks no `datasets`/`huggingface_hub`/`subprocess`/`urllib`/
  `socket` and does not even load `sweep` at import time.
- No live network call anywhere in the suite; the fetch module is touched only via subprocess in the
  two robustness tests, both of which target the missing-cache (no-download) path.

### No regressions / no test weakening
Assertions are strictly stronger (tightened the entrypoint stderr assertion; added the poison
no-download proof; added two malformed-input guards with tests). Nothing deleted; count went
11 → 14. The drop-in `SweepQuery` shape (D21 scorer-unchanged) is intact.

### Scope discipline (HARD)
- Zero `src/`/`Cargo.toml`/`Cargo.lock`/Rust-`tests/*.rs` changes in this slice. (The `git diff main`
  noise is because the whole branch was cut from an empty main; the working-tree change set for
  R2.5a is exactly: new `fetch_contextbench.py`, `contextbench.py`, `test_contextbench.py`, this brief,
  and modified `research/CLAUDE.md` / `requirements.txt` / `.gitignore` / `docs/ROADMAP.md`.)
- No corpus blobs tracked; `cache/` is gitignored.
- `.claude/settings.json`: modified-but-UNSTAGED in the working tree (the Stop/SubagentStop
  cargo-hook removal pre-dates this slice and is environmental — PowerShell hooks can't run on this
  Linux env). It is NOT part of the R2.5a commits and the brief instructs an explicit-pathspec commit
  that excludes it. **Manager: confirm it is excluded at commit time** (DoD line 71).

### Non-blocking doc-sync items for the manager (NOT engineer defects; close at commit)
1. `docs/TODO.md` is not yet updated — line 430 still reads "R2.5 ... — NEXT" with no R2.5a-done /
   R2.5b-next split (DoD line 69). The manager owns TODO + the OUTCOME section (still placeholder) and
   marks the slice done; do this in the commit.
2. Brief GREEN section line 121 has a stale "99 passed" (superseded by the R2.5a-rev "102 passed" at
   line 321). Harmless historical record; optional to reconcile.

### Block-level symbol_name proxy (re-affirmed from prior review)
Still NOT a blocker for R2.5a (honest, documented limitation). The hard follow-up stands: before any
ContextBench *scoring* run (R2.5b/R2.7), either re-encode retrieved blocks into the same line-range
proxy or restrict the ContextBench ablation to file-level metrics (block-level = N/A in the reporter).
Manager: log against R2.5b/R2.7.

**Bottom line: APPROVE.** Slice is correct, hermetic, idiomatic, and in scope. Remaining items are
manager-owned doc-sync/closeout at commit time, not code defects.
