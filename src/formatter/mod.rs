//! Formatter: output serialization — TOON, JSON, plaintext.
//!
//! API anchor: `project_plan.md` §6.4. A pure `QueryResult -> String` function (Decision Log
//! **D4** — no I/O). `file:start-end` line ranges come from the chunk's stored 1-based inclusive
//! `start_line`/`end_line` (Decision Log **D7**), so the formatter does zero file reads. Owner:
//! `principal-engineering-lead`. Scenarios: `docs/TEST_STRATEGY.md#formatter`. Implemented at M7.

mod json;
mod text;
mod toon;

use crate::retriever::QueryResult;

/// Which serialization to emit. `Default` is [`Format::Text`] (the CLI display format).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    /// Compact, locator-only `file:start-end` list (§6.4.1).
    Toon,
    /// Programmatic §6.4.2 JSON.
    Json,
    /// Human-readable, agent-first §6.4.3 text (the default).
    #[default]
    Text,
}

/// Serialize `result` to a `String` in the requested `fmt`, echoing `query` where the format
/// carries it (§6.4.2 JSON `query`, §6.4.3 text header). Pure: no I/O, no file reads (D7).
pub fn format(result: &QueryResult, query: &str, fmt: Format) -> String {
    match fmt {
        Format::Toon => toon::render(result),
        Format::Json => json::render(result, query),
        Format::Text => text::render(result, query),
    }
}

#[cfg(test)]
mod tests {}
