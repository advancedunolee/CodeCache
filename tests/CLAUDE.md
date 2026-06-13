# tests/ — CLAUDE.md

Integration, end-to-end, and property tests for CodeCache. **Owner agent:**
`principal-test-engineering-lead`. Scenario matrix: [`../docs/TEST_STRATEGY.md`](../docs/TEST_STRATEGY.md).

## Purpose
Cross-module tests that exercise the crate from the outside (the `codecache` library + the
built binary). Per-module unit tests live in each module's `#[cfg(test)] mod tests`; this
directory holds the wider integration/E2E/property surface.

## Layout
| Path | Role | Milestone |
|---|---|---|
| `smoke_test.rs` | M0 smoke test: crate links; `codecache::VERSION == CARGO_PKG_VERSION`. | M0 |
| `parser_tests.rs` | M3 parser integration: exact byte spans, method/decorator/nested, ERROR-rate (D2). | M3 |
| `chunker_tests.rs` | M4 chunker integration: AST→Chunk, D3 enrichment, D2 heuristic fallback flag. | M4 |
| `chunker_proptest.rs` | M4 property: spans in-bounds; chunks disjoint-or-nested; child contained in parent. | M4 |
| `storage_tests.rs` | M1 storage integration: schema idempotency, chunk round-trip CRUD, BM25/MATCH ordering, empty-DB/error paths. + M8.3 D19 `symbols_for_path` (exact-file / directory-prefix / unknown-path ordering). | M1/M8 |
| `retriever_tests.rs` | M6 retriever integration: BM25 ranking determinism, dedup, token budget. | M6 |
| `formatter_tests.rs` | M7.1 formatter golden outputs: TOON/JSON/text + JSON round-trip + D13 text ordering (goldens in `fixtures/golden/`). | M7 |
| `cli_tests.rs` | M7.2/M7.3 CLI: clap parsing/defaults/exit-codes + handler behavior (init/index/update/query/status/config; serve stub) via `assert_cmd`. | M7 |
| `e2e_cli.rs` | M7.4 full E2E through the built binary: init→index→query happy path + JSON parse + failure-path nonzero/exit-code on a copied fixture repo. + M8.1 `serve --transport sse` → clean unsupported error. | M7/M8 |
| `mcp_tests.rs` | M8.1 MCP server: JSON-RPC framing + `initialize` handshake + error codes (-32700/-32601/-32602) + no-panic recovery. M8.2 `tools/list`: all three D13 tools with exact §8.2 inputSchemas + stable tool order. M8.3 `tools/call`: search/update/outline round-trips + bad-args → -32602. Over an in-memory reader/writer seam (no real stdio). | M8 |
| `fixtures/golden/` | Committed golden formatter outputs (`query_{basic,empty}.{toon,json,txt}`) compared CRLF→LF-normalized. | M7 |
| `fixtures/` | Sample source trees / files used by integration + E2E tests (added as needed). | M3+ |

### `fixtures/python/` (M3 parser)
Minimal, purpose-built Python files loaded by `parser_tests.rs`. Span assertions compare
`&source[start_byte..end_byte]` to the expected text, so the exact bytes (incl. newlines) matter
— do not reformat these.

| File | Purpose | Newlines |
|---|---|---|
| `valid_module.py` | well-formed module: imports + free fn + class/method (parse-without-error). | LF |
| `top_level_function.py` | single free function `greet`. | LF |
| `simple_class.py` | `Greeter` class with `__init__` + `greet` methods. | LF |
| `nested_function.py` | `outer` free fn containing a nested `inner`. | LF |
| `async_def.py` | `async def fetch`. | LF |
| `decorated_function.py` | `@cache` + `@retry(3)` over `def compute` (decorator-in-span). | LF |
| `multibyte_identifier.py` | `def αβγ(τ)` — multibyte UTF-8 identifiers (byte-vs-char guard). | LF |
| `crlf_function.py` | `def crlf_fn` with CRLF endings (span preserves `\r\n`). | **CRLF** |
| `malformed.py` | one good fn + a broken `def broken(:` → some ERROR nodes (positive rate). | LF |
| `high_error.py` | mostly garbage → ERROR-rate above `HEURISTIC_FALLBACK_THRESHOLD`. | LF |
| `enriched_module.py` | module docstring + `import os`/`from typing import List` + `UserService.register` calling free fn `hash_password` (D3 enrichment: docstring/imports/cross_references). | LF |

Integration tests for storage round-trips (M1), parser fixtures (M3), chunker non-overlap
property (M4), indexer idempotency (M5), retriever ranking/budget (M6), formatter goldens +
E2E `init→index→query` (M7), and MCP round-trip (M8) land in their milestones — one file or
module per concern, named after the behavior under test.

## Rules (TDD)
- Tests are written **first** (RED) before any production line they cover (`../docs/ENGINEERING_PLAN.md` §3).
- Never weaken or delete a test to make it pass.
- Property tests use `proptest` (declared in `[dev-dependencies]` from M0).
- Keep fixtures small and deterministic; stable ordering so assertions don't flake.

## Status
M0: only `smoke_test.rs` exists (the RED→GREEN gate for scaffolding). No fixtures yet.
