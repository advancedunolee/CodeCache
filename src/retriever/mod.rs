//! Retriever: FTS5 BM25 search, snippet extraction, token counting, greedy token-budget packing.
//!
//! API anchor: `project_plan.md` §3.2.3 / §6. Kept behind a trait so a `HybridRetriever` can
//! wrap it in v0.2 (Decision Log D1). Owner: `principal-engineering-lead`. Scenarios:
//! `docs/TEST_STRATEGY.md#retriever`.
//!
//! **M6.1 (query preprocessing)** shipped the module-private, dependency-free string functions
//! [`preprocess_query`] (tokenize → lowercase → drop stopwords → FTS5-escape) and
//! [`build_match_expression`] (` OR `-join into a valid FTS5 `MATCH` string, §6.1).
//!
//! **M6.2 (BM25 search + determinism + dedup)** lands the [`Retriever`] struct + the minimal
//! [`Retrieve`] trait (Decision Log D1, so a future `HybridRetriever` can wrap it without churn).
//! [`Retriever::query`] runs the M6.1 preprocessing, binds the resulting expression to
//! `symbols MATCH ?` **parameterized** via [`crate::storage::Storage::search`] (never
//! string-interpolated), then applies a deterministic stable tie-break, dedups overlapping
//! same-file spans, and applies the optional `file_filter`.
//!
//! **M6.3 (token-budget packing)** lands [`estimate_tokens`] (the §6.3 char heuristic
//! `(len/4).max(1)` over a chunk's `chunk_text`) and [`Retriever::apply_token_budget`] — a greedy,
//! hard-stop packer that keeps the highest-ranked prefix fitting within `max_tokens`. [`query`]
//! now packs the deduped results and reports `total_tokens` = sum over the packed chunks, while
//! `total_results_found` stays the **pre-budget** (post-filter + dedup) count.

use std::path::PathBuf;

use globset::{GlobBuilder, GlobSetBuilder};

use crate::storage::{SearchResult, Storage, StorageError};

/// Stopwords dropped during preprocessing (§6.1). Deliberately **small and code-search-oriented**:
/// only the few natural-language filler words an agent prefixes a query with (e.g. "find the
/// user", "show me how X works") carry no FTS5 signal and only dilute BM25. We do **not** strip
/// programming keywords (`if`, `for`, `class`, `type`, …) — those are often exactly what a code
/// query targets. Lowercase; matched after the token itself is lowercased.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "the", "of", "to", "in", "is", "it", "for", "on", "with", "find", "show",
    "me", "how", "where", "what", "that", "this", "get",
];

/// Tunable knobs for a single query (§3.2.3). `max_tokens` is the token budget honored by the
/// M6.3 greedy packer; `max_results` bounds the FTS5 row count; `file_filter`, when `Some`,
/// restricts results to the listed files (a **post-filter** over `chunk.file_path` — see
/// [`Retriever::query`]).
#[derive(Debug, Clone)]
pub struct QueryOptions {
    /// Token budget for the packed result set (default 4000). Enforced by greedy packing (§6.3).
    pub max_tokens: usize,
    /// Maximum number of FTS5 hits to fetch (default 20). Bounds in-flight chunks (§11.3).
    pub max_results: usize,
    /// When `Some`, keep only results whose `file_path` is in this list (post-filter).
    pub file_filter: Option<Vec<PathBuf>>,
    /// Optional per-column BM25 weight override (R2.2a / **D24**). One `f64` per indexed FTS5 column
    /// in `schema::CREATE_SYMBOLS` order (`symbol_name`, `symbol_type`, `chunk_text`,
    /// `parent_symbol`, `imports`, `cross_references`, `file_docstring`). `None` ⇒ the built-in
    /// default weights (10,1,1,5,2,2,2), byte-identical to pre-R2.2a behavior; `Some(w)` re-ranks
    /// via [`crate::storage::Storage::search_with_weights`]. Threaded from the CLI `--bm25-weights`
    /// flag so the R2 harness can sweep ranking weights per `codecache query` without recompiling.
    pub bm25_weights: Option<[f64; 7]>,
}

impl Default for QueryOptions {
    /// §3.2.3 defaults: 4000-token budget, 20 results, no file filter, default BM25 weights.
    fn default() -> Self {
        QueryOptions {
            max_tokens: 4000,
            max_results: 20,
            file_filter: None,
            bm25_weights: None,
        }
    }
}

/// The structured outcome of a query (§3.2.3). Transport-agnostic (Decision Log D4) — formatting
/// and CLI/MCP transport live downstream, so the core stays adapter-agnostic.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    /// The retrieved chunks, best-first, after tie-break + dedup + greedy token-budget packing.
    pub chunks: Vec<SearchResult>,
    /// Sum of estimated tokens (§6.3) across the packed `chunks`. Always `<= QueryOptions.max_tokens`
    /// (the pack is a fitting prefix); `0` for the empty / no-result paths.
    pub total_tokens: usize,
    /// How many results matched **before** token-budget trimming (post-filter + dedup count).
    pub total_results_found: usize,
}

/// A typed retriever error. Wraps the underlying [`StorageError`] so the caller sees one error
/// type for the whole query path. Never panics.
#[derive(Debug)]
pub enum RetrieverError {
    /// A failure in the underlying storage / FTS5 layer (lock, SQLite, corrupt row).
    Storage(StorageError),
    /// A `file_filter` glob pattern that `globset` could not compile (D33). Carries the offending
    /// pattern string so the CLI/MCP message can name it. The CLI maps this to a clean nonzero
    /// exit and the MCP handler to `-32602` (invalid params — a malformed argument).
    InvalidFilter(String),
}

impl std::fmt::Display for RetrieverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetrieverError::Storage(e) => write!(f, "retriever storage error: {e}"),
            RetrieverError::InvalidFilter(pattern) => {
                write!(f, "invalid file_filter glob pattern: {pattern}")
            }
        }
    }
}

impl std::error::Error for RetrieverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RetrieverError::Storage(e) => Some(e),
            // No inner source: the offending pattern is carried in the variant, not a wrapped error.
            RetrieverError::InvalidFilter(_) => None,
        }
    }
}

impl From<StorageError> for RetrieverError {
    fn from(e: StorageError) -> Self {
        RetrieverError::Storage(e)
    }
}

/// Convenience alias for retriever results.
pub type Result<T> = std::result::Result<T, RetrieverError>;

/// The retrieval seam (Decision Log **D1**). Minimal on purpose: just `query`, so a future
/// `HybridRetriever` (embeddings) can implement the same trait and be swapped in without churning
/// the CLI / MCP callers. Kept intentionally small — only what the v0.1 callers need.
pub trait Retrieve {
    /// Run one query and return a structured, transport-agnostic [`QueryResult`].
    fn query(&self, user_query: &str, options: QueryOptions) -> Result<QueryResult>;
}

/// AST/FTS5-backed retriever (§3.2.3). Holds a (cheaply `Clone`-able, D8) [`Storage`] handle and
/// implements [`Retrieve`]. A future `HybridRetriever` wraps this behind the same trait (D1).
pub struct Retriever {
    storage: Storage,
}

impl Retriever {
    /// Build a retriever over an existing storage handle. The MCP server (M8) can hand the same
    /// underlying connection to both `Retriever` and `Indexer` (D8).
    pub fn new(storage: Storage) -> Self {
        Retriever { storage }
    }

    /// Re-sort search hits into the documented **deterministic** order: BM25 ascending
    /// (best-first; FTS5 `bm25()` is lower-is-better), and among ties the stable key
    /// `(file_path, start_byte, end_byte)` ascending. Storage already orders by `bm25 ASC, rowid
    /// ASC`, but `rowid` is an insertion-order artifact, not a stable property of the data;
    /// re-sorting on the span key makes the order reproducible regardless of insertion order.
    /// `end_byte` is the final tie-break so two same-file chunks sharing a `start_byte` (a class
    /// and a method that begins on the class line) still order deterministically.
    fn stable_sort(results: &mut [SearchResult]) {
        results.sort_by(|a, b| {
            // f64 BM25 scores are finite in practice; `total_cmp` gives a total order with no
            // panic even on the pathological NaN/inf, keeping the sort total and deterministic.
            a.bm25_score
                .total_cmp(&b.bm25_score)
                .then_with(|| a.chunk.file_path.cmp(&b.chunk.file_path))
                .then_with(|| a.chunk.start_byte.cmp(&b.chunk.start_byte))
                .then_with(|| a.chunk.end_byte.cmp(&b.chunk.end_byte))
        });
    }

    /// Drop later results that **partially overlap** an already-kept result in the **same file**.
    /// Input must already be in best-first order so the higher-ranked chunk in each overlapping
    /// cluster is the one kept.
    ///
    /// Containment is **preserved**, not collapsed: the M4 chunker's invariant is that same-file
    /// chunks are either disjoint (siblings) or strictly nested (a method inside its class), so a
    /// class and a method within it are distinct, legitimately-retrievable units — collapsing one
    /// would destroy real signal. We therefore drop a later chunk only when it **partially**
    /// overlaps a kept chunk (neither contains the other) or duplicates it exactly. Different
    /// files never collide; disjoint and strictly-nested same-file spans are kept.
    fn dedup_overlapping(results: Vec<SearchResult>) -> Vec<SearchResult> {
        let mut kept: Vec<SearchResult> = Vec::with_capacity(results.len());
        for r in results {
            let redundant = kept.iter().any(|k| {
                k.chunk.file_path == r.chunk.file_path
                    && partial_overlap_or_equal(
                        k.chunk.start_byte,
                        k.chunk.end_byte,
                        r.chunk.start_byte,
                        r.chunk.end_byte,
                    )
            });
            if !redundant {
                kept.push(r);
            }
        }
        kept
    }

    /// Apply the optional `file_filter` **glob** post-filter (D33): when `Some`, keep only results
    /// whose absolute `chunk.file_path` matches ANY of the patterns. Documented behavior: this is a
    /// **post-filter** over the returned chunks (not a SQL `file_path` predicate), so the FTS5 query
    /// stays simple and one code path serves both the M7 CLI `--file-filter` and the MCP
    /// `file_filter` argument (D4).
    ///
    /// **Anchoring (per pattern):** a pattern starting with `/` is used verbatim (root-anchored);
    /// any other pattern is suffix-anchored by prepending `**/` so a basename/extension glob
    /// (`*.py`, `query.py`, `a/**`) matches at any depth. Each glob is built with
    /// `literal_separator(true)` so `*` does NOT cross `/` while `**` does. A malformed pattern that
    /// `globset` cannot compile yields [`RetrieverError::InvalidFilter`] carrying that pattern — the
    /// whole query fails rather than silently filtering on the survivors.
    ///
    /// The `GlobSet` is built **once per query** (not per result), so matching is a single
    /// pre-compiled pass over the hits.
    fn apply_file_filter(
        results: Vec<SearchResult>,
        filter: &Option<Vec<PathBuf>>,
    ) -> Result<Vec<SearchResult>> {
        let Some(patterns) = filter else {
            return Ok(results); // None ⇒ no filtering.
        };

        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            let raw = pattern.to_string_lossy();
            // D33 anchoring: absolute (leading `/`) ⇒ verbatim; otherwise suffix-anchor with `**/`.
            let anchored = if raw.starts_with('/') {
                raw.into_owned()
            } else {
                format!("**/{raw}")
            };
            let glob = GlobBuilder::new(&anchored)
                .literal_separator(true)
                .build()
                .map_err(|_| {
                    RetrieverError::InvalidFilter(pattern.to_string_lossy().into_owned())
                })?;
            builder.add(glob);
        }
        let set = builder
            .build()
            .map_err(|e| RetrieverError::InvalidFilter(e.to_string()))?;

        Ok(results
            .into_iter()
            .filter(|r| set.is_match(&r.chunk.file_path))
            .collect())
    }

    /// Greedily pack the already-ranked, deduped results within `max_tokens` (§6.3). Input must be
    /// in best-first order: we walk it front-to-back, keep each chunk whose [`estimate_tokens`]
    /// still fits the running budget, and **hard-stop** (`break`) at the first chunk that would
    /// push the total over `max_tokens`. This is the §3.2.3 documented surface
    /// (`fn apply_token_budget(&self, …)`).
    ///
    /// **Greedy stop, not skip-and-continue:** once a chunk doesn't fit we stop entirely rather
    /// than skipping it to squeeze in a smaller later chunk — this keeps the highest-ranked
    /// contiguous prefix, matching §6.3's `break`. A consequence is that an **oversized first
    /// chunk** (its own estimate already exceeds `max_tokens`) yields an **empty** pack rather than
    /// a forced top-1; the budget is a hard ceiling the caller asked for, so we never exceed it.
    ///
    /// Token length is measured over each chunk's `chunk_text` (the full signature+body source —
    /// the same text the M7 formatter emits), so the budget reflects the bytes actually delivered.
    fn apply_token_budget(
        &self,
        results: Vec<SearchResult>,
        max_tokens: usize,
    ) -> Vec<SearchResult> {
        let mut packed: Vec<SearchResult> = Vec::with_capacity(results.len());
        let mut total_tokens: usize = 0;
        for result in results {
            let chunk_tokens = estimate_tokens(&result.chunk.chunk_text);
            if total_tokens + chunk_tokens > max_tokens {
                break; // Budget exhausted — hard-stop, keep the fitting prefix (§6.3).
            }
            total_tokens += chunk_tokens;
            packed.push(result);
        }
        packed
    }
}

impl Retrieve for Retriever {
    /// Execute a query: preprocess → (short-circuit if no tokens) → parameterized FTS5 `MATCH` →
    /// stable tie-break → file_filter → dedup overlapping spans → **greedy token-budget packing** →
    /// assemble [`QueryResult`].
    ///
    /// An empty / all-stopword query yields no tokens; the method short-circuits to an empty,
    /// well-formed result **without ever running `MATCH ""`** (which FTS5 rejects). Otherwise the
    /// deduped results are packed within `options.max_tokens` (§6.3); `total_tokens` is the sum over
    /// the packed chunks and `total_results_found` is the **pre-budget** (post-filter+dedup) count.
    fn query(&self, user_query: &str, options: QueryOptions) -> Result<QueryResult> {
        let tokens = preprocess_query(user_query);
        if tokens.is_empty() {
            // No tokens ⇒ no MATCH. Short-circuit; never issue `MATCH ""`.
            return Ok(QueryResult {
                chunks: Vec::new(),
                total_tokens: 0,
                total_results_found: 0,
            });
        }

        let match_expr = build_match_expression(&tokens);
        // The expression is bound to `symbols MATCH ?1` inside `Storage::search_with_weights` —
        // parameterized, never string-interpolated into SQL. `options.bm25_weights` (R2.2a / D24)
        // selects the per-column BM25 weights: `None` ⇒ the built-in defaults (the default-identical
        // path); `Some(w)` ⇒ the caller's `--bm25-weights` override re-ranks the FTS5 results.
        let mut hits = self.storage.search_with_weights(
            &match_expr,
            options.max_results,
            options.bm25_weights.as_ref(),
        )?;

        Self::stable_sort(&mut hits);
        let filtered = Self::apply_file_filter(hits, &options.file_filter)?;
        let deduped = Self::dedup_overlapping(filtered);

        // `total_results_found` is the PRE-budget count (post-filter + post-dedup) — how many
        // results matched before token-budget trimming (§3.2.3).
        let total_results_found = deduped.len();

        // M6.3: greedily pack within the token budget, then report the sum of the packed chunks'
        // estimated tokens. The pack is a prefix of `deduped`, so `total_tokens <= max_tokens`.
        let packed = self.apply_token_budget(deduped, options.max_tokens);
        let total_tokens: usize = packed
            .iter()
            .map(|r| estimate_tokens(&r.chunk.chunk_text))
            .sum();

        Ok(QueryResult {
            chunks: packed,
            total_tokens,
            total_results_found,
        })
    }
}

/// Whether two half-open byte spans `[a_start, a_end)` and `[b_start, b_end)` are **redundant**
/// for dedup: they cross (partial overlap) or are identical, but **not** a containment relation.
///
/// - Disjoint (incl. touching at an endpoint) ⇒ `false` — both kept.
/// - One strictly contains the other (nested method/class) ⇒ `false` — both kept (real signal).
/// - Equal spans, or partial crossing overlap ⇒ `true` — the later one is dropped.
fn partial_overlap_or_equal(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    let overlaps = a_start < b_end && b_start < a_end;
    if !overlaps {
        return false;
    }
    let a_contains_b = a_start <= b_start && b_end <= a_end;
    let b_contains_a = b_start <= a_start && a_end <= b_end;
    // Equal spans satisfy both containment checks; treat them as redundant (drop the later).
    if a_start == b_start && a_end == b_end {
        return true;
    }
    // Strict containment (one inside the other) is kept; only crossing partial overlap is redundant.
    !(a_contains_b || b_contains_a)
}

/// Preprocess a raw user query into a normalized, FTS5-safe token list (§3.2.3 / §6.1).
///
/// Pipeline: split into tokens (maximal runs of alphanumeric / `_` / `"`; every other char —
/// whitespace, `()`, `:`, `-`, … — separates) → lowercase (Unicode-aware, never slices a char
/// boundary) → drop [`STOPWORDS`] → FTS5-escape each survivor. A safe ASCII bareword
/// (alphanumeric/`_` only) is left unquoted; any other token (non-ASCII like `café`, or one
/// carrying a `"`) is wrapped as an FTS5 string literal with internal `"` doubled, so the joined
/// expression is always syntactically valid. An empty or all-stopword query yields `[]` — the
/// caller maps that to an empty result downstream (never `MATCH ""`). Deterministic; total.
fn preprocess_query(query: &str) -> Vec<String> {
    query
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '"'))
        .filter(|tok| !tok.is_empty())
        .map(str::to_lowercase)
        .filter(|tok| !STOPWORDS.contains(&tok.as_str()))
        .map(|tok| escape_fts5_token(&tok))
        .collect()
}

/// Join already-escaped tokens into an FTS5 `MATCH` expression with ` OR ` (§6.1).
///
/// An empty token slice yields `""` — the caller treats that as "no query" and returns an empty,
/// well-formed result rather than running `MATCH ""` (which FTS5 rejects).
fn build_match_expression(tokens: &[String]) -> String {
    tokens.join(" OR ")
}

/// Escape one (already lowercased, non-empty) token for safe inclusion in an FTS5 `MATCH`
/// expression. A token that is a plain ASCII bareword (alphanumeric / `_`) is returned as-is;
/// anything else is wrapped in double quotes with internal `"` doubled, producing a valid FTS5
/// string literal that can never introduce a syntax error.
fn escape_fts5_token(token: &str) -> String {
    let is_safe_bareword = token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if is_safe_bareword {
        token.to_string()
    } else {
        format!("\"{}\"", token.replace('"', "\"\""))
    }
}

/// Fast, dependency-free token estimate (§6.3): `(text.len() / 4).max(1)`, the GPT-style
/// "1 token ≈ 4 chars" heuristic with **no tokenizer crate** in v0.1. `text.len()` is the **byte**
/// length, so a multibyte identifier counts its UTF-8 bytes (a deliberate over-estimate vs.
/// characters, which keeps the budget conservative). The `.max(1)` floor means even an empty or
/// 1–3 byte chunk costs at least one token, so a tiny chunk is never "free" in the packing loop.
/// Callers pass the chunk's `chunk_text` (full signature+body), matching what the M7 formatter
/// emits so the budget reflects the bytes actually delivered to the agent.
fn estimate_tokens(text: &str) -> usize {
    (text.len() / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── M6.1: query preprocessing (tokenize, lowercase, stopwords, FTS5 escaping) ──────────────

    #[test]
    fn preprocess_tokenizes_and_lowercases() {
        // §6.1: "Authenticate User" → ["authenticate", "user"]
        assert_eq!(
            preprocess_query("Authenticate User"),
            vec!["authenticate".to_string(), "user".to_string()]
        );
    }

    #[test]
    fn preprocess_builds_or_match_expression() {
        // §6.1: tokens join with " OR " into the FTS5 MATCH string.
        let tokens = preprocess_query("authenticate user");
        assert_eq!(build_match_expression(&tokens), "authenticate OR user");
    }

    #[test]
    fn preprocess_removes_stopwords() {
        // "find the user" → stopwords `find`/`the` dropped → ["user"].
        assert_eq!(preprocess_query("find the user"), vec!["user".to_string()]);
    }

    #[test]
    fn empty_query_after_stopword_removal_handled() {
        // Empty input and all-stopword input both degrade to an empty token list — no panic,
        // and the MATCH expression is empty (downstream M6.2 maps this to an empty QueryResult,
        // never `MATCH ""`).
        assert_eq!(preprocess_query(""), Vec::<String>::new());
        assert_eq!(preprocess_query("   "), Vec::<String>::new());
        assert_eq!(preprocess_query("find the"), Vec::<String>::new());

        let empty: Vec<String> = Vec::new();
        assert_eq!(build_match_expression(&empty), "");
    }

    #[test]
    fn preprocess_escapes_fts5_special_chars() {
        // FTS5 safety: special chars (parens, colon, quote) must not produce a MATCH syntax
        // error. A safe ASCII bareword stays unquoted; non-bareword tokens are wrapped as an FTS5
        // string literal with internal double-quotes doubled, so the joined expression is valid.
        // `foo()` → the `()` are separators; only the safe bareword `foo` survives.
        let tokens = preprocess_query("foo()");
        assert_eq!(tokens, vec!["foo".to_string()]);
        assert_eq!(build_match_expression(&tokens), "foo");

        // `user:name` → two safe barewords (colon is a separator) → unquoted, OR-joined.
        let tokens = preprocess_query("user:name");
        assert_eq!(build_match_expression(&tokens), "user OR name");

        // An embedded double-quote is the one in-token special char: it is kept and escaped by
        // doubling it inside the literal, so the expression stays balanced/valid (no dangling
        // quote → no FTS5 syntax error).
        let tokens = preprocess_query("sa\"y");
        assert_eq!(tokens, vec!["\"sa\"\"y\"".to_string()]);
    }

    #[test]
    fn preprocess_is_deterministic() {
        // Same input ⇒ identical token order/output across repeated calls.
        let input = "Parse the Config and Validate User Input";
        let first = preprocess_query(input);
        for _ in 0..5 {
            assert_eq!(preprocess_query(input), first);
        }
    }

    #[test]
    fn preprocess_handles_utf8_multibyte() {
        // Multibyte identifiers survive lowercasing without slicing a char boundary (no panic).
        assert_eq!(preprocess_query("Café"), vec!["\"café\"".to_string()]);
        assert_eq!(preprocess_query("Naïve"), vec!["\"naïve\"".to_string()]);
        // An identifier already lowercase + ASCII stays a bareword (no needless quoting).
        assert_eq!(preprocess_query("naive"), vec!["naive".to_string()]);
    }

    // ── M6.2: span overlap helper (unit) ───────────────────────────────────────────────────────

    #[test]
    fn partial_overlap_or_equal_keeps_containment_drops_crossing() {
        // Partial (crossing) overlap ⇒ redundant (drop the later one).
        assert!(partial_overlap_or_equal(0, 50, 40, 90));
        assert!(partial_overlap_or_equal(40, 90, 0, 50));
        // Equal spans ⇒ redundant.
        assert!(partial_overlap_or_equal(0, 50, 0, 50));
        // Disjoint / touching at an endpoint ⇒ not redundant (half-open).
        assert!(!partial_overlap_or_equal(0, 50, 50, 90));
        assert!(!partial_overlap_or_equal(0, 50, 200, 250));
        // Strict containment (nested method/class) ⇒ NOT redundant; both kept.
        assert!(!partial_overlap_or_equal(0, 100, 40, 60));
        assert!(!partial_overlap_or_equal(40, 60, 0, 100));
    }

    // ── M6.3: token estimation (§6.3 char heuristic) ────────────────────────────────────────────

    #[test]
    fn estimate_tokens_is_len_div_4_min_1() {
        // §6.3: estimate_tokens(text) == (text.len() / 4).max(1). `len` is the BYTE length of the
        // text. The `.max(1)` floor means any non-empty short text — and even the empty string —
        // estimates at least 1 token, so a tiny chunk is never free.
        assert_eq!(estimate_tokens(""), 1, "empty text floors to 1 token");
        assert_eq!(estimate_tokens("abc"), 1, "len 3 / 4 = 0 → floored to 1");
        assert_eq!(estimate_tokens("abcd"), 1, "len 4 / 4 = 1");
        assert_eq!(estimate_tokens("abcdefgh"), 2, "len 8 / 4 = 2");
        // 100-byte string ⇒ 25 tokens (matches the integration-test fixtures' arithmetic).
        let hundred = "x".repeat(100);
        assert_eq!(estimate_tokens(&hundred), 25, "len 100 / 4 = 25");
        // Byte length, not char count: a 2-byte UTF-8 char counts as 2 bytes.
        // "é" is 2 bytes; four of them = 8 bytes ⇒ 2 tokens.
        let multibyte = "é".repeat(4);
        assert_eq!(multibyte.len(), 8, "four 2-byte chars = 8 bytes");
        assert_eq!(
            estimate_tokens(&multibyte),
            2,
            "estimate uses byte length (8 / 4 = 2)"
        );
    }
}
