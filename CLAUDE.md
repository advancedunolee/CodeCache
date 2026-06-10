# CodeCache — CLAUDE.md (root)

CodeCache is a **local-first, AST-driven code-context retrieval engine** for terminal AI
workflows: a Rust CLI that parses code with Tree-sitter, indexes semantic units in SQLite +
FTS5, retrieves only relevant snippets (BM25, token-budgeted), and serves them to agents via
stdout or an MCP server. v0.1 targets Python/TS/Go. Full spec: [`docs/project_plan.md`](docs/project_plan.md).

## How we work here — read these first
- [`docs/ENGINEERING_PLAN.md`](docs/ENGINEERING_PLAN.md) — team, module ownership, build order, **TDD workflow**, Definition of Done.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — M0–M10 milestones + Decision Log.
- [`docs/TEST_STRATEGY.md`](docs/TEST_STRATEGY.md) — the test scenario matrix.
- [`docs/TODO.md`](docs/TODO.md) — the living checklist (start here to see what's next).

## This project is test-driven (TDD)
Tests are written **first**. Every production line exists to satisfy a failing test. The loop
is red → green → refactor → review → integrate, driven by the `/tdd-cycle` skill. Never weaken
or delete a test to make it pass.

## The agent team (`.claude/agents/`)
- **principal-engineering-manager** — orchestrates, sequences, verifies alignment, owns docs/TODO/CLAUDE.md.
- **principal-test-engineering-lead** — writes failing tests first.
- **principal-engineering-lead** — implements minimum idiomatic Rust to go green.
- **code-reviewer** — independent gate (APPROVE/BLOCK) before done.
- **performance-bench-engineer** — criterion benches + perf budgets.
- **rust-treesitter-specialist** — Tree-sitter grammars/queries + FTS5 tuning.
- **devops-release-engineer** — CI parity with local gates + releases.

Start any non-trivial work by invoking `principal-engineering-manager` for a task brief.
The team operating manual is [`.claude/CLAUDE.md`](.claude/CLAUDE.md).

## Commands
```
cargo build                                   # build
cargo test                                     # run all tests (TDD inner loop)
cargo clippy --all-targets -- -D warnings      # lint gate
cargo fmt                                       # format
cargo bench                                     # perf budgets (see /bench skill)
```

## Quality gates (automated)
Hooks in `.claude/settings.json` run `cargo fmt` on every `.rs` edit and `clippy -D warnings`
+ `cargo test` when a turn/subagent ends (no-op until `Cargo.toml` exists). CI mirrors them.

## Golden rules
- TDD: failing test first, always.
- Match the documented APIs (`project_plan.md` §3.2); change the plan before diverging.
- Code change ⇒ update `docs/TODO.md` + the local module `CLAUDE.md` in the same change.
- No reachable `unwrap()/expect()/panic!`; keep `Cargo.toml` lean (deps per §10.3).
