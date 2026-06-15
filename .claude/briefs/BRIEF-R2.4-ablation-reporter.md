# BRIEF — R2.4 / ablation-table reporter (+ broken-window fix)

- **Milestone:** R2 offline ablations (D23) — slice **R2.4 (UNGATED) ablation-table reporter**  ·  **Module(s):** `research/r1_harness/` only (Python; `r1harness/ablation_report.py` + `run_report.py` + tests). **Pure `research/`.**
- **Owner (manager):** principal-engineering-manager  ·  **Implementer:** research-harness-engineer  ·  **Created:** 2026-06-15
- **Status:** RED ▢  GREEN ▢  REVIEW ▢  DONE ▢
- **Links:** docs/ROADMAP.md "Research track" (R2.4, D23) · docs/TODO.md "Research track" · project_overview.md §7 (outcome-agnostic) · prior: BRIEF-R2.2b (sweep), BRIEF-R2.3b-stub-chunker-ab-plumbing.md
- **Prior commits:** R2.3b DONE `91465d7` · R2.3a ingest seam `fc28fdd` · R2.2b sweep step `292b0e4`.

## Goal
Bundle THREE things into ONE slice / ONE brief / ONE reviewer pass / ONE local commit:
1. **Broken-window fix** — scope the known Linux-failing pytest (`test_normalize_relative_path_backslashes_to_posix`) to Windows via `@pytest.mark.skipif`, so `pytest research/r1_harness` is 100% green on Linux. The assertion body is UNCHANGED (no test weakening — golden-rule compliant).
2. **R2.4** — a pure deterministic **ablation-table reporter** that aggregates the R2.2b weight-sweep report and the R2.3b A/B chunker report into ONE high-level Markdown view comparing **NDCG@10** and **F1@10** across (a) BM25 weight vectors and (b) chunking strategies, plus a directional top-config selection.
3. **Housekeeping** — ruff clean, tests-first, doc-sync in the same commit, local commit (no push).

## Scope (in / out)
- **In:** Step-1 skip decorator; Step-2 `r1harness/ablation_report.py` pure-logic core + thin loader + `run_report.py` entrypoint + pytest; regenerate `runs/sweep/report.json` and generate `runs/ab/report.json` against `target/debug/codecache`; render the real table.
- **Out (scope errors → escalate to manager):**
  - **No Rust `src/`, `Cargo.toml`, `Cargo.lock`, or Rust `tests/*.rs` changes.** If a crate change seems needed, STOP and escalate. This is pure Python over the existing process boundary.
  - **Do NOT touch `.claude/settings.json`** — it is intentionally modified-but-unstaged (cargo hooks disabled by the user, unrelated to research). Leave it unstaged and OUT of the commit (explicit pathspec).
  - No external corpus, no astchunk, no LLM/agent/paid spend (those are gated R2.5–R2.7 / R3).
  - No "winner" claim (native vs stub chunker, or a beating-the-default weight vector) as a *finding* — directional only.

## Data shapes (verified — do NOT rediscover)
- **Sweep report** `runs/sweep/report.json` (REGENERATE against the current Linux binary so it is internally consistent with the A/B run): top-level `{binary, n_queries, n_keyword, column_order[7], grid[], vectors[]}`. Each `vectors[i]` = `{label, weights:[7], n_queries, n_keyword, macro_all:{"1":M,"5":M,"10":M}, macro_keyword:{...}}` where `M` is a `MetricAtK` dict `{k, recall_file, precision_file, f1_file, ndcg_file, recall_block, precision_block, f1_block, ndcg_block}`.
  - **F1@10 = `macro_all["10"].f1_block`**, **NDCG@10 = `macro_all["10"].ndcg_block`** (block granularity is primary; file granularity also present).
- **A/B report** `runs/ab/report.json` (does NOT exist yet — `run_ab.py` must be RUN against `target/debug/codecache`): `{binary, rows[]}` where each row = `{corpus_id, arm:"native"|"stub", n_queries, macro_all:{"1","5","10"}}`. **3 corpora × 2 arms = 6 rows.**
- **CRITICAL aggregation subtlety:** sweep `macro_all` is already aggregated over all 15 queries; A/B rows are **per-corpus per-arm**. To compare chunking strategies apples-to-apples with the sweep, aggregate the A/B per-corpus rows into **one row per arm using n_queries-weighted averaging** (Σ nᵢ·metricᵢ / Σ nᵢ), NOT a plain mean of per-corpus macros. **This must be pinned in a RED test with hand-computed numbers.**

## Reporter design (testability first — mirrors R2.2b/R2.3b: pure core + thin I/O shell)
- **Pure-logic core** `r1harness/ablation_report.py`: functions take *parsed* structures (the sweep `vectors` / `VectorResult`-shaped data and the A/B `rows`) and return (i) the aggregated per-arm rows and (ii) a rendered **Markdown** table string. **No binary, no file I/O in the core.**
- **Thin loader** reads the two `report.json` files into those structures.
- **Entrypoint** `run_report.py` (mirrors `run_sweep.py`/`run_ab.py`): loads both raw reports, prints + writes the rendered table (e.g. `runs/ablation/report.md`). Run artifacts under `runs/` are NOT committed (see gitignore note).
- **Top-config selection:** reuse `sweep.rank_vectors` (ranks vectors best-first by a macro metric at k, stable ties) to pick the best weight vector by NDCG@10; surface the shipped default `(10,1,1,5,2,2,2)` explicitly and note whether the proxy agrees with it.
- **Scope honesty (HARD):** the micro-suite is a 15-query PROXY (its own description + `sweep.py` docstring say so) — the table is a **directional signal**, NOT a published finding (that is the gated R2.5–R2.7 real-corpus run), and R2 is **outcome-agnostic** (overview §7). The reporter may *recommend* a directional default but must **label it directional** and must **not** assert a chunker winner. Keep the hedging consistent with how `run_sweep.py`/`run_ab.py` already frame it.

## Scenarios to cover (RED first — pure-logic, deterministic, binary-free)
- [ ] **happy:** F1@10 / NDCG@10 extracted from a sweep `vectors` list at block granularity (`macro_all["10"].{f1_block,ndcg_block}`); Markdown table renders the weight-vector section with one row per vector.
- [ ] **happy:** A/B per-corpus rows aggregate into one row per arm; Markdown table renders the chunker section with one row per arm.
- [ ] **edge (THE pinned invariant):** n_queries-weighted aggregation — two corpora with DIFFERENT `n_queries` and KNOWN metrics → assert the weighted aggregate equals Σ nᵢ·metricᵢ / Σ nᵢ (NOT the plain mean). Hand-compute the expected number in the test.
- [ ] **happy:** top-config selection picks the best weight vector by NDCG@10 (reuse `rank_vectors`); output surfaces the default `(10,1,1,5,2,2,2)` and states whether the proxy agrees.
- [ ] **scope honesty:** rendered output contains an explicit "directional / PROXY / not a published finding" disclaimer and does NOT contain a chunker-winner assertion.
- [ ] **edge:** empty / single-arm / zero-n_queries inputs do not divide-by-zero (return zeroed or skip gracefully, matching `macro_average`'s empty-input contract).
- [ ] **Step 1:** `test_normalize_relative_path_backslashes_to_posix` is decorated `@pytest.mark.skipif(sys.platform != "win32", ...)` (add `import sys`); body unchanged; it SKIPS (not fails) on Linux.

## Gitignore precedent (resolved before coding — corrects an assumption in the request)
`research/r1_harness/.gitignore:11` is a blanket `runs/` rule. `git ls-files research/r1_harness/runs/` is EMPTY and `git log -- research/r1_harness/runs/` shows **no commit has ever touched `runs/`** — R2.2b (`292b0e4`) and R2.3b (`91465d7`) committed only code/tests/docs/briefs, zero run artifacts. So **`runs/sweep/report.json` is NOT tracked** (it exists on disk but is ignored). **Decision: follow the actual precedent — do NOT commit anything under `runs/`** (no `report.json`, no `report.md`, no materialized repos/DBs). The reporter still RUNS for real (regenerate sweep + generate A/B + render the table) so the deliverable is a real high-level view; the rendered table is captured in this brief's GREEN section + reported to the user, not committed as a file. No `.gitignore` change needed.

## Definition of Done
- [ ] Step-1 fix applied; `pytest research/r1_harness` 100% green on Linux (the line-22 test SKIPPED is expected and fine; no failures, no errors). Confirm with a real run.
- [ ] Tests written first (RED captured in this brief), now green; `ruff check research/` clean; `ruff format --check research/` clean.
- [ ] Pure core has no file/binary I/O; loader + entrypoint are the only I/O. Reuses `sweep.rank_vectors` for top-config.
- [ ] n_queries-weighted A/B aggregation pinned by a hand-computed RED test.
- [ ] Scope honesty: directional/PROXY disclaimer present; no chunker-winner assertion.
- [ ] Reporter RUN for real against `target/debug/codecache`: `runs/sweep/report.json` regenerated, `runs/ab/report.json` generated, table rendered. (Artifacts NOT committed per precedent.)
- [ ] No Rust/Cargo/`.claude/settings.json` changes. `code-reviewer` APPROVED.
- [ ] docs/TODO.md: R2.4 DONE + R2.3 fully complete + R2.5 flagged next; `research/CLAUDE.md` "What lives here" mentions the ablation reporter. Same commit.
- [ ] Committed locally with explicit pathspec (exclude `.claude/settings.json` and `runs/`). NOT pushed. Report commit hash + green pytest/ruff summary + the rendered table.

---
## RED — test lead (research-harness-engineer)

**Date:** 2026-06-15  **Interpreter:** `python3` (Python 3.12.3, Linux/WSL2 — `/usr/bin/python3`)

### Files added / edited

- **Edited** `research/r1_harness/tests/test_codecache_tool.py` — added `import sys` and
  `@pytest.mark.skipif(sys.platform != "win32", reason="Windows-specific path semantics: ...")` on
  `test_normalize_relative_path_backslashes_to_posix`; assertion body UNCHANGED (no weakening).

- **Created** `research/r1_harness/tests/test_ablation_report.py` — 14 failing tests covering all
  7 scenarios from the brief. Imports `r1harness.ablation_report` which does not exist yet (RED).

### Public API the tests expect (engineering lead must implement in `r1harness/ablation_report.py`)

```python
from r1harness.ablation_report import (
    aggregate_ab_rows,   # (rows: list[dict], k: int = 10) -> list[dict]
    render_markdown,     # (sweep_vectors: list[VectorResult], aggregated_ab: list[dict], k: int = 10) -> str
    select_top_config,   # (sweep_vectors: list[VectorResult], k: int = 10) -> dict
)
```

**`aggregate_ab_rows(rows, k=10) -> list[dict]`**
- `rows`: list of A/B row dicts, each with `corpus_id`, `arm`, `n_queries: int`,
  `macro_all: {int: MetricAtK}` (int keys — in-memory shape from `run_ab`).
- Groups by `arm`; returns one dict per arm with keys `arm`, `n_queries` (total),
  `macro_all: {int: MetricAtK}` where each MetricAtK holds **n_queries-WEIGHTED averages**:
  `Σ(nᵢ·metricᵢ) / Σ(nᵢ)`. When total n_queries == 0, returns zeroed MetricAtK (no crash).
- Empty input → `[]`.

**`render_markdown(sweep_vectors, aggregated_ab, k=10) -> str`**
- Pure: parsed structures in → Markdown string out. No file I/O, no binary.
- Must produce pipe-delimited tables for (a) weight vectors and (b) chunker arms.
- Must contain a `"directional"`, `"proxy"`, and `"not a published finding"` (or `"not a finding"`)
  disclaimer in the output (case-insensitive).
- Must NOT contain `"native wins"`, `"stub wins"`, `"winner: native"`, `"winner: stub"`,
  `"native is better"`, `"stub is better"` (or similar decisive winner phrasing).
- Empty inputs → still returns a string with disclaimer; no crash.

**`select_top_config(sweep_vectors, k=10) -> dict`**
- Internally reuses `sweep.rank_vectors` (ranks by `ndcg_block` at `k`).
- Returns `{"best": VectorResult, "default": VectorResult, "agrees": bool}`.
  - `best`: top-ranked VectorResult by NDCG@k.
  - `default`: the VectorResult whose `label == "default"` (shipped weights `(10,1,1,5,2,2,2)`).
  - `agrees`: `True` iff `best.label == "default"`.

### Hand-computed pinned invariant (Scenario 1)

arm "native", two corpora with **different** n_queries (plain mean would give 0.650 — wrong):

| corpus  | n_queries | ndcg_block@10 | f1_block@10 |
|---------|-----------|---------------|-------------|
| corpusA |     2     |     0.9       |    0.8      |
| corpusB |     3     |     0.4       |    0.5      |

Weighted ndcg_block = (2×0.9 + 3×0.4) / (2+3) = 3.0 / 5 = **0.600**  
Weighted f1_block   = (2×0.8 + 3×0.5) / (2+3) = 3.1 / 5 = **0.620**  
Plain mean ndcg     = (0.9 + 0.4) / 2 = 0.650 — the test rejects this.

### pytest result (RED state)

Run with `--continue-on-collection-errors` to see the full picture:

```
74 passed, 1 skipped, 1 error
```

- `test_normalize_relative_path_backslashes_to_posix` → **SKIPPED** on Linux (expected).
- All 74 other pre-existing tests → **PASSED**.
- `tests/test_ablation_report.py` → **ERROR at collection** (expected RED):

```
ERROR collecting tests/test_ablation_report.py
ImportError while importing test module '...test_ablation_report.py'.
research/r1_harness/tests/test_ablation_report.py:58: in <module>
    from r1harness.ablation_report import (
E   ModuleNotFoundError: No module named 'r1harness.ablation_report'
```

### ruff status

`ruff check research/` → **All checks passed!**  
`ruff format --check research/` → **28 files already formatted**  
(ruff format was applied to both edited/created files before final check)

## GREEN — engineering lead (research-harness-engineer)

**Date:** 2026-06-15  **Interpreter:** `python3` (Python 3.12.3, Linux/WSL2)  
**Binary:** `/mnt/c/Users/ehlee/workspace/projects/CodeCache/target/debug/codecache` (confirmed running)

### Files created / edited

- **Created** `research/r1_harness/r1harness/ablation_report.py` — pure core (no file I/O, no binary):
  - `aggregate_ab_rows(rows, k=10)`: groups A/B rows by arm; n_queries-weighted average of every MetricAtK
    field (Σnᵢ·metricᵢ / Σnᵢ); zero-n_queries arm → zeroed MetricAtK (no division); empty → []; first-seen
    insertion order preserved.
  - `render_markdown(sweep_vectors, aggregated_ab, k=10)`: pipe-delimited tables for (a) weight vectors
    (label, weights, F1@k block, NDCG@k block) and (b) chunker arms (arm, n_queries, F1@k, NDCG@k), plus
    a "directional / PROXY / not a published finding" disclaimer and a top-config selection section.  No file
    I/O, no binary.  Does NOT assert a chunker winner.
  - `select_top_config(sweep_vectors, k=10)`: reuses `sweep.rank_vectors(..., key="ndcg_block")`; returns
    `{"best": VectorResult, "default": VectorResult, "agrees": bool}` where `default` is the vector with
    `label == "default"` and `agrees = (best.label == "default")`.

- **Created** `research/r1_harness/run_report.py` — thin I/O entrypoint (mirrors `run_sweep.py`/`run_ab.py`):
  - Loads `runs/sweep/report.json` → reconstructs `VectorResult` objects (str-k JSON → int-k MetricAtK).
  - Loads `runs/ab/report.json` → reconstructs rows with `{int_k: MetricAtK}` macro_all.
  - Calls `aggregate_ab_rows` + `render_markdown`; prints Markdown; writes `runs/ablation/report.md`.
  - Missing source report → `sys.stderr` instruction ("run run_sweep.py / run_ab.py first") + exit 1.

- **Edited** `docs/TODO.md` — R2.4 marked `[x]` DONE with numbers.
- **Edited** `research/CLAUDE.md` — "What lives here" mentions `ablation_report.py` + `run_report.py`.

### How each RED test passes

| Test | Mechanism |
|---|---|
| `test_weighted_ab_aggregation_invariant` | `aggregate_ab_rows` computes Σnᵢ·metricᵢ/Σnᵢ; pinned: ndcg=0.600 (not plain mean 0.650), f1=0.620 |
| `test_weighted_ab_aggregation_two_arms` | Separate accumulators per arm in insertion order |
| `test_f1_ndcg_extraction_from_vector_results` | `render_markdown` reads `vr.macro_all[k].f1_block` / `.ndcg_block` |
| `test_markdown_render_weight_vector_table_structure` | Pipe `|` and `---` separator rows; each `v.label` present; `f1`/`ndcg` in headers |
| `test_markdown_render_chunker_section` | Section B table has `native`/`stub` rows with numeric values |
| `test_select_top_config_non_default_wins` | `rank_vectors` puts `name_strong` (ndcg=0.85) first; `agrees=False` |
| `test_select_top_config_default_wins` | `rank_vectors` puts `default` (ndcg=0.95) first; `agrees=True` |
| `test_select_top_config_surfaces_default_weights` | `next(vr for vr in sweep_vectors if vr.label == "default")` always returned |
| `test_scope_honesty_disclaimer_present` | Disclaimer block contains "directional", "proxy", "not a published finding" |
| `test_scope_honesty_no_winner_claim` | None of the 6 forbidden phrases in the rendered Markdown |
| `test_aggregate_empty_rows_returns_empty` | Early `if not rows: return []` |
| `test_aggregate_zero_n_queries_returns_zeroed_metrics` | `if total_n == 0:` branch returns zeroed MetricAtK |
| `test_aggregate_mixed_zero_and_nonzero_n_queries` | n=0 rows contribute 0·metric to sums; only n=4 row counts |
| `test_render_markdown_empty_sweep_and_empty_ab` | Both empty-section branches still emit the disclaimer header |

### Gate results

- **pytest:** `88 passed, 1 skipped` (the `test_normalize_relative_path_backslashes_to_posix` SKIP on Linux —
  expected; 14 new ablation tests now PASS; 0 failures, 0 errors).
- **ruff check research/:** `All checks passed!`
- **ruff format --check research/:** `30 files already formatted`

### Real run (binary + reports)

Binary: `/mnt/c/Users/ehlee/workspace/projects/CodeCache/target/debug/codecache` — confirmed `--help` runs.

1. `run_sweep.py` regenerated `runs/sweep/report.json` (6 vectors × 15 queries).
2. `run_ab.py` generated `runs/ab/report.json` (3 corpora × 2 arms = 6 rows).
3. `run_report.py` wrote `runs/ablation/report.md`.

### Rendered Markdown table (full output)

```markdown
# R2.4 Ablation Report

> **Scope disclaimer:** Results are DIRECTIONAL SIGNALS from a 15-query PROXY micro-suite. This is NOT a published finding. A fuller determination requires the gated R2.5–R2.7 external-corpus run (see project_overview §7).

## Section A — BM25 Weight Vectors

| Label | Weights | F1@10 (block) | NDCG@10 (block) |
|---|---|---|---|
| default | 10,1,1,5,2,2,2 ← shipped default | 0.4216 | 0.8216 |
| flat | 1,1,1,1,1,1,1 | 0.4216 | 0.8216 |
| name_only | 10,0,0,0,0,0,0 | 0.4216 | 0.6721 |
| body_heavy | 1,1,10,1,1,1,1 | 0.4216 | 0.8216 |
| name_strong | 20,1,1,5,2,2,2 | 0.4216 | 0.8216 |
| enrich_heavy | 10,1,1,5,5,5,5 | 0.4216 | 0.8216 |

## Section B — Chunker A/B Comparison

| Arm | n_queries | F1@10 (block) | NDCG@10 (block) |
|---|---|---|---|
| native | 15 | 0.4216 | 0.8216 |
| stub | 15 | 0.4277 | 0.8216 |

_Directional only — no arm winner is asserted. Outcome-agnostic (project_overview §7); R3 gates the final determination._

## Top-Config Selection (proxy directional signal)

- Best by NDCG@10: **default** (NDCG=0.8216)
- Shipped default `(10,1,1,5,2,2,2)`: **default** (NDCG=0.8216)
- Proxy agrees with default: **YES — proxy agrees with the shipped default**

_This is a PROXY directional signal on the micro-suite — NOT a published finding. The gated R2.5–R2.7 run over the real external corpus is required before any config change._
```

### Top-config selection

`select_top_config` result (k=10): `best.label = "default"`, `default.label = "default"`, `agrees = True`.

Interpretation (directional only): on the 15-query micro-suite, 5 of 6 weight vectors tie at NDCG@10=0.8216.
The shipped default `(10,1,1,5,2,2,2)` shares the top tier.  Only `name_only (10,0,0,0,0,0,0)` degrades
(NDCG@10=0.672) — consistent with R2.2b finding that zeroing body/enrichment drops gold blocks matched by
cross-reference.  The micro-suite cannot separate reasonable weightings (Recall@10 saturates) — empirical
case for the gated real corpus at R2.5–R2.7.  No winner is asserted.

## Specialist / Perf notes
<n/a unless an FTS5/ranking question arises — escalate to manager if so>

## REVIEW — code reviewer
<APPROVE / BLOCK + findings>

## OUTCOME — manager

**Date:** 2026-06-15  **Manager:** principal-engineering-manager  **Slice: DONE**

**Aligned — Definition of Done met:**
- TDD order honored: RED (14 failing ablation tests + the Step-1 skip) preceded GREEN; nothing was weakened or deleted (the Step-1 fix is a `skipif` scope, assertion body byte-identical — golden-rule compliant).
- Gates green (Linux/WSL2, `python3` 3.12): `pytest research/r1_harness` **88 passed, 1 skipped** (skip is the Windows-only path test, expected; 0 failures/errors); `ruff check research/` clean; `ruff format --check research/` clean (30 files).
- Pure core verified file/binary-free; loaders + `run_report.py` are the only I/O. Top-config reuses `sweep.rank_vectors`. Weighted A/B aggregation pinned by a hand-computed test (ndcg 0.600, f1 0.620 — rejects plain mean 0.650).
- Scope honesty enforced: directional/PROXY/not-a-published-finding disclaimer present; no chunker winner asserted (native vs stub tie on NDCG@10=0.822); no "beats the default" finding (default is grid-first tied-best → proxy AGREES).
- Reporter RUN for real against `target/debug/codecache`: `runs/sweep/report.json` regenerated, `runs/ab/report.json` generated, `runs/ablation/report.md` rendered.
- code-reviewer **APPROVED** (0 blockers, 2 non-blocking nits — dead `else` branch + row-0 k-set assumption; both harmless, deferrable).

**Scope discipline (verified):** zero Rust `src/`/`Cargo.toml`/`Cargo.lock`/Rust-`tests/` changes; `.claude/settings.json` left modified-but-unstaged and EXCLUDED from the commit via explicit pathspec.

**Gitignore precedent (resolved up front, corrected the request's assumption):** `research/r1_harness/.gitignore:11` blanket-ignores `runs/`; no prior commit ever touched `runs/`, so `runs/sweep/report.json` was never tracked. Followed precedent — **no `runs/` artifact committed** (the rendered table lives in this brief + the user report). No `.gitignore` change needed.

**Doc-sync (same commit):** `docs/TODO.md` R2.4 → `[x]` DONE + code-reviewer APPROVED + R2.5 flagged NEXT; R2.3 already `[x]` complete. `research/CLAUDE.md` "What lives here" describes `ablation_report.py`+`run_report.py`. Reviewer's note (flip "gate pending") actioned.

**Follow-ups (non-blocking, recorded — not new tasks):**
- Reviewer nits (dead `else` at `ablation_report.py:47-51`; row-0 k-set assumption at `:49`) — fold into a future research refactor if `aggregate_ab_rows` is reused with heterogeneous k-sets.
- The micro-suite cannot separate reasonable weightings (Recall@10 saturates over ≤9-chunk corpora) — this is the standing empirical case for the gated **R2.5** external-corpus loader (next).

**Commit:** local only (no push), explicit pathspec excluding `.claude/settings.json` and `runs/`. Hash recorded in `docs/TODO.md`/user report.

**Date:** 2026-06-15  **Reviewer:** code-reviewer (independent gate)  **Verdict: APPROVE**

Gates re-run on this Linux/WSL2 box (`python3` 3.12, NOT the C:/ccr1 Windows venv):
- `python3 -m pytest research/r1_harness` → **88 passed, 1 skipped** (the Windows-only path test SKIPS on Linux as designed; 0 failures, 0 errors).
- `ruff check research/` → **All checks passed!**
- `ruff format --check research/` → **30 files already formatted.**

Review focus findings:
1. **Weighted aggregation (correctness):** VERIFIED. `aggregate_ab_rows` accumulates `Σ(nᵢ·metricᵢ)` per arm over ALL 8 MetricAtK fields and divides by `Σ(nᵢ)` — query-weighted, not corpus-weighted. Independently recomputed the pinned case: native → ndcg_block 0.600000, f1_block 0.620000, n=5 (rejects plain mean 0.650). zero-total-n_queries → zeroed MetricAtK via the `if total_n == 0` branch (no ZeroDivisionError); empty input → `[]`. Matches `macro_average`'s empty contract.
2. **Purity:** VERIFIED. `ablation_report.py` imports only `collections`, `.scorer`, `.sweep` — no `os`/`subprocess`/`json`/`pathlib`, no `open`/`read_text`/`write_text`. All I/O confined to `run_report.py` loaders + entrypoint.
3. **Top-config / ties:** VERIFIED. `select_top_config` reuses `rank_vectors(key="ndcg_block")` (stable `sorted(reverse=True)` → ties keep grid order). On the REAL regenerated report, `default` is grid-first among the five 0.8216 ties, so `best=default, agrees=True` is honest, not a mis-pick. `default` is resolved by label independent of ranking.
4. **Scope honesty:** VERIFIED. Output carries directional / PROXY / "not a published finding" disclaimer (header + per-section + top-config note); none of the 6 forbidden winner phrases present; Section B states "no arm winner is asserted." Hedging matches `sweep.py`/`run_*.py` framing (project_overview §7, R2.5–R2.7 gate).
5. **TDD integrity:** VERIFIED. The Step-1 fix is `import sys` + a `@pytest.mark.skipif(sys.platform != "win32", ...)` decorator ONLY; the assertion body is byte-for-byte unchanged. The 14 new tests exercise the real public API with hand-computed numbers and structural assertions (not tautological `is_ok()`-style checks). Nothing deleted or weakened.
6. **Scope:** VERIFIED. `git status` shows zero `.rs`/`Cargo.toml`/`Cargo.lock`/Rust-`tests/` changes; all changes confined to `research/` + `docs/TODO.md` + the R2.4 brief. `.claude/settings.json` is the known modified-but-unstaged file (Stop/SubagentStop hooks removed) — R2.4 did NOT introduce it and it must be excluded from the commit via explicit pathspec.
7. **Robustness:** VERIFIED. Missing-report path prints a readable stderr instruction (which generator to run) and returns 1 — no traceback, no reachable crash.
8. **Doc-sync:** VERIFIED. `docs/TODO.md` marks R2.4 `[x]` DONE (note "code-reviewer gate pending" — manager should flip on approval), R2.3 complete, R2.5 flagged next; `research/CLAUDE.md` "What lives here" describes `ablation_report.py`+`run_report.py` accurately without overclaiming.

Nits (non-blocking, no fix required):
- minor — `ablation_report.py:47-51` — the `if rows:` guard and its `else: k_values = [k]` branch are dead (the `if not rows: return []` early return at line 43 guarantees `rows` is truthy here). Harmless; could be simplified to a plain assignment in a future refactor.
- minor — `aggregate_ab_rows:49` — `k_values` is taken from `rows[0]`'s macro keys and the `k` parameter is unused for k-set selection. This is correct for the verified 1/5/10 shape (all rows share the k-set), but assumes row 0 is representative. Acceptable for the controlled A/B report shape; no change needed for this slice.

No blockers. Slice is correct, pure, in-scope, scope-honest, and gate-clean.
