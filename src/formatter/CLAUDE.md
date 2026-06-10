# src/formatter/ — CLAUDE.md

**Module:** `formatter` · **Owner:** `principal-engineering-lead` · **Milestone:** M7 (stub at M0).

## Purpose
Serialize query results to TOON (`file:start-end` line pairs), JSON (programmatic), and plain
text (CLI display). Line ranges come from the stored `start_line`/`end_line` (**Decision Log
D7**) so no source re-read is needed at query time (preserves the §11.2 formatting budget).

## API anchor
`docs/project_plan.md` §6.4 (output formats).

## Tests / scenarios
`docs/TEST_STRATEGY.md#formatter` — golden outputs for TOON/JSON/text; JSON valid + round-trips;
`file:line` pairs correct.

## Status
M0: empty stub. Implemented at M7.
