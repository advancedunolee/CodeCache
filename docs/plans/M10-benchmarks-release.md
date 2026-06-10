# M10 — Benchmarks + Release

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m10--benchmarks--release),
> [`../project_plan.md`](../project_plan.md) §1.3 / §5.4 / §9.3 / §10.3 / §11, [`../ENGINEERING_PLAN.md`](../ENGINEERING_PLAN.md) §5.

## Goal / acceptance criteria
Land the full criterion suite measured against every budget, run the token-reduction benchmark,
and ship `v0.1.0`. **Exit (from ROADMAP):**
- [ ] Budgets met: **p95 < 500ms**, **index < 100MB**, **incremental < 2s**, **≥ 40% token reduction**.
- [ ] `v0.1.0` tagged and published (crates.io); install smoke test passes.

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `benches/indexing_bench.rs` | Cold/incremental index timing + index size. | perf |
| `benches/query_bench.rs` | p50/p95/p99 query latency (finalize M6 skeleton). | perf |
| `benches/hashing_bench.rs` | xxHash3 throughput vs §5.4 (1K files < 500ms). | perf |
| `benches/token_reduction.rs` (or `examples/`) | 5-task token-reduction benchmark (§9.3). | perf |
| `examples/django_benchmark/` | Sample codebase / fixture for size + latency (§10.4). | perf |
| `.github/workflows/release.yml` | Tag → build → crates.io publish + artifacts. | devops |
| `.github/workflows/bench.yml` | Scheduled bench run (gate-tracking — §ENG_PLAN §5). | devops |
| `benches/CLAUDE.md` | Bench inventory + budgets table. | manager |
| `README.md`, `docs/CLAUDE_CODE_SETUP.md` | Quickstart + MCP setup (§9.1 deliverables). | manager/devops |

## Dependencies
- **Prior:** **all** milestones (M0–M9) complete and green. Benches need a working
  index+query+parse path across all three languages.
- Crates: `criterion` (already §10.3). Token-reduction benchmark compares against a baseline
  "dump whole files" measurement (§8.5 / §9.3) — uses the §6.3 char-based token estimate for
  consistency (document that it's an estimate, not a real tokenizer).

## Ordered slices

### Slice M10.1 — criterion suite vs perf budgets
- **PERF (perf engineer):**
  - `indexing_bench`: cold 10K LOC **< 5s**, 100K LOC **< 30s** (§5.4); record index size
    on Django-scale **< 100MB** (§1.3, §4.2 estimates ~6MB).
  - `incremental`: modify 10 files, total re-index **< 2s** (§1.3/§5.4).
  - `query_bench`: p95 **< 500ms** on 100K LOC cold cache (§1.3/§11.2).
  - `hashing_bench`: 1K files **< 500ms** (§5.4).
- **REVIEW:** budgets asserted/tracked; numbers recorded in `benches/CLAUDE.md` + the brief.
  Where a budget can't be a hard CI assert (machine variance), track trend in `bench.yml` and
  fail on large regressions.

### Slice M10.2 — token-reduction benchmark (≥ 40%)
- **PERF:** 5 real-world function-lookup tasks (§1.3/§9.3). For each, measure tokens for
  (a) naive full-file/dir dump vs (b) `codecache query --max-tokens ...` output. Assert
  **average reduction ≥ 40%** (§9.3 pass criterion; §1.3 target). Document tasks + method so
  it's reproducible (the §8.5 JWT example is the template).

### Slice M10.3 — CI bench wiring + parity
- **DEVOPS:** `bench.yml` scheduled (not per-PR, to avoid noise); ensure `ci.yml` still mirrors
  local gates (`../ENGINEERING_PLAN.md` §5). Cache builds (tree-sitter/rusqlite C compile).

### Slice M10.4 — release v0.1.0
- **DEVOPS:**
  - Version bump to `0.1.0` (already in `Cargo.toml` §10.3); changelog.
  - `release.yml`: on tag `v0.1.0` → `cargo publish` (crates.io) + build release binaries
    (Linux/macOS/Windows) as GitHub release artifacts.
  - **Install smoke test:** `cargo install codecache` (or download artifact) → `codecache init`
    → `index` → `query` on a tiny fixture → assert success. Gate the release on this.
- **MANAGER/DEVOPS:** finalize README quickstart + `CLAUDE_CODE_SETUP.md` (MCP config §8.4),
  `CONTRIBUTING.md` (§10.4 deliverable), LICENSE.

## API contracts / data structures
- No new runtime API. Benches consume the public surface (`Indexer`, `Retriever`, CLI binary).
- Release metadata: `Cargo.toml` package fields (description, license, repository, keywords) for
  crates.io — add if missing (devops), no behavior change.

## Performance budgets (the full set — `../project_plan.md` §1.3 / §5.4 / §11)
| Budget | Target | Source |
|---|---|---|
| Query latency p95 | < 500ms (100K LOC, cold) | §1.3, §11.2 |
| Index size | < 100MB (Django, ~450K LOC) | §1.3, §4.2 |
| Incremental re-index | < 2s (10-file change) | §1.3, §5.4 |
| Cold index 10K / 100K LOC | < 5s / < 30s | §5.4 |
| Hash 1K files | < 500ms | §5.4 |
| Token reduction | ≥ 40% avg (5 tasks) | §1.3, §9.3 |
| Memory footprint | ~150MB acceptable | §11.3 |

## Decision Log bindings
- **D1:** token-reduction benchmark may note BM25-only recall gaps on semantic queries (the
  rationale for the deferred v0.2 hybrid path) — informational, not a v0.1 gate.
- **D2/D3/D4:** benches run on real-world code (will include malformed files — D2 must hold) and
  across all transports' shared core (D4).

## Definition of Done (this phase / v0.1 release)
- [ ] M10.1–M10.4 complete; all budgets in the table met or tracked with justification.
- [ ] Token-reduction ≥ 40% demonstrated on 5 documented tasks.
- [ ] `release.yml` + `bench.yml` green; CI still mirrors local gates.
- [ ] `v0.1.0` tagged + published; install smoke test passes on Linux/macOS/Windows.
- [ ] README, `CLAUDE_CODE_SETUP.md`, CONTRIBUTING, LICENSE complete (§9.1 deliverables).
- [ ] clippy/fmt clean repo-wide; reviewer APPROVED; `docs/TODO.md` Phase 10 + `benches/CLAUDE.md` updated.
