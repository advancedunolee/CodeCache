# src/mcp_server/ — CLAUDE.md

**Module:** `mcp_server` · **Owner:** `principal-engineering-lead` · **Milestone:** M8 (stub at M0).

## Purpose
MCP protocol adapter over stdio JSON-RPC: handshake, tool registration (`codecache_search`,
`codecache_update`), tool-call dispatch. Kept a **separate module** from `retriever`/`cli` so the
retrieval core stays transport-agnostic and an HTTP/LSP adapter can be added in v0.2 without
refactoring (**Decision Log D4**). Lends a shared `Storage` (`Arc<Mutex<Connection>>`, **D8**) to
`Retriever`/`Indexer`.

## API anchor
`docs/project_plan.md` §8.2 (tool schemas) + §8.3 (server pseudocode).

## Tests / scenarios
`docs/TEST_STRATEGY.md#mcp_server` — JSON-RPC handshake; tool registration list; `query`
round-trip vs mock client; malformed request → proper JSON-RPC error.
`tests/mcp_tests.rs` drives the server over an **in-memory** reader/writer pair (no real stdio,
no subprocess) via the generic `serve` seam below.

## Protocol decisions (D15: hand-rolled, serde/serde_json only — no `rmcp`, no tokio)
- **Framing:** line-delimited JSON — exactly one JSON-RPC 2.0 object per line, each `\n`-terminated.
  No `Content-Length` headers. Blank lines skipped; clean EOF → `Ok(())`.
- **protocolVersion:** `"2024-11-05"` advertised in the `initialize` result (constant in `mod.rs`).
- **Error codes:** parse/non-object → `-32700`; unknown method → `-32601`; missing/invalid params
  (incl. `initialize` without `params`/`protocolVersion`) → `-32602`. Every error is a structured
  `{ jsonrpc, id, error: { code, message } }`; the loop **never panics** and recovers per-line.

## Shipped API (M8.1 — framing + handshake)
```rust
pub struct CodeCacheServer { /* holds a shared Storage (D8); Retriever/Indexer wired in M8.3 */ }
impl CodeCacheServer { pub fn new(storage: crate::storage::Storage) -> Self; }

/// Transport-agnostic (D4) read→dispatch→write loop. Generic over reader/writer so tests inject
/// in-memory pipes; `cli::serve` passes `stdin().lock()` / `stdout().lock()`.
pub fn serve<R: std::io::BufRead, W: std::io::Write>(
    reader: R, writer: W, server: CodeCacheServer,
) -> anyhow::Result<()>;
```
`initialize` → `result { protocolVersion, capabilities, serverInfo { name: "codecache", version } }`,
echoing the request `id`. No reachable `unwrap/expect/panic`; `StorageError`/serde/io map via `?`.
`tools/call` (+ D13 outline execution, D14 self-healing) land in M8.3–M8.4.

## Shipped API (M8.2 — tools/list)
`tools/list` → `result { tools: [ {name, description, inputSchema}, ... ] }` in the **fixed,
deterministic order** `[codecache_search, codecache_update, codecache_outline]` (a `Vec`, never
HashMap iteration). Schemas live in `src/mcp_server/tools.rs`
(`pub(crate) fn tool_definitions() -> Vec<serde_json::Value>`), hand-written `serde_json::json!`
copies of **§8.2 verbatim** (D13): search `{query(req), max_tokens=4000, file_filter=null}`,
update `{files[](req)}`, outline `{path(req), max_tokens=2000}`. `default` values are JSON values
of each property's own type (numbers / null). `tools/list` accepts absent `params` (not -32602).

## Shipped API (M8.3 — tools/call round-trip)
`tools/call` → parses `params.name` + `params.arguments`, routes to the three handlers in
`src/mcp_server/handlers.rs`, and returns `result { content: [ { type:"text", text } ] }`:
- `codecache_search` → `Retriever::query` (`query` req, `max_tokens`=4000, `file_filter` opt) →
  M7 `formatter::format(.., Format::Text)` (agent-first, D13).
- `codecache_update` → force re-index of the named `files` (delete each `files_metadata` row via
  `Storage::delete_file_meta`, then `Indexer::update_files`) → stats text. This is the MCP
  "re-index these now" semantic; it does not change `update_files`' own idempotency.
- `codecache_outline` → `Storage::symbols_for_path` (D19) → `SymbolOutline` skeleton lines (D13).
Error mapping: unknown tool name / missing or mistyped required arg → `-32602`; handler-internal
failure (retrieval/index/storage error) → `-32603`. `CodeCacheServer` now holds `Retriever` +
`Indexer` over its shared `Storage` (D8); the `serve` loop dispatches `&mut self` so `update` can
mutate the index. No reachable `unwrap/expect/panic`.

## Status
M0: empty stub. **M8.1 DONE (2026-06-12):** JSON-RPC framing + `initialize` handshake + error
mapping; `serve` stub replaced (stdio wired; SSE → clean unsupported error, D4); all four gates green.
**M8.2 DONE (2026-06-12):** `tools/list` with all three D13 tool schemas (`tools.rs`); schemas match
§8.2 char-for-char. **M8.3 DONE (2026-06-12):** `tools/call` round-trip (search/update/outline) +
`handlers.rs` + D19 `symbols_for_path`; reviewer APPROVED; 162 tests green (Rust 1.85). M8.4 pending.
