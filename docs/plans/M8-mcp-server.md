# M8 — mcp_server (stdio JSON-RPC)

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m8--mcp_server),
> [`../project_plan.md`](../project_plan.md) §8, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#mcp_server).

## Goal / acceptance criteria
Expose CodeCache as an MCP server over stdio JSON-RPC with two tools, wired to `codecache serve`.
**Exit (from ROADMAP):**
- [ ] JSON-RPC handshake completes; tool registration list returned.
- [ ] `codecache_search` tool round-trips against a mock client.
- [ ] Malformed request ⇒ proper JSON-RPC error (no panic).

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/mcp_server/mod.rs` | Server loop: read/dispatch/write JSON-RPC over stdio. | eng-lead |
| `src/mcp_server/tools.rs` | `codecache_search` + `codecache_update` schemas + handlers. | eng-lead |
| `src/cli/serve.rs` | `serve --transport stdio` wiring (replaces M7 stub). | eng-lead |
| `tests/mcp_tests.rs` | Handshake, tools/list, tools/call round-trip, error cases. | test-lead |
| `src/mcp_server/CLAUDE.md` | Protocol surface + tool schemas. | manager |

## Dependencies
- **Prior:** M6 `retriever` (search), M5 `indexer` (update), M7 `cli` (serve command + config).
- **MCP = JSON-RPC 2.0 over stdio** implemented manually (§10.2: "Custom (no SDK yet)"). No new
  external SDK dep; reuse `serde_json`. v0.1 = stdio only (SSE/HTTP deferred — D4).

## Ordered slices

### Slice M8.1 — JSON-RPC framing + handshake
- **RED (test-lead):** drive the server with a mock client over piped stdin/stdout:
  - `initialize_request_returns_server_capabilities`
  - `malformed_json_returns_parse_error` (JSON-RPC code -32700)
  - `unknown_method_returns_method_not_found` (-32601)
  - `missing_required_param_returns_invalid_params` (-32602)
- **GREEN:** read newline/length-framed JSON-RPC requests from stdin, dispatch by `method`,
  write responses to stdout. Implement `initialize` handshake. Strict error mapping, no panic.

### Slice M8.2 — tool registration (tools/list)
- **RED:**
  - `tools_list_includes_codecache_search_with_input_schema` (§8.2 Tool 1 schema)
  - `tools_list_includes_codecache_update_with_input_schema` (§8.2 Tool 2 schema)
- **GREEN:** register both tools with the exact `inputSchema` from §8.2 (query/max_tokens/
  file_filter; files[]). Return them from `tools/list`.

### Slice M8.3 — tools/call round-trip
- **RED:**
  - `call_codecache_search_returns_formatted_results` (seed an index; assert content text — §8.2 response)
  - `call_codecache_update_reindexes_and_reports_stats` (§8.3 handle_update)
  - `call_with_bad_arguments_returns_invalid_params`
- **GREEN:** `handle_search` → `Retriever::query` → LLM-friendly markdown (§8.2 example
  response shape); `handle_update` → `Indexer::update_files` → stats text (§8.3). Reuse the M7
  formatter where possible.

## API contracts / data structures (from `../project_plan.md` §8)
- **Transport:** stdio JSON-RPC 2.0 (`serve --transport stdio`, §7.2). `--transport sse`/`--port`
  parse but return "unsupported in v0.1" (D4 — adapter seam kept).
- **Tool 1 `codecache_search`** input schema (§8.2): `{ query: string (req), max_tokens:
  integer=4000, file_filter: string|null }`. Response: `{ content: [{ type:"text", text }] }`.
- **Tool 2 `codecache_update`** (§8.2): `{ files: string[] (req) }` → text stats.
- Server holds `storage`, `retriever`, `indexer` (§8.3 `CodeCacheMCPServer`). Note §8.3
  pseudocode shows `Storage` being cloned into retriever+indexer — reconcile ownership
  (Arc/shared connection or re-open) — **deviation D8 below**.

## Performance budgets
- No new latency budget; a `codecache_search` call is bounded by the M6 query budget
  (p95 < 500ms). Server framing overhead must be negligible (< few ms).

## Decision Log bindings
- **D4 (integration decoupling):** keep the retrieval core transport-agnostic — `mcp_server` is
  a thin adapter calling the same `Retriever`/`Indexer` the CLI uses. This is what lets an HTTP
  REST adapter (`serve --http`) be added in v0.2 without refactoring. Document the seam.

## Definition of Done (this phase)
- [ ] M8.1–M8.3 green; handshake + tools/list + tools/call round-trip vs mock client.
- [ ] Malformed/unknown/invalid-params → correct JSON-RPC error codes, no panic.
- [ ] Tool schemas match §8.2 exactly; `serve` stub from M7 replaced.
- [ ] Storage-sharing ownership (D8) resolved; SSE/HTTP cleanly unsupported (not crashing).
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 8 + `src/mcp_server/CLAUDE.md` updated.

## Deviation to record (ROADMAP)
- **D8 — server resource ownership.** §8.3 clones `Storage` into both `Retriever` and `Indexer`.
  SQLite `Connection` isn't trivially `Clone`. Resolve via a shared `Arc<Mutex<Connection>>` or
  by giving the server one `Storage` it lends out, or re-opening read/write handles. Pick the
  simplest correct option; update `project_plan.md` §3.2.3/§8.3 to match before implementing.
