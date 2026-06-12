//! Text serializer — the human-readable, agent-first format (`project_plan.md` §6.4.3, D13).
//!
//! Layout: a 56-char `─` rule, `Query: "<q>"`, `Found <found> results (showing top <shown>,
//! <tokens> tokens)`, a second rule, a blank line, then one block per chunk:
//! `[<n>] <qualified_name> (<symbol_type>) <file>:<start>-<end> (score: <bm25_score>)` followed
//! by the full `chunk_text`, blocks separated by a blank line, and a closing rule.
//!
//! Agent-first (D13): the metadata line — symbol, qualified parent, `file:start-end` range — and
//! the one-line signature (the first line of `chunk_text`) precede the body. Line ranges come
//! from stored `start_line`/`end_line` (D7). ASCII-only header (no emoji), an intentional
//! deviation from the §6.4.3 emoji example. The empty result is the header block + closing rule.

use std::fmt::Write as _;

use crate::retriever::QueryResult;
use crate::storage::SearchResult;

/// The 56-character `─` (U+2500) horizontal rule framing the header and closing the output.
const RULE: &str = "────────────────────────────────────────────────────────";

/// Serialize `result` (with its `query`) to the §6.4.3 human-readable text format.
pub(super) fn render(result: &QueryResult, query: &str) -> String {
    let mut out = String::new();

    // Header block: rule / query echo / counts / rule, then a blank line.
    let _ = writeln!(out, "{RULE}");
    let _ = writeln!(out, "Query: \"{query}\"");
    let _ = writeln!(
        out,
        "Found {} results (showing top {}, {} tokens)",
        result.total_results_found,
        result.chunks.len(),
        result.total_tokens
    );
    let _ = writeln!(out, "{RULE}");
    out.push('\n');

    // One block per result, each followed by a blank line.
    for (i, sr) in result.chunks.iter().enumerate() {
        write_block(&mut out, i + 1, sr);
        out.push('\n');
    }

    // Closing rule.
    let _ = writeln!(out, "{RULE}");
    out
}

/// Write a single numbered result block: metadata line, then the full chunk body.
fn write_block(out: &mut String, n: usize, sr: &SearchResult) {
    let c = &sr.chunk;
    let qualified = match &c.parent_symbol {
        Some(parent) => format!("{parent}.{}", c.symbol_name),
        None => c.symbol_name.clone(),
    };
    let _ = writeln!(
        out,
        "[{n}] {qualified} ({}) {}:{}-{} (score: {:.2})",
        c.symbol_type.as_str(),
        c.file_path.to_string_lossy(),
        c.start_line,
        c.end_line,
        sr.bm25_score
    );
    let _ = writeln!(out, "{}", c.chunk_text);
}
