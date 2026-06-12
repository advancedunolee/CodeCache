//! JSON serializer — the programmatic format (`project_plan.md` §6.4.2).
//!
//! Uses a format-local DTO so `types::Chunk` stays free of transport concerns (Decision Log
//! D4/D5). Top-level keys, in order: `query`, `total_results` (from `total_results_found`),
//! `total_tokens`, `chunks[]`. Pretty-printed with 2-space indent. The empty result serializes
//! to a valid object with `chunks: []` and zero counts.

use serde::{Deserialize, Serialize};

use crate::retriever::QueryResult;

/// Top-level §6.4.2 envelope echoing the query alongside the packed results.
#[derive(Debug, Serialize, Deserialize)]
struct JsonResult<'a> {
    query: &'a str,
    total_results: usize,
    total_tokens: usize,
    chunks: Vec<JsonChunk<'a>>,
}

/// Per-chunk §6.4.2 record carrying exactly the documented transport fields.
#[derive(Debug, Serialize, Deserialize)]
struct JsonChunk<'a> {
    symbol_name: &'a str,
    symbol_type: &'a str,
    file_path: String,
    start_byte: usize,
    end_byte: usize,
    language: &'a str,
    bm25_score: f64,
    chunk_text: &'a str,
}

/// Serialize `result` (with its `query`) to pretty-printed §6.4.2 JSON.
pub(super) fn render(result: &QueryResult, query: &str) -> String {
    let chunks = result
        .chunks
        .iter()
        .map(|sr| {
            let c = &sr.chunk;
            JsonChunk {
                symbol_name: &c.symbol_name,
                symbol_type: c.symbol_type.as_str(),
                file_path: c.file_path.to_string_lossy().into_owned(),
                start_byte: c.start_byte,
                end_byte: c.end_byte,
                language: c.language.as_str(),
                bm25_score: sr.bm25_score,
                chunk_text: &c.chunk_text,
            }
        })
        .collect();

    let dto = JsonResult {
        query,
        total_results: result.total_results_found,
        total_tokens: result.total_tokens,
        chunks,
    };

    // serde_json with a `#[derive(Serialize)]` struct cannot fail here (no maps with non-string
    // keys, no custom Serialize), but fall back to a minimal valid object rather than panicking on
    // the unreachable error path.
    serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".to_string())
}
