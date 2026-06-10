# CodeCache — Roadmap

Milestones for v0.1, each gated by tests. Build order is bottom-up (see
[`ENGINEERING_PLAN.md`](ENGINEERING_PLAN.md) §2). The live, checkable status lives in
[`TODO.md`](TODO.md); this file defines **entry/exit criteria** and the **Decision Log**.

A milestone is *done* only when its exit criteria are met under the Definition of Done
(`ENGINEERING_PLAN.md` §4).

---

## Milestones

### M0 — Scaffolding & CI
- **Entry**: empty repo.
- **Work**: `cargo` project per §10.4 layout; `Cargo.toml` deps per §10.3; root + per-dir
  `CLAUDE.md`; CI (`ci.yml`: fmt/clippy/test) green on an empty `lib.rs`.
- **Exit**: `cargo build`/`cargo test` run; CI green; hooks fire on `.rs` edits.

### M1 — `config` + `storage` (SQLite schema + FTS5)
- **Work**: `.codecache/config.toml` load/validate; SQLite schema (`symbols` FTS5,
  `files_metadata`, `index_state`); create/migrate; CRUD; FTS5 virtual table with `bm25()`.
- **Exit**: round-trip insert/query/delete tested; FTS5 `MATCH` + `bm25()` ordering tested;
  schema creation idempotent.

### M2 — `hasher`
- **Work**: xxHash3-128 of file contents; compare against cached hash for change detection.
- **Exit**: stable hashes; change/no-change detection tested; large-file and binary handling.

### M3 — `parser` (Python first)
- **Work**: load `tree-sitter-python`; `LanguageConfig`; run `.scm` queries to extract
  function/class/method nodes with exact byte spans; ERROR-node detection.
- **Exit**: byte spans correct on fixtures incl. nested/async/decorated symbols; malformed
  files don't panic (degradation path exercised — Decision Log #2).

### M4 — `chunker`
- **Work**: turn AST nodes into `Chunk`s with metadata enrichment (`parent_symbol`,
  `file_docstring`, `imports`, `cross_references` — Decision Log #3).
- **Exit**: non-overlapping chunks within file bounds (property test); enrichment fields populated.

### M5 — `indexer`
- **Work**: file discovery honoring `.gitignore`/ignore patterns; orchestrate
  parse→chunk→hash→store; incremental update (only changed files).
- **Exit**: full index of a fixture repo; re-index unchanged = no-op (idempotent); change 1
  file ⇒ only it re-indexed; e2e `init→index` green.

### M6 — `retriever`
- **Work**: FTS5 BM25 search; snippet extraction; token counting; greedy token-budget packing.
- **Exit**: ranking deterministic; `--max-tokens` respected exactly; empty/no-match queries
  handled; latency bench wired (perf engineer).

### M7 — `formatter` + `cli`
- **Work**: TOON/JSON/text formatters; `clap` commands `init/index/update/query/status/config/serve`.
- **Exit**: golden-output tests per format; CLI arg parsing + error messages tested; e2e
  `init→index→query` through the binary.

### M8 — `mcp_server`
- **Work**: stdio JSON-RPC MCP adapter; tool registration; `serve` command.
- **Exit**: protocol handshake + tool-call round-trip tested against a mock client.

### M9 — TypeScript + Go parsers
- **Work**: add `tree-sitter-typescript` and `tree-sitter-go` configs + queries.
- **Exit**: per-language fixture suites green; language coverage = Python/TS/Go.

### M10 — Benchmarks + Release
- **Work**: criterion suite vs all budgets; token-reduction benchmark (5 tasks); release
  workflow; crates.io publish.
- **Exit**: budgets met (p95<500ms, index<100MB, incr<2s, ≥40% token reduction); `v0.1.0`
  tagged and published; install smoke test passes.

---

## Decision Log

Design critiques raised during review of the original spec, with their disposition. The
manager updates this as decisions are made.

### D1 — Hybrid retrieval (AST+BM25 → optional embeddings)  · **Deferred to v0.2**
AST+BM25 misses ~30–40% of semantic queries ("find all error-handling patterns"). v0.1 ships
BM25-only; a `--enable-embeddings` flag may log a low-recall warning. Plan: optional
CodeBERT/UniXcoder index in v0.1.5, hybrid default in v0.2. Keep `Retriever` behind a trait so
a `HybridRetriever` can wrap it without churn. *Cost: +2 wks, +~3GB/index.*

### D2 — Graceful Tree-sitter degradation  · **Adopted for v0.1**
Tree-sitter produces `ERROR` nodes on 5–15% of real files. Count ERROR nodes; above a
threshold (~20%) fall back to heuristic/regex chunking and mark chunks `heuristic` in metadata.
Indexing must never hard-fail on malformed input. Owned by `rust-treesitter-specialist`;
enforced by M3/M5 tests. *Cost: +~1 wk, +~200 LOC.*

### D3 — Chunk metadata enrichment  · **Adopted for v0.1**
Extend `Chunk` with `parent_symbol`, `file_docstring`, `imports`, `cross_references` (indexed
in FTS5) to lift recall on indirect queries. Extracted during AST traversal in M4. *Cost:
~+1MB/index (negligible).*

### D4 — Integration decoupling (HTTP / LSP beyond MCP)  · **Partially deferred**
Avoid single-vendor lock-in on MCP. v0.1: keep the retrieval core transport-agnostic so a thin
HTTP REST adapter (`codecache serve --http`) can be added without refactoring; document an LSP
path. Full HTTP API hardening and LSP land in v0.2. *Cost: +~1 wk (HTTP), +~2 wks (LSP, v0.2).*

---

## Clarifications raised during phase planning (`docs/plans/`)

These were surfaced while writing the per-milestone phase plans. They refine — not contradict —
the spec; where a public API or schema is affected, `project_plan.md` is updated **first**.

**Ratified 2026-06-09** (manager, during M0 kickoff): D5–D8 dispositions below are final for
v0.1 and now reflected in `project_plan.md`. D5–D7 affect M0/M1 and were ratified before any
code was written, per the "change the plan before diverging" rule. The M0 scaffolding therefore
declares a `types` module in `src/lib.rs` (D5); D6/D7 are realized by M1's schema + M4/M5's
populate logic; D8 is realized at M8.

### D5 — Shared core types location  · **Ratified for v0.1** (plan: M0/M1) — *spec: §4.3, §3.2.1*
`Chunk`, `Language`, `SymbolType` (and `FileMeta`) live in a dependency-free `crate::types`
module rather than inside `parser`, so `storage` need not depend on `parser` and the bottom-up
build order (`ENGINEERING_PLAN.md` §2) stays acyclic. **M0 action:** `src/lib.rs` declares
`pub mod types;` and `src/types/mod.rs` is created as an (initially empty) stub module.

### D6 — `files_metadata` write signature  · **Ratified for v0.1** (plan: M1, M5) — *spec: §3.2.2*
`update_file_hash(file_path, meta: &FileMeta)` takes a `FileMeta { content_hash, mtime,
file_size, language, chunk_count }` so M5's incremental indexer persists every §4.1 column in
one call. `project_plan.md` §3.2.2 updated to match.

### D7 — Store line numbers at index time  · **Ratified for v0.1** (plan: M7, affects M1/M4/M5) — *spec: §3.2.2, §4.1, §4.3*
`symbols` gains `start_line`/`end_line` UNINDEXED columns and `Chunk` gains `start_line`/
`end_line` (1-based, inclusive), populated at index time. This lets the TOON/text formatters
emit `file:start-end` line ranges without re-reading source at query time (preserves the §11.2
budget). Ratified before M1 ships to avoid a later schema migration.

### D8 — MCP server resource ownership  · **Ratified for v0.1** (plan: M8) — *spec: §3.2.2, §3.2.3, §8.3*
`Storage` wraps `Arc<Mutex<rusqlite::Connection>>` (Connection is not `Clone`); cloning
`Storage` is a cheap Arc clone, so the MCP server lends the same connection to both `Retriever`
and `Indexer`. Single-writer semantics are preserved by the Mutex. `project_plan.md`
§3.2.2/§3.2.3/§8.3 updated to match. No M0 action (module boundary already exists).

**Ratified 2026-06-10** (during M0 build verification): D9–D10 below correct two issues caught
by the first real `cargo build` (the toolchain was installed locally this session).

### D9 — rusqlite FTS5 feature  · **Ratified for v0.1** (plan: M0, affects M1) — *spec: §10.3*
rusqlite 0.32 has **no `fts5` cargo feature**; FTS5 is compiled into the `bundled` SQLite
amalgamation by default. The original `features = ["bundled", "fts5"]` failed dependency
resolution. Corrected to `features = ["bundled"]` in both `Cargo.toml` and `project_plan.md`
§10.3. FTS5 availability is proven by M1's first `CREATE VIRTUAL TABLE ... USING fts5`.

### D10 — Toolchain/MSRV bump 1.82.0 → 1.85.0  · **Ratified for v0.1** (plan: M0, affects CI + all phases) — *spec: §10.3 (`edition`)*
The 1.82.0 pin was a planning-time guess (Oct 2024). With no committed `Cargo.lock`, cargo
resolved transitive deps to latest, and `hashbrown 0.17` (pulled in via `toml`/`indexmap`) now
requires **edition 2024**, which Cargo only understands from **1.85** onward; `cargo build`
fails on 1.82 with *"feature `edition2024` is required ... not stabilized in this version of
Cargo (1.82.0)"*. Compounding this, 1.82 predates the **MSRV-aware dependency resolver**
(stabilized in 1.84), so it cannot auto-select MSRV-compatible deps and would re-break on every
`cargo update`.

**Decision (Path A — bump the pin).** Rejected the alternative of staying on 1.82 with a
hand-pinned `Cargo.lock` holding every too-new transitive dep down: that is fragile whack-a-mole
that fights the ecosystem and, lacking the 1.84+ resolver, re-breaks indefinitely. We pin a
**deliberate MSRV of 1.85.0** — the exact floor that (a) stabilizes edition 2024 (what
`hashbrown 0.17` demands) and (b) ships the MSRV-aware resolver — rather than chasing latest
stable (1.96.0). This keeps our MSRV as conservative as dependency reality allows, gives
downstream consumers a meaningful compatibility contract, and lets the `rust-version = "1.85"`
key in `Cargo.toml` hold transitive deps to 1.85-compatible versions.

**Disposition.** `rust-toolchain.toml` `channel = "1.85.0"` is the **single source of truth**;
`Cargo.toml` `rust-version = "1.85"` is the MSRV; CI honors the toolchain file (so local == CI
parity is unchanged — same gates, same flags). `project_plan.md` §10.3 keeps `edition = "2021"`
for our own crate (the edition-2024 requirement is a *transitive dependency's*, not ours).
Owner of `rust-toolchain.toml` + `ci.yml` + `.github/CLAUDE.md`: `devops-release-engineer`;
ROADMAP/ENGINEERING_PLAN/phase-plan edits: manager. A generated `Cargo.lock` is committed
(ROADMAP follow-up R1) so the resolved versions are reproducible.

---

## Deferred to v0.2+ (from project_plan §9.2)
Embeddings retrieval (D1), call-graph analysis, additional languages (Rust/Java/C++), real-time
file watching, web UI, multi-repo support, full HTTP/LSP integrations (D4).
