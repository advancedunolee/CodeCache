//! `tools/call` handlers for the three D13 tools (`codecache_search`, `codecache_update`,
//! `codecache_outline`), wired in M8.3. Each handler parses its tool arguments, runs the
//! retrieval/index/outline core over the shared [`Storage`] (D8), and returns the text payload
//! the MCP content envelope wraps (`{ content: [ { type:"text", text } ] }`, `project_plan.md`
//! §8.2 / §8.3). A missing/wrong-typed required argument maps to `-32602` (invalid params); the
//! dispatcher in [`super`] turns the returned `(code, message)` into a JSON-RPC error object.
//!
//! No reachable `unwrap`/`expect`/`panic!`: storage/retriever/indexer failures surface as
//! `(-32603, message)` (internal error) via `?`, and argument-shape failures as `-32602`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::config::Config;
use crate::formatter::{self, Format};
use crate::hasher;
use crate::indexer::Indexer;
use crate::retriever::{QueryOptions, QueryResult, Retrieve, Retriever, RetrieverError};
use crate::storage::Storage;
use crate::types::SymbolOutline;

use super::StalenessStats;

/// JSON-RPC "invalid params" (a malformed/missing tool argument).
const INVALID_PARAMS: i64 = -32602;
/// JSON-RPC "internal error" (a retrieval/index/storage failure while executing a tool).
const INTERNAL_ERROR: i64 = -32603;

/// `codecache_search` (§8.2/§8.3, D14 self-healing): `query` (required), `max_tokens`
/// (default 4000), `file_filter` (optional). Before answering, the handler hash-checks ONLY the
/// files surfaced by a first query (heal cost ∝ result count, overview §5.2), transparently
/// re-indexes the ones whose on-disk content changed, evicts the ones deleted on disk, then
/// re-runs the query ONCE and formats THAT fresh result with the M7 agent-first text formatter
/// (§6.4.3, D13). A clean (unchanged) result set triggers NO re-index writes. The staleness-window
/// metric (checked / reindexed / dropped) is recorded into `staleness` at the end of the search.
pub(super) fn handle_search(
    storage: &Storage,
    args: &Value,
    staleness: &Arc<Mutex<StalenessStats>>,
) -> Result<String, (i64, String)> {
    let query = require_str(args, "query")?;
    let max_tokens = optional_usize(args, "max_tokens")?.unwrap_or(4000);
    let file_filter = args
        .get("file_filter")
        .and_then(Value::as_str)
        .map(|f| vec![PathBuf::from(f)]);

    let options = || QueryOptions {
        max_tokens,
        file_filter: file_filter.clone(),
        ..Default::default()
    };

    let retriever = Retriever::new(storage.clone());

    // (a) Run the query once to find the implicated files = the distinct `file_path`s of the hits.
    let first = run_query(&retriever, query, options())?;
    let implicated = distinct_files(&first);

    // (b) Heal each implicated file: re-index a changed-but-present file, evict a deleted one.
    // Only files tracked in `files_metadata` (a stored content+mtime hash, §4.4) are part of the
    // staleness window — those are the genuinely-indexed files the heal protects. A result chunk
    // with no stored hash (e.g. directly seeded, never indexed from disk) is left untouched: there
    // is no on-disk source to compare against, so it is neither checked nor re-indexed.
    let mut stats = StalenessStats::default();
    for path in &implicated {
        let cached = storage
            .get_file_hash(path)
            .map_err(|e| (INTERNAL_ERROR, format!("staleness check failed: {e}")))?;
        let Some(cached) = cached else {
            continue;
        };
        stats.files_checked += 1;
        match hasher::is_changed(path, Some(cached.as_str())) {
            // Changed and still readable on disk → transparent re-index.
            Ok(true) => {
                let mut indexer =
                    Indexer::new(Config::default(), storage.clone(), PathBuf::from("."))
                        .map_err(|e| (INTERNAL_ERROR, format!("could not build indexer: {e}")))?;
                indexer
                    .update_files(std::slice::from_ref(path))
                    .map_err(|e| (INTERNAL_ERROR, format!("re-index failed: {e}")))?;
                stats.files_reindexed += 1;
            }
            // Unchanged → leave untouched (NO write).
            Ok(false) => {}
            // Unreadable / deleted on disk → the hash error IS the deletion signal: evict the
            // file's now-stale chunks + metadata so a later search never returns them. Never an
            // error, never a panic.
            Err(_) => {
                storage
                    .delete_chunks_for_file(path)
                    .map_err(|e| (INTERNAL_ERROR, format!("eviction failed: {e}")))?;
                storage
                    .delete_file_meta(path)
                    .map_err(|e| (INTERNAL_ERROR, format!("eviction failed: {e}")))?;
                stats.files_dropped += 1;
            }
        }
    }

    // (c) Re-run the query ONCE and format THAT fresh result.
    let fresh = run_query(&retriever, query, options())?;

    // Record the staleness-window metric for this search (observational; a poisoned lock is
    // ignored rather than propagated — the metric must never fail the search).
    if let Ok(mut cell) = staleness.lock() {
        *cell = stats;
    }

    Ok(formatter::format(&fresh, query, Format::Text))
}

/// Run one retrieval query, mapping a retrieval failure to a JSON-RPC error. A malformed
/// `file_filter` glob ([`RetrieverError::InvalidFilter`]) is a bad ARGUMENT, so it maps to
/// `-32602` (invalid params); every other retrieval/storage failure stays `-32603` (internal
/// error). Both the self-healing probe query and the final query route through here, so a bad
/// `file_filter` surfaces as `-32602` from either.
fn run_query(
    retriever: &Retriever,
    query: &str,
    options: QueryOptions,
) -> Result<QueryResult, (i64, String)> {
    retriever.query(query, options).map_err(|e| match e {
        RetrieverError::InvalidFilter(_) => (INVALID_PARAMS, format!("invalid file_filter: {e}")),
        other => (INTERNAL_ERROR, format!("search failed: {other}")),
    })
}

/// The distinct result `file_path`s of a query, in first-seen order (deterministic — the query
/// result is already in the stable D-order). This is the bounded set the self-heal touches.
fn distinct_files(result: &QueryResult) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    for hit in &result.chunks {
        if !files.contains(&hit.chunk.file_path) {
            files.push(hit.chunk.file_path.clone());
        }
    }
    files
}

/// `codecache_update` (§8.2/§8.3): `files` (required string array). Re-indexes the named files via
/// [`Indexer::update_files`] over the shared storage and reports the §8.3 stats line
/// ("Updated N files, indexed M chunks in Tms"). `update_files` re-indexes the explicit paths and
/// does not walk `root`, so a default config + `.` root suffice for the indexer construction.
pub(super) fn handle_update(storage: &Storage, args: &Value) -> Result<String, (i64, String)> {
    let files_val = args.get("files").and_then(Value::as_array).ok_or_else(|| {
        (
            INVALID_PARAMS,
            "`files` (string array) is required".to_string(),
        )
    })?;

    let mut paths = Vec::with_capacity(files_val.len());
    for f in files_val {
        let s = f.as_str().ok_or_else(|| {
            (
                INVALID_PARAMS,
                "`files` entries must be strings".to_string(),
            )
        })?;
        paths.push(PathBuf::from(s));
    }

    // The MCP `update` tool is an explicit "re-index these files NOW" request from the agent, so
    // it must process every named file even when the on-disk content is byte-identical to the
    // stored copy. `Indexer::update_files` skips files whose hash is unchanged (the M5.3
    // idempotency guarantee), so we first clear each file's stored metadata row — making it look
    // new to `detect_changed_files` — then re-index. This forces the re-index without weakening
    // `update_files`' own idempotency contract, using only public storage APIs.
    for path in &paths {
        storage
            .delete_file_meta(path)
            .map_err(|e| (INTERNAL_ERROR, format!("update failed: {e}")))?;
    }

    let mut indexer = Indexer::new(Config::default(), storage.clone(), PathBuf::from("."))
        .map_err(|e| (INTERNAL_ERROR, format!("could not build indexer: {e}")))?;
    let stats = indexer
        .update_files(&paths)
        .map_err(|e| (INTERNAL_ERROR, format!("update failed: {e}")))?;

    Ok(format!(
        "Updated {} {}, indexed {} {} in {}ms",
        stats.files_processed,
        plural(stats.files_processed, "file", "files"),
        stats.chunks_indexed,
        plural(stats.chunks_indexed, "chunk", "chunks"),
        stats.duration_ms,
    ))
}

/// `codecache_outline` (§8.2/§8.3, D13/D19): `path` (required), `max_tokens` (default 2000). Looks
/// up the path's symbol skeleton via [`Storage::symbols_for_path`] (zero source reads, D7) and
/// renders one agent-first locator line per symbol (`[n] <qualified> (<type>) file:s-e`),
/// consistent with the §6.4.3 text skeleton-line shape.
pub(super) fn handle_outline(storage: &Storage, args: &Value) -> Result<String, (i64, String)> {
    let path = require_str(args, "path")?;
    let max_tokens = optional_usize(args, "max_tokens")?.unwrap_or(2000);

    let symbols = storage
        .symbols_for_path(&PathBuf::from(path))
        .map_err(|e| (INTERNAL_ERROR, format!("outline failed: {e}")))?;

    Ok(render_skeleton(path, &symbols, max_tokens))
}

/// Render the outline skeleton: a `Path: "<p>"` / `Found N symbols` header, then one locator line
/// per symbol — `[n] <qualified> (<type>) file:start-end` (the §6.4.3 skeleton-line shape, D13).
/// `max_tokens` is honored as a soft cap (the §6.3 `len/4` heuristic over the emitted text): we
/// stop adding symbol lines once the running estimate would exceed the budget, so a huge directory
/// outline stays bounded without truncating mid-line.
fn render_skeleton(path: &str, symbols: &[SymbolOutline], max_tokens: usize) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out, "Path: \"{path}\"");
    let _ = writeln!(out, "Found {} symbols", symbols.len());

    for (i, s) in symbols.iter().enumerate() {
        let qualified = match &s.parent_symbol {
            Some(parent) => format!("{parent}.{}", s.symbol_name),
            None => s.symbol_name.clone(),
        };
        let line = format!(
            "[{}] {qualified} ({}) {}:{}-{}",
            i + 1,
            s.symbol_type.as_str(),
            s.file_path.to_string_lossy(),
            s.start_line,
            s.end_line,
        );
        // Soft cap: stop before exceeding the budget rather than emitting a torn half-line.
        if !out.is_empty() && (out.len() + line.len() + 1) / 4 > max_tokens {
            break;
        }
        let _ = writeln!(out, "{line}");
    }

    out
}

/// Read a required string argument, mapping its absence/wrong type to `-32602` (invalid params).
fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, (i64, String)> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| (INVALID_PARAMS, format!("`{key}` (string) is required")))
}

/// Read an optional non-negative integer argument. A present-but-wrong-typed value is `-32602`;
/// an absent value is `Ok(None)` so the caller applies its documented default.
fn optional_usize(args: &Value, key: &str) -> Result<Option<usize>, (i64, String)> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(|n| Some(n as usize)).ok_or_else(|| {
            (
                INVALID_PARAMS,
                format!("`{key}` must be a non-negative integer"),
            )
        }),
    }
}

/// Pick the singular or plural noun for a count (1 → singular, else plural).
fn plural(n: usize, singular: &'static str, plural: &'static str) -> &'static str {
    if n == 1 {
        singular
    } else {
        plural
    }
}
