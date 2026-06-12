# src/formatter/ — CLAUDE.md

**Module:** `formatter` · **Owner:** `principal-engineering-lead` · **Milestone:** M7 (stub at M0).

## Purpose
Serialize query results to TOON (`file:start-end` line pairs), JSON (programmatic), and plain
text (CLI display). Line ranges come from the stored `start_line`/`end_line` (**Decision Log
D7**) so no source re-read is needed at query time (preserves the §11.2 formatting budget).

## API anchor
`docs/project_plan.md` §6.4 (output formats).

## Shipped API (M7.1)
```rust
pub enum Format { Toon, Json, Text }                          // Debug+Clone+Copy+Default(=Text)+Eq
pub fn format(result: &QueryResult, query: &str, fmt: Format) -> String
```
Pure `QueryResult -> String` (D4): **no I/O, no file reads**. `query` is a parameter (it is not a
field on `QueryResult`). Dispatch lives in `mod.rs`; one private serializer module per format
(`toon`/`json`/`text`, each a `pub(super) fn render`).

### Format shapes
- **TOON** (`toon.rs`) — locator-only: one `<file>:<start>-<end>` line per chunk in incoming BM25
  order (no re-sort, no bodies/headers); pipes straight to `cat`/an editor. Empty → empty string.
  Ranges from stored `start_line`/`end_line` (D7), `file_path` via `to_string_lossy()`.
- **JSON** (`json.rs`) — §6.4.2 schema via a format-local DTO (`JsonResult`/`JsonChunk`, private);
  **no serde derives on `types::Chunk`** (transport separation, D4/D5). Keys `query`,
  `total_results` (from `total_results_found`), `total_tokens`, `chunks[]`; each chunk carries
  `symbol_name`, `symbol_type`, `file_path`, `start_byte`, `end_byte`, `language`, `bm25_score`,
  `chunk_text`. Pretty (2-space); round-trips. Infallible serialize fallback to `"{}"` (no panic).
- **Text** (`text.rs`, default) — ASCII (no emoji): 56-char `─` rules framing a `Query: "…"` +
  `Found N results (showing top M, T tokens)` header, then `[n] <qualified> (<type>) file:s-e
  (score: …)` blocks (score `{:.2}`), each followed by the full `chunk_text`. **D13 agent-first**:
  qualified parent (`parent_symbol.symbol_name` when present) + range + one-line signature (first
  line of `chunk_text`) precede the body. Empty → header + closing rule, no `[n]` blocks.

## Tests / scenarios
`tests/formatter_tests.rs` (6 golden tests) + committed goldens `tests/fixtures/golden/query_{basic,empty}.{toon,json,txt}`.
`docs/TEST_STRATEGY.md#formatter` — golden outputs for TOON/JSON/text; JSON valid + round-trips;
`file:line` pairs correct.

## Status
M7.1 DONE (2026-06-12): all three serializers shipped + green; reviewer APPROVED. CLI (`cli`) that
drives this formatter lands in M7.2–M7.4.
