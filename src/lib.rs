//! CodeCache — a local-first, AST-driven code-context retrieval engine.
//!
//! This is the M0 skeleton: module bodies are empty stubs, filled milestone by milestone per
//! [`docs/ROADMAP.md`]. The only public symbol with a value at M0 is [`VERSION`].
//!
//! Module map (build order is bottom-up — see `docs/ENGINEERING_PLAN.md` §2):
//! - [`types`]      shared, dependency-free core types (`Chunk`, `Language`, …) — Decision Log D5
//! - [`config`]     `.codecache/config.toml` load + validation
//! - [`storage`]    SQLite + FTS5 schema, CRUD, BM25 search
//! - [`hasher`]     xxHash3-128 content hashing + change detection
//! - [`parser`]     Tree-sitter integration: grammars, queries, AST nodes
//! - [`chunker`]    AST nodes → enriched `Chunk`s
//! - [`indexer`]    discovery → parse → chunk → hash → store (incremental)
//! - [`retriever`]  BM25 search + snippet extraction + token budgeting
//! - [`formatter`]  TOON / JSON / plaintext output
//! - [`cli`]        `clap` command parsing + dispatch
//! - [`mcp_server`] stdio JSON-RPC MCP adapter (transport-agnostic core — Decision Log D4)

/// The crate version, sourced from `Cargo.toml` so it has a single source of truth.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod types;

pub mod chunker;
pub mod cli;
pub mod config;
pub mod formatter;
pub mod hasher;
pub mod indexer;
pub mod mcp_server;
pub mod parser;
pub mod retriever;
pub mod storage;
