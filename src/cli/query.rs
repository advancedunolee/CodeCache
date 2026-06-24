//! `codecache query <QUERY>` handler (M7.3).
//!
//! Opens `Storage`, builds a [`Retriever`], runs the query under the flag-derived
//! [`QueryOptions`], and prints the result through the M7.1 [`crate::formatter`] (format chosen by
//! `--format`, default text). `--file-filter` maps the given glob to a single-entry `file_filter`
//! list — the retriever compiles it with `globset` and keeps only results whose absolute
//! `chunk.file_path` matches (D33): a non-absolute pattern is suffix-anchored (`*.py` ⇒ `**/*.py`),
//! an absolute pattern (leading `/`) is used as-is, `*` does not cross `/` while `**` does. A
//! malformed glob surfaces as `RetrieverError::InvalidFilter`, which propagates through the
//! `.map_err(anyhow::Error::new)?` below to a clean nonzero exit. `--bm25-weights` (R2.2a / **D24**)
//! parses 7 comma-separated `f64` into the per-column BM25 weight override (absent ⇒ default weights).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::formatter::{self, Format};
use crate::retriever::{QueryOptions, Retrieve, Retriever};
use crate::storage::Storage;

use super::{paths, OutputFormat};

/// Number of per-column BM25 weights `--bm25-weights` expects — one per indexed FTS5 column
/// (`schema::CREATE_SYMBOLS` order). The fixed-size [`QueryOptions::bm25_weights`] array makes this
/// arity a compile-time invariant everywhere below the parse boundary; this is the single site that
/// turns free-form CLI text into that invariant, so the arity check lives here.
const BM25_WEIGHT_COUNT: usize = 7;

/// Parse the `--bm25-weights` flag value into the 7-element per-column BM25 weight override
/// (R2.2a / **D24**). The value is exactly `BM25_WEIGHT_COUNT` comma-separated `f64` in
/// `schema::CREATE_SYMBOLS` indexed-column order. Returns a typed `anyhow` error (never a panic) on
/// any malformed input, which surfaces as a clean nonzero exit:
/// - wrong arity (not exactly 7 comma-separated fields, including the empty string ⇒ 1 empty field),
/// - a field that does not parse as `f64` (non-numeric),
/// - a non-finite value (`NaN`/±`inf`), which cannot be a valid SQL numeric literal downstream.
///
/// Zero and negative weights are **accepted** — FTS5 `bm25()` honors them, and the R2 ranking sweep
/// uses them. Deterministic and total; no reachable `unwrap`/`expect`/`panic`.
fn parse_bm25_weights(raw: &str) -> Result<[f64; BM25_WEIGHT_COUNT]> {
    let fields: Vec<&str> = raw.split(',').collect();
    if fields.len() != BM25_WEIGHT_COUNT {
        bail!(
            "--bm25-weights expects exactly {BM25_WEIGHT_COUNT} comma-separated numbers \
             (symbol_name,symbol_type,chunk_text,parent_symbol,imports,cross_references,\
             file_docstring), got {} in {raw:?}",
            fields.len()
        );
    }
    let mut weights = [0.0_f64; BM25_WEIGHT_COUNT];
    for (slot, field) in weights.iter_mut().zip(fields) {
        let value: f64 = field.trim().parse().with_context(|| {
            format!("--bm25-weights entry {field:?} is not a valid number in {raw:?}")
        })?;
        if !value.is_finite() {
            bail!("--bm25-weights entry {field:?} must be finite (no NaN/inf) in {raw:?}");
        }
        *slot = value;
    }
    Ok(weights)
}

/// Search the index and print formatted results.
pub fn run(
    query: &str,
    max_tokens: usize,
    max_results: usize,
    format: OutputFormat,
    file_filter: Option<&str>,
    bm25_weights: Option<&str>,
    db_path: &Path,
) -> Result<()> {
    // Validate/parse the optional weight override BEFORE touching the database, so a malformed flag
    // fails fast with a clean typed error (nonzero exit) rather than after opening storage.
    let bm25_weights = bm25_weights.map(parse_bm25_weights).transpose()?;

    let root =
        std::env::current_dir().context("could not resolve the current working directory")?;
    let resolved_db = paths::resolve(&root, db_path);
    let storage = Storage::new(&resolved_db)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("could not open index database at {}", resolved_db.display()))?;

    let retriever = Retriever::new(storage);
    let options = QueryOptions {
        max_tokens,
        max_results,
        // Glob post-filter (D33): wrap the raw pattern as a single-entry list; the retriever
        // compiles it with `globset` (suffix-anchored unless absolute) and keeps only results
        // whose `chunk.file_path` matches. A malformed glob ⇒ `RetrieverError::InvalidFilter`.
        file_filter: file_filter.map(|f| vec![PathBuf::from(f)]),
        // R2.2a / D24: parsed per-column BM25 weights (None ⇒ default-weighted retrieval).
        bm25_weights,
    };

    let result = retriever
        .query(query, options)
        .map_err(anyhow::Error::new)?;

    let fmt: Format = format.into();
    // For an empty result set in the human-readable TEXT format, emit a query-free notice rather
    // than the formatter's `Query: "<q>"` header echo: a "no results" report must not look like it
    // surfaced the searched-for symbol (a caller checks that an unindexed symbol is genuinely
    // absent from query output). JSON stays a pipe-through so its output is always parseable, and
    // TOON's empty output is already an empty (query-free) string.
    if result.chunks.is_empty() && fmt == Format::Text {
        println!("No results found.");
        return Ok(());
    }
    print!("{}", formatter::format(&result, query, fmt));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bm25_weights_accepts_seven_values() {
        // The documented default vector round-trips into the 7-element array, in order.
        assert_eq!(
            parse_bm25_weights("10,1,1,5,2,2,2").expect("valid default vector"),
            [10.0, 1.0, 1.0, 5.0, 2.0, 2.0, 2.0]
        );
        // Fractional values parse too (the sweep explores non-integers).
        assert_eq!(
            parse_bm25_weights("0.5,1.5,2.25,3,4,5,6").expect("valid fractional vector"),
            [0.5, 1.5, 2.25, 3.0, 4.0, 5.0, 6.0]
        );
        // Surrounding whitespace per field is tolerated.
        assert_eq!(
            parse_bm25_weights(" 10 , 1 , 1 , 5 , 2 , 2 , 2 ").expect("whitespace tolerated"),
            [10.0, 1.0, 1.0, 5.0, 2.0, 2.0, 2.0]
        );
    }

    #[test]
    fn parse_bm25_weights_allows_zero_and_negative() {
        // Decision (documented): zero and negative weights are accepted — FTS5 bm25() honors them
        // and the R2 ranking sweep uses them. This is the storage edge vector.
        assert_eq!(
            parse_bm25_weights("0,1,1,5,2,2,-1").expect("zero/negative allowed"),
            [0.0, 1.0, 1.0, 5.0, 2.0, 2.0, -1.0]
        );
    }

    #[test]
    fn parse_bm25_weights_rejects_wrong_arity() {
        // Too few and too many are both clean errors (Err, not a panic). Empty string ⇒ one empty
        // field ⇒ arity 1 ≠ 7 (and would also fail the numeric parse) ⇒ rejected.
        assert!(parse_bm25_weights("1,2,3").is_err(), "3 values rejected");
        assert!(
            parse_bm25_weights("1,2,3,4,5,6,7,8").is_err(),
            "8 values rejected"
        );
        assert!(parse_bm25_weights("").is_err(), "empty string rejected");
    }

    #[test]
    fn parse_bm25_weights_rejects_non_numeric_and_non_finite() {
        // A non-numeric field is a clean error.
        assert!(
            parse_bm25_weights("a,b,c,d,e,f,g").is_err(),
            "non-numeric rejected"
        );
        // Non-finite values cannot be SQL numeric literals downstream ⇒ rejected here.
        assert!(
            parse_bm25_weights("inf,1,1,5,2,2,2").is_err(),
            "inf rejected"
        );
        assert!(
            parse_bm25_weights("NaN,1,1,5,2,2,2").is_err(),
            "NaN rejected"
        );
    }
}
