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

## Status
M0: empty stub. Implemented at M8.
