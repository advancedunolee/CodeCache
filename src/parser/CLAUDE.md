# src/parser/ — CLAUDE.md

**Module:** `parser` · **Owner:** `principal-engineering-lead` + `rust-treesitter-specialist`
· **Milestone:** M3 (Python), M9 (TypeScript + Go) · stub at M0.

## Purpose
Tree-sitter integration: load grammars, run `.scm` queries to extract function/class/method
nodes with **exact** byte spans; detect ERROR-node rate and route high-error files to heuristic
fallback (**Decision Log D2** — indexing never hard-fails on malformed input).

## API anchor
`docs/project_plan.md` §3.2.1 (`Parser`, `LanguageConfig`) + §5.3 (per-language queries).

## Tests / scenarios
`docs/TEST_STRATEGY.md#parser-python--ts--go` — exact spans on nested/async/decorated symbols;
ERROR-node rate computed; heuristic fallback exercised; unsupported language → error.

## Status
M0: empty stub. Python at M3, TS/Go at M9.
