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

use codecache::retriever::{QueryOptions, Retrieve, Retriever};
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
fn opts() -> QueryOptions {
    QueryOptions {
        max_tokens: 1_000_000,
        max_results: 20,
        file_filter: None,
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
    // With a file_filter, only chunks whose file_path is in the listed set survive.
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
        file_filter: Some(vec![PathBuf::from("src/keep.py")]),
    };
    let result = retriever
        .query("configuration load", options)
        .expect("query");

    assert!(!result.chunks.is_empty(), "the kept file must still match");
    for r in &result.chunks {
        assert_eq!(
            r.chunk.file_path,
            PathBuf::from("src/keep.py"),
            "only listed files survive the filter"
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
