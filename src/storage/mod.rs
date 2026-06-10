//! Storage: SQLite interface — schema (FTS5 `symbols`, `files_metadata`, `index_state`),
//! insert/query/delete, BM25 search.
//!
//! API anchor: `project_plan.md` §3.2.2 / §4.1. `Storage` will wrap `Arc<Mutex<Connection>>`
//! (Decision Log D8). Owner: `principal-engineering-lead` + `rust-treesitter-specialist` (FTS5).
//! Scenarios: `docs/TEST_STRATEGY.md#storage-sqlite--fts5`. M0: empty stub; implemented at M1.

#[cfg(test)]
mod tests {}
