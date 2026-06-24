//! M6 slice M6.2 — retriever (BM25 search + determinism + dedup) integration tests (RED first).
//!
//! Scenarios: docs/TEST_STRATEGY.md#retriever and docs/plans/M6-retriever.md (Slice M6.2).
//! API anchor: docs/project_plan.md §3.2.3 (`Retriever`/`QueryOptions`/`QueryResult`) + §6.2.
//!
//! These tests seed `Storage` directly (no real indexing needed — M6 is independent of the
//! M3→M4→M5 chain) and exercise `Retriever::query` end to end over the seeded FTS5 index:
//! BM25 relevance ordering, deterministic + stable tie-break, no-match / empty-query → empty
//! well-formed result, dedup of overlapping spans in the same file, and the `file_filter`.
//!
//! Token-budget packing is NOT exercised here (that is M6.3); `max_tokens` is set generously so
//! it never trims, isolating the search/dedup/ordering behavior this slice owns.

use std::path::PathBuf;

use codecache::retriever::{QueryOptions, Retrieve, Retriever, RetrieverError};
use codecache::storage::Storage;
use codecache::types::{Chunk, Language, SymbolType};

/// Build a `Storage` over a fresh temp DB with the schema initialized.
/// Returns (dir, storage); keep `dir` alive for the test's duration.
fn fresh_storage() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("index.db");
    let storage = Storage::new(&db_path).expect("open/create db");
    storage.init_schema().expect("init schema");
    (dir, storage)
}

/// Construct a `Chunk` with explicit byte span, so dedup-by-overlap tests can control spans.
#[allow(clippy::too_many_arguments)]
fn chunk_at(
    file: &str,
    name: &str,
    body: &str,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
) -> Chunk {
    Chunk {
        symbol_name: name.to_string(),
        symbol_type: SymbolType::Function,
        file_path: PathBuf::from(file),
        start_byte,
        end_byte,
        start_line,
        end_line,
        chunk_text: body.to_string(),
        language: Language::Python,
        parent_symbol: None,
        file_docstring: None,
        imports: Vec::new(),
        cross_references: Vec::new(),
        is_heuristic: false,
    }
}

/// A simple chunk at line 1..10, bytes 0..len.
fn chunk(file: &str, name: &str, body: &str) -> Chunk {
    chunk_at(file, name, body, 0, body.len(), 1, 10)
}

/// Generous options: large token budget (no trimming in this slice), default-ish caps.
/// `bm25_weights: None` ⇒ the default per-column BM25 weights (R2.2a / D24) — this is the
/// default-identical path, NOT a behavior change.
fn opts() -> QueryOptions {
    QueryOptions {
        max_tokens: 1_000_000,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    }
}

// ───────────────────────── relevance: relevant ranks above irrelevant ─────────────────────────

#[test]
fn relevant_chunk_ranks_above_irrelevant() {
    // Seed two chunks: one whose symbol name matches the query strongly, one unrelated.
    // The query "authenticate user" must rank the auth chunk first (BM25 name weighting).
    let (_dir, storage) = fresh_storage();
    let relevant = chunk(
        "src/auth.py",
        "authenticate_user",
        "def authenticate_user(): validate the user credentials",
    );
    let irrelevant = chunk(
        "src/math.py",
        "compute_factorial",
        "def compute_factorial(n): return product of range",
    );
    storage
        .insert_chunks(&[irrelevant, relevant])
        .expect("seed chunks");

    let retriever = Retriever::new(storage);
    let result = retriever
        .query("authenticate user", opts())
        .expect("query succeeds");

    assert!(
        !result.chunks.is_empty(),
        "the relevant chunk must be retrieved"
    );
    assert_eq!(
        result.chunks[0].chunk.symbol_name, "authenticate_user",
        "the strongly-matching chunk must rank first"
    );
    // total_results_found reflects how many matched before any (future) budget trimming.
    assert!(
        result.total_results_found >= 1,
        "found count reflects matches"
    );
}

// ───────────────────────── determinism + stable tie-break ─────────────────────────

#[test]
fn same_query_same_index_yields_identical_order() {
    // Several chunks that all match the same term equally (identical body text) so their BM25
    // scores tie; the retriever must apply a deterministic, stable tie-break so the order is
    // identical across repeated queries. Tie-break key documented as (file_path, start_byte).
    let (_dir, storage) = fresh_storage();
    let body = "handle request and return response";
    let chunks = vec![
        chunk_at("src/c.py", "c_handler", body, 0, body.len(), 1, 5),
        chunk_at("src/a.py", "a_handler", body, 0, body.len(), 1, 5),
        chunk_at("src/b.py", "b_handler", body, 0, body.len(), 1, 5),
        chunk_at(
            "src/a.py",
            "a_handler2",
            body,
            100,
            100 + body.len(),
            20,
            25,
        ),
    ];
    storage.insert_chunks(&chunks).expect("seed chunks");

    let retriever = Retriever::new(storage);
    let first = retriever
        .query("request response", opts())
        .expect("query 1");
    for _ in 0..5 {
        let again = retriever
            .query("request response", opts())
            .expect("query n");
        let order_first: Vec<_> = first
            .chunks
            .iter()
            .map(|r| (r.chunk.file_path.clone(), r.chunk.start_byte))
            .collect();
        let order_again: Vec<_> = again
            .chunks
            .iter()
            .map(|r| (r.chunk.file_path.clone(), r.chunk.start_byte))
            .collect();
        assert_eq!(
            order_first, order_again,
            "repeated identical queries must yield identical order"
        );
    }

    // The stable key is (file_path, start_byte): among tied scores, a.py(0) < a.py(100) < b.py < c.py.
    let order: Vec<(String, usize)> = first
        .chunks
        .iter()
        .map(|r| {
            (
                r.chunk.file_path.to_string_lossy().into_owned(),
                r.chunk.start_byte,
            )
        })
        .collect();
    assert_eq!(
        order,
        vec![
            ("src/a.py".to_string(), 0),
            ("src/a.py".to_string(), 100),
            ("src/b.py".to_string(), 0),
            ("src/c.py".to_string(), 0),
        ],
        "tied scores break by (file_path, start_byte) ascending"
    );
}

// ───────────────────────── no-match / empty query → empty well-formed result ─────────────────────────

#[test]
fn no_match_query_returns_empty_result() {
    // A query whose terms appear nowhere in the index returns an empty, well-formed result.
    let (_dir, storage) = fresh_storage();
    storage
        .insert_chunks(&[chunk("src/a.py", "alpha", "def alpha(): pass")])
        .expect("seed");

    let retriever = Retriever::new(storage);
    let result = retriever
        .query("nonexistentterm zzzqqq", opts())
        .expect("query succeeds even with no match");
    assert!(result.chunks.is_empty(), "no match ⇒ empty chunks");
    assert_eq!(result.total_results_found, 0, "no match ⇒ zero found");
    assert_eq!(result.total_tokens, 0, "no chunks ⇒ zero tokens");
}

#[test]
fn empty_or_all_stopword_query_short_circuits_without_running_match() {
    // Empty / whitespace / all-stopword queries reduce to no tokens after preprocessing. The
    // retriever must short-circuit to an empty, well-formed result WITHOUT ever issuing
    // `MATCH ""` (which FTS5 rejects). If it tried, the call would error — so success here proves
    // the short-circuit path.
    let (_dir, storage) = fresh_storage();
    storage
        .insert_chunks(&[chunk("src/a.py", "alpha", "def alpha(): pass")])
        .expect("seed");

    let retriever = Retriever::new(storage);
    for q in ["", "   ", "find the"] {
        let result = retriever
            .query(q, opts())
            .expect("empty/all-stopword query must not error (no MATCH \"\")");
        assert!(
            result.chunks.is_empty(),
            "no tokens ⇒ empty chunks for {q:?}"
        );
        assert_eq!(
            result.total_results_found, 0,
            "no tokens ⇒ zero found for {q:?}"
        );
        assert_eq!(result.total_tokens, 0, "no tokens ⇒ zero tokens for {q:?}");
    }
}

#[test]
fn empty_db_query_returns_empty_result_without_panic() {
    // Cross-cutting: querying an empty index is a well-formed empty result, not a panic/error.
    let (_dir, storage) = fresh_storage();
    let retriever = Retriever::new(storage);
    let result = retriever
        .query("anything at all", opts())
        .expect("query empty db");
    assert!(result.chunks.is_empty());
    assert_eq!(result.total_results_found, 0);
}

// ───────────────────────── dedup overlapping snippets ─────────────────────────

#[test]
fn overlapping_snippets_deduplicated() {
    // Two chunks in the SAME file whose byte spans overlap must collapse to one in the result
    // (keep the better-ranked / first-encountered). A chunk in a different file, or a
    // non-overlapping span in the same file, is kept.
    let (_dir, storage) = fresh_storage();
    let body = "process payment and charge the card";
    // a.py: [0,50) and [40,90) overlap (40 < 50) ⇒ one survives.
    // a.py: [200,250) does NOT overlap the first cluster ⇒ kept.
    // b.py: [0,50) is a different file ⇒ kept even though byte span coincides.
    let chunks = vec![
        chunk_at("src/a.py", "process_payment", body, 0, 50, 1, 5),
        chunk_at("src/a.py", "process_payment_dup", body, 40, 90, 4, 9),
        chunk_at("src/a.py", "charge_card", body, 200, 250, 30, 35),
        chunk_at("src/b.py", "b_payment", body, 0, 50, 1, 5),
    ];
    storage.insert_chunks(&chunks).expect("seed");

    let retriever = Retriever::new(storage);
    let result = retriever.query("payment charge", opts()).expect("query");

    // 4 seeded, one overlapping pair collapses ⇒ 3 distinct results.
    assert_eq!(
        result.chunks.len(),
        3,
        "overlapping same-file spans collapse to one"
    );

    // No two surviving results share a file AND overlap in bytes.
    for i in 0..result.chunks.len() {
        for j in (i + 1)..result.chunks.len() {
            let a = &result.chunks[i].chunk;
            let b = &result.chunks[j].chunk;
            if a.file_path == b.file_path {
                let overlap = a.start_byte < b.end_byte && b.start_byte < a.end_byte;
                assert!(
                    !overlap,
                    "surviving results in the same file must not overlap: {:?} vs {:?}",
                    (a.start_byte, a.end_byte),
                    (b.start_byte, b.end_byte)
                );
            }
        }
    }
}

// ───────────────────────── file_filter ─────────────────────────

#[test]
fn file_filter_restricts_results_to_listed_files() {
    // With a file_filter, only chunks whose file_path matches survive.
    //
    // D33 MIGRATION (2026-06-22): this M6.2 test originally asserted EXACT-absolute-path equality
    // with `file_filter: Some(vec![PathBuf::from("src/keep.py")])`. Under D33 the filter is now a
    // GLOB match, not exact-path equality, so the original literal would only have matched by glob
    // coincidence. To preserve the test's intent (keep `src/keep.py`, drop `src/drop.py`) under the
    // new semantics WITHOUT weakening it, the filter value is re-expressed as the BASENAME GLOB
    // `keep.py`: a non-absolute pattern is suffix-anchored (`**/keep.py`), so it selects exactly the
    // same `src/keep.py` file and still excludes `src/drop.py`. Same assertion, glob-expressed value.
    let (_dir, storage) = fresh_storage();
    let body = "load configuration from disk";
    let chunks = vec![
        chunk("src/keep.py", "load_config", body),
        chunk("src/drop.py", "load_config_other", body),
    ];
    storage.insert_chunks(&chunks).expect("seed");

    let retriever = Retriever::new(storage);
    let options = QueryOptions {
        max_tokens: 1_000_000,
        max_results: 20,
        // Basename glob `keep.py` ⇒ suffix-anchored `**/keep.py` ⇒ selects only `src/keep.py`.
        file_filter: Some(vec![PathBuf::from("keep.py")]),
        bm25_weights: None,
    };
    let result = retriever
        .query("configuration load", options)
        .expect("query");

    assert!(!result.chunks.is_empty(), "the kept file must still match");
    for r in &result.chunks {
        assert_eq!(
            r.chunk.file_path,
            PathBuf::from("src/keep.py"),
            "only files matching the glob survive the filter"
        );
    }
}

// ───────────────────────── M6.3: token-budget packing (greedy, §6.3) ─────────────────────────
//
// The §6.3 char heuristic: estimate_tokens(text) == (text.len() / 4).max(1), counted over the
// chunk's `chunk_text` (signature + body — the text the M7 formatter emits). Packing is greedy
// over the already-ranked (stable-sorted, deduped) list: keep the prefix that fits and HARD-STOP
// (`break`) at the first chunk that would push the running total over `max_tokens`. `total_tokens`
// is the sum over the packed chunks; `total_results_found` is the pre-budget (post-filter+dedup)
// count.

/// Build a chunk whose `chunk_text` has a controlled length so token estimates are predictable.
/// All seeded chunks share the search term "widget" so they all match; distinct byte spans avoid
/// dedup. `len` is the exact byte length of `chunk_text` ⇒ estimate_tokens == (len/4).max(1).
fn sized_chunk(file: &str, name: &str, span_start: usize, len: usize) -> Chunk {
    // Body always contains the matched term "widget"; pad with 'x' to reach exactly `len` bytes.
    let base = "widget ";
    let mut body = String::from(base);
    while body.len() < len {
        body.push('x');
    }
    body.truncate(len);
    debug_assert_eq!(body.len(), len);
    chunk_at(file, name, &body, span_start, span_start + len, 1, 5)
}

#[test]
fn packing_never_exceeds_max_tokens() {
    // Seed several chunks each ~25 tokens (100 bytes). With a 60-token budget, the packed set's
    // summed estimate must stay <= 60 — the headline correctness invariant (--max-tokens never
    // exceeded).
    let (_dir, storage) = fresh_storage();
    let chunks = vec![
        sized_chunk("src/a.py", "widget_a", 0, 100),
        sized_chunk("src/b.py", "widget_b", 0, 100),
        sized_chunk("src/c.py", "widget_c", 0, 100),
        sized_chunk("src/d.py", "widget_d", 0, 100),
    ];
    storage.insert_chunks(&chunks).expect("seed");

    let retriever = Retriever::new(storage);
    let options = QueryOptions {
        max_tokens: 60,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    };
    let result = retriever.query("widget", options).expect("query");

    let summed: usize = result
        .chunks
        .iter()
        .map(|r| (r.chunk.chunk_text.len() / 4).max(1))
        .sum();
    assert!(
        summed <= 60,
        "packed estimate {summed} must not exceed max_tokens 60"
    );
    assert_eq!(
        result.total_tokens, summed,
        "total_tokens equals the summed estimate over packed chunks"
    );
}

#[test]
fn greedy_stops_at_budget_keeping_top_ranked() {
    // Each chunk is 100 bytes ⇒ 25 tokens. Budget 60 fits exactly two (25+25=50; the third would
    // make 75 > 60 ⇒ hard-stop). Greedy keeps the top-ranked prefix and stops; it does NOT skip a
    // too-big chunk to fit a smaller later one. All chunks tie on score, so the deterministic
    // tie-break (file_path, start_byte) fixes the order a.py, b.py, c.py, d.py.
    let (_dir, storage) = fresh_storage();
    let chunks = vec![
        sized_chunk("src/a.py", "widget_a", 0, 100),
        sized_chunk("src/b.py", "widget_b", 0, 100),
        sized_chunk("src/c.py", "widget_c", 0, 100),
        sized_chunk("src/d.py", "widget_d", 0, 100),
    ];
    storage.insert_chunks(&chunks).expect("seed");

    let retriever = Retriever::new(storage);
    let options = QueryOptions {
        max_tokens: 60,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    };
    let result = retriever.query("widget", options).expect("query");

    assert_eq!(
        result.chunks.len(),
        2,
        "two 25-token chunks fit in a 60-token budget; the third hard-stops packing"
    );
    let kept: Vec<_> = result
        .chunks
        .iter()
        .map(|r| r.chunk.file_path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        kept,
        vec!["src/a.py".to_string(), "src/b.py".to_string()],
        "greedy keeps the top-ranked prefix (best-first), stopping at the budget"
    );
}

#[test]
fn total_results_found_reflects_pre_budget_count() {
    // Four matching chunks; a tight budget packs only two. total_results_found must report the
    // PRE-budget count (post-filter + dedup = 4), not the packed count (2).
    let (_dir, storage) = fresh_storage();
    let chunks = vec![
        sized_chunk("src/a.py", "widget_a", 0, 100),
        sized_chunk("src/b.py", "widget_b", 0, 100),
        sized_chunk("src/c.py", "widget_c", 0, 100),
        sized_chunk("src/d.py", "widget_d", 0, 100),
    ];
    storage.insert_chunks(&chunks).expect("seed");

    let retriever = Retriever::new(storage);
    let options = QueryOptions {
        max_tokens: 60,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    };
    let result = retriever.query("widget", options).expect("query");

    assert_eq!(
        result.total_results_found, 4,
        "found count is the pre-budget (post-filter+dedup) count, not the packed count"
    );
    assert!(
        result.chunks.len() < result.total_results_found,
        "budget actually trimmed the result set for this scenario"
    );
}

#[test]
fn total_tokens_reported_matches_sum_of_packed() {
    // total_tokens must equal the sum of estimate_tokens over the chunks that survive packing —
    // not the pre-budget total. Use mixed sizes to make the sum non-trivial: 40 bytes (10 tok) +
    // 80 bytes (20 tok) = 30 tokens fits a 35-token budget; the next chunk would overflow.
    let (_dir, storage) = fresh_storage();
    let chunks = vec![
        sized_chunk("src/a.py", "widget_a", 0, 40),
        sized_chunk("src/b.py", "widget_b", 0, 80),
        sized_chunk("src/c.py", "widget_c", 0, 80),
    ];
    storage.insert_chunks(&chunks).expect("seed");

    let retriever = Retriever::new(storage);
    let options = QueryOptions {
        max_tokens: 35,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    };
    let result = retriever.query("widget", options).expect("query");

    let summed: usize = result
        .chunks
        .iter()
        .map(|r| (r.chunk.chunk_text.len() / 4).max(1))
        .sum();
    assert_eq!(
        result.total_tokens, summed,
        "total_tokens equals the sum of packed chunk estimates"
    );
    // a.py(10) + b.py(20) = 30 <= 35; c.py(20) would make 50 > 35 ⇒ excluded.
    assert_eq!(
        result.total_tokens, 30,
        "expected 10 + 20 = 30 tokens packed"
    );
    assert_eq!(result.chunks.len(), 2);
}

#[test]
fn oversized_first_chunk_yields_empty_pack() {
    // Edge: the single (top-ranked) chunk's own estimate exceeds the whole budget. Per §6.3's
    // hard-stop (`if total + chunk > max_tokens { break }`), even the FIRST chunk that doesn't fit
    // stops packing — so the packed set is EMPTY and total_tokens == 0. total_results_found still
    // reports the pre-budget count (1). Documented choice: hard-stop, NOT keep-top-1.
    let (_dir, storage) = fresh_storage();
    // 400 bytes ⇒ 100 tokens, far over a 10-token budget.
    let chunks = vec![sized_chunk("src/a.py", "widget_a", 0, 400)];
    storage.insert_chunks(&chunks).expect("seed");

    let retriever = Retriever::new(storage);
    let options = QueryOptions {
        max_tokens: 10,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    };
    let result = retriever.query("widget", options).expect("query");

    assert!(
        result.chunks.is_empty(),
        "an oversized first chunk does not fit ⇒ empty pack (hard-stop, §6.3)"
    );
    assert_eq!(result.total_tokens, 0, "empty pack ⇒ zero tokens");
    assert_eq!(
        result.total_results_found, 1,
        "found count still reflects the one pre-budget match"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// R2.2a — D24: `QueryOptions.bm25_weights` threads through to ranking (RED).
//
// The retriever must route `options.bm25_weights` into the storage call (was `storage.search`, now
// `storage.search_with_weights(.., options.bm25_weights.as_ref())`). `None` ⇒ default weights (every
// existing retriever test above keeps its order, now that the literals carry `bm25_weights: None`).
// `Some(custom)` ⇒ the custom per-column weights change the returned ranking.
//
// Seed (verified empirically against FTS5, see tests/storage_tests.rs R2.2a notes):
//   a.py / symbol_name="session"  + body "def session(): pass"            (term in the NAME column)
//   b.py / symbol_name="helper"   + body "... session and the session again" (term in BODY only)
// Default weights (symbol_name=10) ⇒ "session" first. REORDER_WEIGHTS = [0,1,5,1,1,1,1]
// (symbol_name=0, chunk_text=5) ⇒ the body-only "helper" first. The custom bm25 scores DIFFER, so
// the retriever's bm25-ascending stable sort decides the order — the (file_path, start_byte)
// tie-break never engages, making the flip robust.
//
// This fails to COMPILE until `QueryOptions` gains the `bm25_weights: Option<[f64; 7]>` field.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// The custom per-column weight vector that flips the default name-vs-body ranking (symbol_name
/// zeroed at index 0, chunk_text boosted to 5.0 at index 2). Matches `storage_tests::REORDER_WEIGHTS`.
const REORDER_WEIGHTS: [f64; 7] = [0.0, 1.0, 5.0, 1.0, 1.0, 1.0, 1.0];

#[test]
fn bm25_weights_some_changes_ranking_vs_none() {
    // The same seed + query, run once with default weights (None) and once with the custom vector,
    // must yield DIFFERENT orderings — proving QueryOptions.bm25_weights reaches storage ranking.
    let (_dir, storage) = fresh_storage();
    let name_match = chunk("a.py", "session", "def session():\n    pass");
    let body_match = chunk(
        "b.py",
        "helper",
        "def helper():\n    open the session and the session again",
    );
    storage
        .insert_chunks(&[body_match, name_match])
        .expect("seed name-vs-body corpus");

    let retriever = Retriever::new(storage);

    // None ⇒ default weights: the NAME match `session` ranks first.
    let default_opts = QueryOptions {
        max_tokens: 1_000_000,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    };
    let default = retriever
        .query("session", default_opts)
        .expect("default-weighted query");
    let default_order: Vec<String> = default
        .chunks
        .iter()
        .map(|r| r.chunk.symbol_name.clone())
        .collect();
    assert_eq!(
        default_order,
        vec!["session".to_string(), "helper".to_string()],
        "default weights: the name match `session` ranks first"
    );

    // Some(custom) ⇒ symbol_name zeroed + chunk_text boosted: the body-only `helper` ranks first.
    let custom_opts = QueryOptions {
        max_tokens: 1_000_000,
        max_results: 20,
        file_filter: None,
        bm25_weights: Some(REORDER_WEIGHTS),
    };
    let custom = retriever
        .query("session", custom_opts)
        .expect("custom-weighted query");
    let custom_order: Vec<String> = custom
        .chunks
        .iter()
        .map(|r| r.chunk.symbol_name.clone())
        .collect();
    assert_eq!(
        custom_order,
        vec!["helper".to_string(), "session".to_string()],
        "custom weights (symbol_name=0, chunk_text=5): the body-only `helper` ranks first"
    );

    assert_ne!(
        default_order, custom_order,
        "QueryOptions.bm25_weights must change the retriever's returned ordering"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// D33 — `file_filter` is a GLOB match (not exact-path equality), suffix-anchored, typed error on
// a malformed glob (RED, test-lead, 2026-06-22).
//
// Bug being specified out of existence: `apply_file_filter` kept a result only on EXACT `PathBuf`
// equality, so any user-typed glob (`*.py`) or relative fragment never equaled an absolute stored
// `chunk.file_path` ⇒ every result was silently dropped. D33 ratifies the documented behavior:
//   - Each `file_filter` pattern is compiled to a `globset` glob and matched against the stored
//     ABSOLUTE `chunk.file_path`. A result is kept if it matches ANY pattern (OR over the set).
//   - Anchoring: a pattern NOT starting with `/` is suffix-anchored (auto-prepend `**/`), so
//     `*.py` matches any `.py` file, `a/**` matches that subtree anywhere, and a basename glob
//     (`query.py`) matches that file in any dir. An ABSOLUTE glob (leading `/`) is used as-is.
//   - A MALFORMED glob (e.g. `a/[`) is a TYPED error `RetrieverError::InvalidFilter(..)`, NEVER a
//     silent empty `Ok`. A valid-but-unmatchable glob still legitimately returns zero results (Ok).
//   - `None` ⇒ no filtering (regression guard).
//
// These tests seed ABSOLUTE paths under a synthetic `/repo` root across multiple dirs/extensions,
// per the brief. They fail to COMPILE until `RetrieverError::InvalidFilter` exists, and once it
// exists they fail on behavior until the retriever compiles+matches globs instead of comparing
// paths for equality. Both are legitimate RED.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// Options carrying a glob `file_filter` (large budget so nothing is trimmed). Each entry is a raw
/// glob pattern (`Option<Vec<PathBuf>>` is unchanged — the retriever compiles them).
fn filter_opts(patterns: &[&str]) -> QueryOptions {
    QueryOptions {
        max_tokens: 1_000_000,
        max_results: 20,
        file_filter: Some(patterns.iter().map(PathBuf::from).collect()),
        bm25_weights: None,
    }
}

/// Seed the four-file corpus from the brief (ABSOLUTE paths under `/repo`, multiple dirs +
/// extensions). Every chunk shares the search term `lookup` so a query surfaces all of them; the
/// `file_filter` is then the only thing that varies which survive. Distinct byte spans avoid dedup.
/// Returns the live `TempDir` (keep alive) + a `Retriever` over the seeded storage.
fn seed_glob_corpus() -> (tempfile::TempDir, Retriever) {
    let (dir, storage) = fresh_storage();
    let body = "def lookup(): perform the lookup";
    let files = [
        "/repo/a/query.py",
        "/repo/a/models.py",
        "/repo/b/sub/query.go",
        "/repo/c/util.ts",
    ];
    let chunks: Vec<Chunk> = files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            // Distinct, non-overlapping spans per file so dedup never collapses anything.
            let start = i * 1000;
            chunk_at(f, "lookup", body, start, start + body.len(), 1, 5)
        })
        .collect();
    storage.insert_chunks(&chunks).expect("seed glob corpus");
    let retriever = Retriever::new(storage);
    (dir, retriever)
}

/// The set of (string) `file_path`s in a query result, sorted for order-independent comparison.
fn result_paths(result: &codecache::retriever::QueryResult) -> Vec<String> {
    let mut paths: Vec<String> = result
        .chunks
        .iter()
        .map(|r| r.chunk.file_path.to_string_lossy().into_owned())
        .collect();
    paths.sort();
    paths
}

#[test]
fn file_filter_star_py_keeps_all_python_regardless_of_dir() {
    // `*.py` is suffix-anchored to `**/*.py` ⇒ matches every `.py` file in any directory, and only
    // `.py` files (not the `.go` or `.ts`). This is the headline case that returned 0 under the bug.
    let (_dir, retriever) = seed_glob_corpus();
    let result = retriever
        .query("lookup", filter_opts(&["*.py"]))
        .expect("glob query succeeds");
    assert_eq!(
        result_paths(&result),
        vec![
            "/repo/a/models.py".to_string(),
            "/repo/a/query.py".to_string(),
        ],
        "`*.py` keeps every .py file (any dir) and excludes .go/.ts"
    );
}

#[test]
fn file_filter_subtree_glob_keeps_only_that_subtree() {
    // `a/**` is suffix-anchored to `**/a/**` ⇒ keeps only the `/repo/a/` subtree, no matter where it
    // sits. The `b/` and `c/` files are excluded.
    let (_dir, retriever) = seed_glob_corpus();
    let result = retriever
        .query("lookup", filter_opts(&["a/**"]))
        .expect("subtree glob query succeeds");
    assert_eq!(
        result_paths(&result),
        vec![
            "/repo/a/models.py".to_string(),
            "/repo/a/query.py".to_string(),
        ],
        "`a/**` keeps only the a/ subtree (any depth), excludes b/ and c/"
    );
}

#[test]
fn file_filter_basename_glob_keeps_that_file_in_any_dir() {
    // A basename glob `query.py` is suffix-anchored to `**/query.py` ⇒ matches the `query.py` file
    // wherever it lives, but NOT `query.go` (different extension) and not `models.py`.
    let (_dir, retriever) = seed_glob_corpus();
    let result = retriever
        .query("lookup", filter_opts(&["query.py"]))
        .expect("basename glob query succeeds");
    assert_eq!(
        result_paths(&result),
        vec!["/repo/a/query.py".to_string()],
        "`query.py` keeps exactly the one query.py (any dir), not query.go or models.py"
    );
}

#[test]
fn file_filter_absolute_glob_used_as_is_keeps_only_that_subtree() {
    // An ABSOLUTE glob (leading `/`) is root-anchored and used verbatim (NOT suffix-anchored):
    // `/repo/a/**` keeps only the `/repo/a/` subtree.
    let (_dir, retriever) = seed_glob_corpus();
    let result = retriever
        .query("lookup", filter_opts(&["/repo/a/**"]))
        .expect("absolute glob query succeeds");
    assert_eq!(
        result_paths(&result),
        vec![
            "/repo/a/models.py".to_string(),
            "/repo/a/query.py".to_string(),
        ],
        "absolute `/repo/a/**` keeps only that subtree (root-anchored, used as-is)"
    );
}

#[test]
fn file_filter_multiple_patterns_or_together() {
    // Multiple patterns OR together: `["*.py", "*.go"]` keeps py + go, excludes the ts file.
    let (_dir, retriever) = seed_glob_corpus();
    let result = retriever
        .query("lookup", filter_opts(&["*.py", "*.go"]))
        .expect("multi-pattern glob query succeeds");
    assert_eq!(
        result_paths(&result),
        vec![
            "/repo/a/models.py".to_string(),
            "/repo/a/query.py".to_string(),
            "/repo/b/sub/query.go".to_string(),
        ],
        "multiple patterns OR together: *.py OR *.go keeps py+go, excludes ts"
    );
}

#[test]
fn file_filter_valid_but_unmatchable_glob_keeps_none_ok() {
    // A valid glob that matches nothing in the corpus (`*.rs`) is a legitimate empty result — `Ok`
    // with zero chunks, NOT an error. (The bug's silent-empty was the WRONG behavior only because it
    // happened for VALID, should-have-matched patterns; a genuinely unmatchable pattern is fine.)
    let (_dir, retriever) = seed_glob_corpus();
    let result = retriever
        .query("lookup", filter_opts(&["*.rs"]))
        .expect("an unmatchable-but-valid glob is Ok, not an error");
    assert!(
        result.chunks.is_empty(),
        "a valid-but-unmatchable glob keeps no chunks (legitimate empty Ok)"
    );
    assert_eq!(result.total_results_found, 0, "no chunks ⇒ zero found");
}

#[test]
fn file_filter_none_keeps_all_regression_guard() {
    // Regression guard: `None` ⇒ no filtering at all — every matching chunk survives unchanged.
    let (_dir, retriever) = seed_glob_corpus();
    let options = QueryOptions {
        max_tokens: 1_000_000,
        max_results: 20,
        file_filter: None,
        bm25_weights: None,
    };
    let result = retriever
        .query("lookup", options)
        .expect("unfiltered query succeeds");
    assert_eq!(
        result_paths(&result),
        vec![
            "/repo/a/models.py".to_string(),
            "/repo/a/query.py".to_string(),
            "/repo/b/sub/query.go".to_string(),
            "/repo/c/util.ts".to_string(),
        ],
        "None filter keeps every file (regression guard for the existing behavior)"
    );
}

#[test]
fn file_filter_malformed_glob_returns_typed_invalid_filter_error() {
    // A MALFORMED glob (unclosed character class `a/[`) must surface as the TYPED error
    // `RetrieverError::InvalidFilter(..)` — NOT a silent empty `Ok` (the exact failure mode that hid
    // the original bug), and NOT a generic storage error. Match the variant precisely.
    let (_dir, retriever) = seed_glob_corpus();
    let err = retriever
        .query("lookup", filter_opts(&["a/["]))
        .expect_err("a malformed glob must be a typed error, not Ok");
    assert!(
        matches!(err, RetrieverError::InvalidFilter(_)),
        "a malformed glob must map to RetrieverError::InvalidFilter, got: {err:?}"
    );
    // And the Display surface must mention the offending pattern so the CLI/MCP message is useful.
    let msg = err.to_string();
    assert!(
        msg.contains("a/["),
        "InvalidFilter Display should name the offending pattern; got: {msg:?}"
    );
}

#[test]
fn file_filter_one_bad_pattern_among_valid_still_errors() {
    // The error is per-pattern-set: if ANY pattern in the OR-set is malformed, the whole query is a
    // typed InvalidFilter error (we never silently ignore the bad pattern and filter on the rest).
    let (_dir, retriever) = seed_glob_corpus();
    let err = retriever
        .query("lookup", filter_opts(&["*.py", "b/["]))
        .expect_err("any malformed pattern in the set makes the query a typed error");
    assert!(
        matches!(err, RetrieverError::InvalidFilter(_)),
        "a malformed pattern anywhere in the set ⇒ InvalidFilter, got: {err:?}"
    );
}
