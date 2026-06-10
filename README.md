# CodeCache

**Local-first, AST-driven code-context retrieval for terminal AI workflows.**

CodeCache parses your codebase into semantic units (functions, classes, methods) with
Tree-sitter, indexes them in SQLite + FTS5, and retrieves only the relevant snippets at query
time — concentrated, token-budgeted context for AI agents instead of dumping whole files.

- **Deterministic** — AST boundaries, not drifting embeddings.
- **Incremental** — re-index only changed files (xxHash).
- **CLI-native** — `codecache query "find auth logic"`, plus an MCP server for Claude Code.
- **v0.1 scope** — Python, TypeScript, Go. AST + BM25 (embeddings deferred to v0.2).

> Status: **pre-implementation.** The architecture is specified and the engineering process is
> set up; module implementation follows the milestones in [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Quickstart (target UX)
```bash
codecache init                  # create the index database
codecache index .               # build the full index
codecache query "authenticate user" --max-tokens 4000
codecache update src/auth.py    # incremental re-index
codecache serve                 # MCP server for Claude Code
```

## How this project is built
CodeCache is developed **test-first (TDD)** by a coordinated team of Claude Code agents, with
quality gates enforced by hooks and CI. If you're contributing (human or agent), start here:

- [`CLAUDE.md`](CLAUDE.md) — project overview + golden rules.
- [`docs/ENGINEERING_PLAN.md`](docs/ENGINEERING_PLAN.md) — team, build order, TDD workflow, Definition of Done.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestones + decision log.
- [`docs/TEST_STRATEGY.md`](docs/TEST_STRATEGY.md) — the test scenario matrix.
- [`docs/TODO.md`](docs/TODO.md) — what's next.
- [`docs/project_plan.md`](docs/project_plan.md) — full technical spec.

## Build & test
```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo bench
```

## License
TBD.
