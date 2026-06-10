# M0 — Scaffolding & CI

> **EXECUTED 2026-06-10** (kept as historical record per `README.md` maintenance contract).
> All slices M0.1–M0.3 complete; `code-reviewer` APPROVED (conditional on the green gate run,
> finding R1: commit the generated `Cargo.lock`). Hand-off record:
> [`../../.claude/briefs/BRIEF-M0-scaffolding.md`](../../.claude/briefs/BRIEF-M0-scaffolding.md).
>
> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m0--scaffolding--ci),
> [`../project_plan.md`](../project_plan.md) §10.3–10.4, [`../ENGINEERING_PLAN.md`](../ENGINEERING_PLAN.md) §5–7.

## Goal / acceptance criteria
Stand up the Rust workspace and CI so every later milestone has a green baseline and the
quality gates actually fire. **Exit (from ROADMAP):**
- [ ] `cargo build` and `cargo test` run on an empty `lib.rs` (no production logic yet).
- [ ] `Cargo.toml` carries the §10.3 dependency set (declared, may be unused behind features).
- [ ] Root + every per-directory `CLAUDE.md` exists.
- [ ] CI workflow `ci.yml` (fmt → clippy → test) is green.
- [ ] Local hooks fire on `.rs` edits (`fmt-on-edit`) and at Stop (`clippy -D warnings`, `test`).

## Modules & files
This milestone creates the skeleton only — module bodies stay empty (`mod.rs` with a doc
comment + `#[cfg(test)] mod tests {}`), one per `../project_plan.md` §3.1.

| Path | Purpose |
|---|---|
| `Cargo.toml` | Package + deps per §10.3; `[lib]` + `[[bin]] name = "codecache"`. |
| `src/main.rs` | Binary entry; calls into `cli` (stub that prints help). |
| `src/lib.rs` | Declares `pub mod {types,cli,indexer,parser,chunker,hasher,storage,retriever,formatter,mcp_server,config};` (`types` per Decision Log D5) + `pub const VERSION`. |
| `src/<module>/mod.rs` | One per module in §3.1; empty stub + test module. |
| `tests/` | Empty integration test dir; placeholder `smoke_test.rs` asserting the crate links. |
| `benches/` | Created in M10; only a `.gitkeep`/note here. |
| `.github/workflows/ci.yml` | fmt-check → clippy `-D warnings` → `cargo test --all`. |
| Per-dir `CLAUDE.md` | `src/`, each `src/<module>/`, `tests/`, `benches/`, `.github/`. |
| `rust-toolchain.toml` | Pin the toolchain (channel `1.85.0`, ROADMAP D10) so local == CI. |

**Layout authority:** `../project_plan.md` §10.4. Do not invent extra top-level dirs.

## Dependencies
- **Prior milestones:** none (entry = empty repo).
- **External crates landing now** (declared in `Cargo.toml`, §10.3): `clap`, `anyhow`,
  `tree-sitter` + `tree-sitter-{python,typescript,go}`, `rusqlite` (`bundled` — FTS5 is in the
  bundled amalgamation; no `fts5` feature exists, ROADMAP D9),
  `xxhash-rust` (`xxh3`), `serde`, `serde_json`, `toml`, `ignore`, `walkdir`, `once_cell`,
  `regex`; dev: `criterion`, `tempfile`, plus `proptest` (test-lead needs it from M4 on — add
  now to keep `Cargo.toml` churn low; record as a deviation note below).

## Ordered slices
Scaffolding is mostly mechanical; keep a thin test-first discipline anyway.

### Slice M0.1 — workspace boots
- **RED (test-lead):** `tests/smoke_test.rs` with `#[test] fn crate_links_and_lib_is_callable()`
  asserting a trivial `codecache::version()` (or a `pub const VERSION`) equals the Cargo
  version. Fails because the symbol does not exist.
- **GREEN (eng-lead):** create `Cargo.toml`, `src/lib.rs` (module declarations + `VERSION`),
  `src/main.rs`, all empty `src/<module>/mod.rs` stubs.
- **REVIEW:** layout matches §10.4; no stray deps; no `unwrap` in `main`.
- **INTEGRATE:** manager scaffolds/verifies every `CLAUDE.md` (delegate to `new-module` skill).

### Slice M0.2 — CI parity with local gates
- **RED:** n/a (infra). Instead: devops adds `ci.yml`; a deliberate `cargo fmt`-violating commit
  on a scratch branch must make CI **red** (proves the gate works), then reverted.
- **GREEN (devops):** `ci.yml` runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all` on stable; cache `~/.cargo` + `target`.
- **REVIEW:** CI steps mirror `../ENGINEERING_PLAN.md` §5 table exactly (same flags).
- **INTEGRATE:** confirm hooks (`.claude/hooks/*.ps1`) now act (they no-op'd pre-`Cargo.toml`).

### Slice M0.3 — CLAUDE.md coverage
- Manager ensures `src/CLAUDE.md` + each `src/<module>/CLAUDE.md` exist with: module purpose,
  its `../project_plan.md` §3.2 API anchor, owner agent, and "tests live in / scenarios in
  TEST_STRATEGY#<module>". No code; doc-only.

## API contracts / data structures
None implemented this milestone. The stubs must *declare* the module names that §3.2 will fill:
`types, config, storage, hasher, parser, chunker, indexer, retriever, formatter, cli, mcp_server`
(`types` added per ratified Decision Log **D5** — the dependency-free home for `Chunk`,
`Language`, `SymbolType`, `FileMeta`; stays an empty stub at M0). `VERSION` is the only public
symbol with a value introduced.

## Performance budgets
None apply at M0. (Budgets begin at M5/M6/M10 — see those plans.) CI build time is the only
practical concern; `rusqlite` `bundled` + tree-sitter compile C — expect a slow cold CI build,
so caching is mandatory in `ci.yml`.

## Decision Log bindings
- **D4** (transport-agnostic core): reflected only as module boundaries here — `mcp_server` is
  its own module separate from `retriever`/`cli`, so an HTTP adapter can be added later without
  touching the core. No logic yet.
- **D9** (rusqlite FTS5): `Cargo.toml` declares `rusqlite = { features = ["bundled"] }` — no
  `fts5` feature; FTS5 ships inside the bundled amalgamation.
- **D10** (toolchain/MSRV bump): caught during M0 build verification — `cargo build` on the
  original 1.82.0 pin fails because a transitive dep (`hashbrown 0.17` via `toml`/`indexmap`)
  requires edition 2024 (Cargo ≥ 1.85). `rust-toolchain.toml` channel and `Cargo.toml`
  `rust-version` move to **1.85.0** / **1.85**; CI is unaffected (it honors the toolchain file).
  A committed `Cargo.lock` (R1) pins the resolved versions.

## Definition of Done (this phase)
- [ ] M0.1–M0.3 slices complete; `cargo build` + `cargo test` succeed locally and in CI.
- [ ] `Cargo.toml` == §10.3 set (deviations recorded in this plan + ROADMAP if any).
- [ ] All required `CLAUDE.md` files present (root, `.claude`, `docs`, `src`, each `src/<m>`, `tests`, `.github`).
- [ ] `clippy --all-targets -- -D warnings` and `fmt --check` clean.
- [ ] Hooks verified firing on a sample `.rs` edit; skills resolve (`/tdd-cycle`, `/new-module`, `/bench`, `/standup`).
- [ ] `code-reviewer` APPROVED; `docs/TODO.md` Phase 0 items checked.

## Deviations from the spec (record here, then in ROADMAP if material)
- `proptest` added to `[dev-dependencies]` at M0 (spec §10.3 lists it implicitly via property
  tests in TEST_STRATEGY but does not enumerate the crate). Rationale: avoid re-touching
  `Cargo.toml` at M4. **Manager sign-off: APPROVED 2026-06-09** per `../ENGINEERING_PLAN.md` §6
  — `proptest` is dev-only (does not enter the release dependency surface) and is required by
  the M4 non-overlap property test in `TEST_STRATEGY.md`; landing it now keeps `Cargo.toml`
  churn low. The `crate::types` stub module (D5) is likewise added to the M0 layout.
