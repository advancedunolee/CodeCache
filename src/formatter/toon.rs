//! TOON serializer — the compact, locator-only format (`project_plan.md` §6.4.1).
//!
//! Emits exactly one `<file>:<start_line>-<end_line>` line per result, in the incoming chunk
//! order (already BM25 best-first — no re-sort). No header, no signatures, no bodies, so the
//! output pipes straight to `cat`/an editor. Line ranges come from the stored 1-based inclusive
//! `start_line`/`end_line` (Decision Log **D7**), never byte offsets. The empty result is the
//! empty string.

use std::fmt::Write as _;

use crate::retriever::QueryResult;

/// Serialize `result` as the compact `file:start-end` locator list.
pub(super) fn render(result: &QueryResult) -> String {
    let mut out = String::new();
    for sr in &result.chunks {
        let c = &sr.chunk;
        let _ = writeln!(
            out,
            "{}:{}-{}",
            c.file_path.to_string_lossy(),
            c.start_line,
            c.end_line
        );
    }
    out
}
