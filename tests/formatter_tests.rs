//! M7 slice M7.1 — formatter golden-output integration tests (RED first).
//!
//! Scenarios: docs/TEST_STRATEGY.md#formatter and docs/plans/M7-formatter-cli.md (Slice M7.1).
//! API anchor: docs/project_plan.md §6.4 (TOON/JSON/text) + §8.2 (D13 agent-first ordering).
//!
//! These tests build a small, deterministic in-memory `QueryResult` (no real indexing / no DB —
//! the formatter is a pure `QueryResult -> String` function, Decision Log D4) and assert each of
//! the three serializers against committed golden files under `tests/fixtures/golden/`.
//!
//! The formatter API these tests pin (engineering lead implements to this exact shape):
//!   pub enum codecache::formatter::Format { Toon, Json, Text }
//!   pub fn codecache::formatter::format(result: &QueryResult, query: &str, fmt: Format) -> String
//!
//! D7: the `file:start-end` ranges come from the chunk's stored `start_line`/`end_line`
//! (1-based inclusive), NOT byte offsets, and the formatter does ZERO file reads.

use std::path::PathBuf;

use codecache::formatter::{format, Format};
use codecache::retriever::QueryResult;
use codecache::storage::SearchResult;
use codecache::types::{Chunk, Language, SymbolType};

// ───────────────────────────── fixture construction ─────────────────────────────

/// Build one `SearchResult` with full control over the fields the formatter emits.
#[allow(clippy::too_many_arguments)]
fn result_of(
    name: &str,
    symbol_type: SymbolType,
    file: &str,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
    language: Language,
    parent: Option<&str>,
    chunk_text: &str,
    bm25_score: f64,
) -> SearchResult {
    SearchResult {
        chunk: Chunk {
            symbol_name: name.to_string(),
            symbol_type,
            file_path: PathBuf::from(file),
            start_byte,
            end_byte,
            start_line,
            end_line,
            chunk_text: chunk_text.to_string(),
            language,
            parent_symbol: parent.map(|p| p.to_string()),
            file_docstring: None,
            imports: Vec::new(),
            cross_references: Vec::new(),
            is_heuristic: false,
        },
        bm25_score,
    }
}

/// The shared, deterministic fixture used by the TOON/JSON/text golden tests:
/// 3 chunks across 2 files, distinct (already best-first) BM25 scores and line ranges, with the
/// middle chunk carrying a `parent_symbol` so the qualified-parent ordering (D13) is exercised.
///
/// `total_results_found = 5` (> chunks.len() == 3) so the "showing top N of M" wording in the text
/// header and the §6.4.2 `total_results` JSON key are pinned to the pre-budget count, not 3.
fn basic_result() -> QueryResult {
    let c1 = result_of(
        "authenticate_user",
        SymbolType::Function,
        "src/auth/handlers.py",
        1234,
        1789,
        45,
        67,
        Language::Python,
        None,
        "def authenticate_user(username: str, password: str) -> Optional[User]:\n    \"\"\"Authenticate user with username and password.\"\"\"\n    user = get_user_by_username(username)\n    if user and verify_password(password, user.password_hash):\n        return user\n    return None",
        -2.45,
    );
    let c2 = result_of(
        "verify_password",
        SymbolType::Method,
        "src/auth/handlers.py",
        1900,
        2100,
        70,
        72,
        Language::Python,
        Some("AuthService"),
        "def verify_password(self, plain: str, hashed: str) -> bool:\n    \"\"\"Verify a plaintext password against its hash.\"\"\"\n    return bcrypt.checkpw(plain.encode(), hashed.encode())",
        -1.89,
    );
    let c3 = result_of(
        "hash_password",
        SymbolType::Function,
        "src/auth/utils.py",
        300,
        460,
        12,
        14,
        Language::Python,
        None,
        "def hash_password(plain: str) -> str:\n    \"\"\"Hash a plaintext password with bcrypt.\"\"\"\n    return bcrypt.hashpw(plain.encode(), bcrypt.gensalt()).decode()",
        -1.20,
    );
    QueryResult {
        chunks: vec![c1, c2, c3],
        total_tokens: 142,
        total_results_found: 5,
    }
}

/// An empty result: no chunks, 0 tokens, 0 found. Exercises the no-result path of all 3 formats.
fn empty_result() -> QueryResult {
    QueryResult {
        chunks: Vec::new(),
        total_tokens: 0,
        total_results_found: 0,
    }
}

// ───────────────────────────── golden-file helpers ─────────────────────────────

/// Absolute path to a committed golden file.
fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden")
        .join(name)
}

/// Read a golden file, normalizing CRLF -> LF so checkout line-ending policy can't flake the test.
fn read_golden(name: &str) -> String {
    let raw = std::fs::read_to_string(golden_path(name))
        .unwrap_or_else(|e| panic!("read golden {name}: {e}"));
    raw.replace("\r\n", "\n")
}

/// Normalize for line-oriented comparison: CRLF -> LF and strip a single trailing newline, so the
/// contract is "exact internal content" without coupling to a final newline either way.
fn norm(s: &str) -> String {
    let s = s.replace("\r\n", "\n");
    s.strip_suffix('\n').map(|t| t.to_string()).unwrap_or(s)
}

// ───────────────────────────── TOON ─────────────────────────────

#[test]
fn toon_format_emits_file_line_pairs_sorted_by_score() {
    // TOON is the compact, locator-only format (§6.4.1, normative): one `file:start-end` line per
    // result, in the order the chunks arrive (already best-first by BM25 — most negative first).
    // No bodies, no signatures, no headers — it pipes directly to `cat`/an editor. D13 agent-first
    // ordering lives in the TEXT format only; TOON carries no signature/body at all.
    let qr = basic_result();
    let out = format(&qr, "authenticate user", Format::Toon);

    // The output is EXACTLY one `file:start-end` line per result, nothing else. The stored 1-based
    // inclusive line ranges come from start_line/end_line (D7), NOT byte offsets.
    let normed = norm(&out);
    let lines: Vec<&str> = normed.lines().collect();
    assert_eq!(
        lines,
        vec![
            "src/auth/handlers.py:45-67",
            "src/auth/handlers.py:70-72",
            "src/auth/utils.py:12-14",
        ],
        "TOON must be the compact bare list of file:start-end ranges in BM25 order:\n{out}"
    );

    // No bodies/signatures/headers leak into the locator-only format.
    assert!(
        !out.contains("def authenticate_user"),
        "TOON must not carry chunk bodies/signatures:\n{out}"
    );
    assert!(
        !out.contains("Query:") && !out.contains("Found "),
        "TOON must not carry a text-style header:\n{out}"
    );

    // Byte offsets must not leak in as the displayed range (D7 — line numbers, not bytes).
    assert!(
        !out.contains(":1234-1789"),
        "TOON leaked byte offsets instead of line range:\n{out}"
    );

    // Exact golden match (line-oriented; trailing newline normalized).
    assert_eq!(norm(&out), norm(&read_golden("query_basic.toon")));
}

// ───────────────────────────── JSON ─────────────────────────────

#[test]
fn json_format_is_valid_and_matches_golden() {
    // §6.4.2 schema: top-level `query`, `total_results`, `total_tokens`, `chunks[]` with
    // `symbol_name`, `symbol_type`, `file_path`, `start_byte`, `end_byte`, `language`,
    // `bm25_score`, `chunk_text`. The top-level key is `total_results` mapped from
    // `total_results_found`.
    let qr = basic_result();
    let out = format(&qr, "authenticate user", Format::Json);

    let v: serde_json::Value =
        serde_json::from_str(&out).expect("formatter JSON output must be valid JSON");

    assert_eq!(v["query"], serde_json::json!("authenticate user"));
    assert_eq!(v["total_results"], serde_json::json!(5)); // from total_results_found, NOT 3
    assert_eq!(v["total_tokens"], serde_json::json!(142));

    let chunks = v["chunks"].as_array().expect("chunks is an array");
    assert_eq!(chunks.len(), 3, "all packed chunks serialized");

    // First chunk: every documented field present with the right value.
    let c0 = &chunks[0];
    assert_eq!(c0["symbol_name"], serde_json::json!("authenticate_user"));
    assert_eq!(c0["symbol_type"], serde_json::json!("function"));
    assert_eq!(c0["file_path"], serde_json::json!("src/auth/handlers.py"));
    assert_eq!(c0["start_byte"], serde_json::json!(1234));
    assert_eq!(c0["end_byte"], serde_json::json!(1789));
    assert_eq!(c0["language"], serde_json::json!("python"));
    assert_eq!(c0["bm25_score"], serde_json::json!(-2.45));
    assert!(
        c0["chunk_text"]
            .as_str()
            .expect("chunk_text is a string")
            .starts_with("def authenticate_user("),
        "chunk_text must carry the full source text"
    );

    // The method chunk serializes its symbol_type correctly.
    assert_eq!(chunks[1]["symbol_type"], serde_json::json!("method"));

    // Exact value-equality against the committed golden (robust to whitespace / key order).
    let golden: serde_json::Value =
        serde_json::from_str(&read_golden("query_basic.json")).expect("golden JSON valid");
    assert_eq!(v, golden, "JSON output does not match golden");
}

#[test]
fn json_round_trips_to_queryresult() {
    // Emit -> parse back -> assert the round-tripped values equal the inputs (at least the fields
    // the JSON transport carries). serde_json is already a dependency.
    let qr = basic_result();
    let out = format(&qr, "authenticate user", Format::Json);
    let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");

    // Top-level counts survive the round trip.
    assert_eq!(
        v["total_results"].as_u64().expect("total_results u64") as usize,
        qr.total_results_found
    );
    assert_eq!(
        v["total_tokens"].as_u64().expect("total_tokens u64") as usize,
        qr.total_tokens
    );
    assert_eq!(v["query"].as_str().expect("query str"), "authenticate user");

    // Each chunk's carried fields equal the source chunk's.
    let chunks = v["chunks"].as_array().expect("chunks array");
    assert_eq!(chunks.len(), qr.chunks.len());
    for (jc, sr) in chunks.iter().zip(qr.chunks.iter()) {
        let c = &sr.chunk;
        assert_eq!(jc["symbol_name"].as_str().expect("name"), c.symbol_name);
        assert_eq!(
            jc["symbol_type"].as_str().expect("type"),
            c.symbol_type.as_str()
        );
        assert_eq!(
            jc["file_path"].as_str().expect("path"),
            c.file_path.to_string_lossy()
        );
        assert_eq!(
            jc["start_byte"].as_u64().expect("start_byte") as usize,
            c.start_byte
        );
        assert_eq!(
            jc["end_byte"].as_u64().expect("end_byte") as usize,
            c.end_byte
        );
        assert_eq!(
            jc["language"].as_str().expect("language"),
            c.language.as_str()
        );
        assert_eq!(
            jc["bm25_score"].as_f64().expect("bm25_score"),
            sr.bm25_score
        );
        assert_eq!(jc["chunk_text"].as_str().expect("chunk_text"), c.chunk_text);
    }
}

// ───────────────────────────── TEXT ─────────────────────────────

#[test]
fn text_format_matches_golden_human_readable() {
    // §6.4.3 layout: a header echoing the query + result/token counts, then `[n] file:start-end
    // (score: …)` blocks. Compare to the committed golden.
    let qr = basic_result();
    let out = format(&qr, "authenticate user", Format::Text);

    // Header echoes the query and the pre-budget result count + shown count + token count.
    assert!(
        out.contains("authenticate user"),
        "text header must echo the query:\n{out}"
    );
    assert!(
        out.contains("Found 5 results"),
        "text header must report pre-budget total (5):\n{out}"
    );
    assert!(
        out.contains("142 tokens"),
        "text header must report token count:\n{out}"
    );

    // Numbered result blocks with file:line range (D7) and score.
    assert!(
        out.contains("[1]") && out.contains("src/auth/handlers.py:45-67"),
        "first result block missing:\n{out}"
    );
    assert!(
        out.contains("(score: -2.45)"),
        "first result must show its bm25 score:\n{out}"
    );

    assert_eq!(norm(&out), norm(&read_golden("query_basic.txt")));
}

// ───────────────────────────── empty result ─────────────────────────────

#[test]
fn empty_result_formats_cleanly_in_all_three() {
    // No chunks, 0 tokens, 0 found. Each format must produce well-formed output without panic.
    let qr = empty_result();

    // TOON: no result lines (matches the empty golden — likely the empty string).
    let toon = format(&qr, "nothing matches here", Format::Toon);
    assert_eq!(norm(&toon), norm(&read_golden("query_empty.toon")));

    // JSON: valid, query echoed, empty chunks, zero counts.
    let json = format(&qr, "nothing matches here", Format::Json);
    let v: serde_json::Value = serde_json::from_str(&json).expect("empty JSON still valid");
    assert_eq!(v["query"], serde_json::json!("nothing matches here"));
    assert_eq!(v["total_results"], serde_json::json!(0));
    assert_eq!(v["total_tokens"], serde_json::json!(0));
    assert!(
        v["chunks"].as_array().expect("chunks array").is_empty(),
        "empty result must serialize chunks as []"
    );
    let golden: serde_json::Value =
        serde_json::from_str(&read_golden("query_empty.json")).expect("golden empty JSON valid");
    assert_eq!(v, golden);

    // Text: header present, no result blocks (no `[1]`).
    let text = format(&qr, "nothing matches here", Format::Text);
    assert!(
        text.contains("Found 0 results"),
        "empty text must report 0 results:\n{text}"
    );
    assert!(
        !text.contains("[1]"),
        "empty text must contain no result blocks:\n{text}"
    );
    assert_eq!(norm(&text), norm(&read_golden("query_empty.txt")));
}

// ───────────────────────────── D13 agent-first ordering (text only) ─────────────────────────────

#[test]
fn text_orders_agent_first() {
    // §8.2 (D13): in the TEXT format, for each result the symbol name, qualified parent,
    // `file:start-end`, and the one-line signature appear BEFORE the full body. A regression to
    // "body-first" must fail. (TOON is locator-only and carries no body, so D13 applies to text.)
    let qr = basic_result();
    let out = format(&qr, "authenticate user", Format::Text);

    // The middle chunk has parent `AuthService`; its qualified name is `AuthService.verify_password`.
    // Its one-line signature is the first line of chunk_text; a body line is a later line.
    let sig = "def verify_password(self, plain: str, hashed: str) -> bool:";
    let body = "return bcrypt.checkpw(plain.encode(), hashed.encode())";

    // Qualified parent is rendered for the method chunk.
    assert!(
        out.contains("AuthService.verify_password"),
        "qualified parent missing in text:\n{out}"
    );

    // The metadata line (symbol/parent + range), the one-line signature, and the body line.
    let qual_pos = out
        .find("AuthService.verify_password")
        .expect("qualified name present");
    let range_pos = out
        .find("src/auth/handlers.py:70-72")
        .expect("range present");
    let sig_pos = out.find(sig).expect("signature line present");
    let body_pos = out.find(body).expect("body line present");

    assert!(
        qual_pos < body_pos,
        "text: qualified name must precede body:\n{out}"
    );
    assert!(
        range_pos < body_pos,
        "text: file:start-end must precede body:\n{out}"
    );
    assert!(
        sig_pos < body_pos,
        "text: signature line must precede body (agent-first, D13):\n{out}"
    );

    // The metadata/range precede the signature, which precedes the body — body is strictly last.
    assert!(
        qual_pos < sig_pos && range_pos < sig_pos,
        "text: metadata + range must precede the signature line:\n{out}"
    );

    // The text golden itself encodes the ordering, so a body-first regression fails the exact-match
    // assertion too.
    assert_eq!(norm(&out), norm(&read_golden("query_basic.txt")));
}
