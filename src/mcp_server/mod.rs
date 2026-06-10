//! MCP server: stdio JSON-RPC adapter, tool registration (`codecache_search`, `codecache_update`).
//!
//! API anchor: `project_plan.md` §8.2 / §8.3. Transport-agnostic core (Decision Log D4); lends a
//! shared `Storage` (`Arc<Mutex<Connection>>`, Decision Log D8) to `Retriever`/`Indexer`. Owner:
//! `principal-engineering-lead`. Scenarios: `docs/TEST_STRATEGY.md#mcp_server`. M0: empty stub;
//! implemented at M8.

#[cfg(test)]
mod tests {}
