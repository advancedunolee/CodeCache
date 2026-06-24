//! M7.4 — full end-to-end through the BUILT `codecache` binary (RED, test-lead).
//!
//! This is the dedicated full-lifecycle + exit-code lock-in file. Every test drives the
//! actual compiled binary as a subprocess (`assert_cmd::Command::cargo_bin("codecache")`)
//! with its working directory set to a fresh `tempfile::TempDir` (`.current_dir(tmp)`), so
//! the WHOLE pipeline — cwd-relative `.codecache/` creation, db-path resolution, indexing,
//! retrieval, formatting, and process exit-code mapping — runs through `main.rs` on a real,
//! on-disk fixture repo. Nothing reaches into the library internals; the only contract under
//! test is "what the binary does to stdout/stderr/exit-code".
//!
//! Relationship to `tests/cli_tests.rs`: that file pins parsing (M7.2) and per-handler
//! behavior (M7.3). This file (M7.4) is the dedicated end-to-end chain plus the explicit
//! failure-path / exit-code coverage:
//!   * happy path: `init` → `index` → `query` (text + json) all succeed and emit the symbol;
//!   * failure paths: query-before-init and an operation pointed at a missing db-path both
//!     exit NONZERO with a stderr message and never panic/segfault;
//!   * idempotency: a second `index` still exits 0.
//!
//! RED rationale: M7.3 shipped the handlers, so the HAPPY-PATH tests are expected to be
//! GREEN-on-arrival (that is legitimate — M7.4's value is the dedicated e2e file + the
//! failure-path/exit-code lock-in). They are nonetheless written to FAIL if any wiring is
//! wrong: they assert real stdout content (the queried symbol + a `file:line` locator, valid
//! JSON whose `chunks[]` carries the symbol) and exact exit codes, not just `.is_ok()`. The
//! FAILURE-path tests assert NONZERO + non-empty stderr; if a handler currently exits 0 where
//! it must fail, that is a real RED for the eng-lead to fix in GREEN.
//!
//! Fixture (committed, deterministic): `tests/fixtures/python/enriched_module.py` — copied
//! into each temp root as `module.py`. It defines the free function `hash_password`, the
//! class `UserService`, and the method `register` (1 Python file / 3 chunks). The stable,
//! clearly-named query target is `hash_password`. We copy the committed fixture into the temp
//! dir rather than indexing the CodeCache repo itself, so the fixture stays deterministic.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;

/// The committed Python fixture indexed by the happy-path tests. Embedded at compile time and
/// written into each test's temp project root as `module.py`.
const ENRICHED_MODULE: &str = include_str!("fixtures/python/enriched_module.py");

/// Fresh handle to the built binary for each invocation (parallel-safe: no shared state).
fn cc() -> Command {
    Command::cargo_bin("codecache").expect("binary `codecache` should build")
}

/// Fresh binary handle whose working directory is `root` — exercises the cwd-relative
/// `.codecache/` creation + db-path resolution the lifecycle depends on end-to-end.
fn cc_in(root: &Path) -> Command {
    let mut cmd = cc();
    cmd.current_dir(root);
    cmd
}

/// A temp project root containing exactly one real `.py` source file (`module.py`, the
/// enriched fixture). The returned `TempDir` cleans itself up on drop.
fn temp_project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("create temp project dir");
    fs::write(tmp.path().join("module.py"), ENRICHED_MODULE).expect("write fixture source");
    tmp
}

/// An EMPTY temp dir with NO `.codecache/` and no source files — for the failure paths that
/// must run before (or without) any index existing.
fn empty_temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create empty temp dir")
}

// ───────────────────────────────────────────────────────────────────────────
// 1. Headline happy path: init → index → query, chained through the binary.
//    Each step exits 0; `index` reports it processed files/chunks; the text
//    query surfaces the symbol AND a `module.py:<line>` locator (D7 line info).
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn e2e_init_index_query_happy_path() {
    let tmp = temp_project();
    let root = tmp.path();

    // init → exit 0, and the on-disk index artifacts now exist.
    cc_in(root).arg("init").assert().success();
    assert!(
        root.join(".codecache").join("config.toml").is_file(),
        "init must create .codecache/config.toml"
    );
    assert!(
        root.join(".codecache").join("index.db").is_file(),
        "init must create .codecache/index.db"
    );

    // index → exit 0, and stdout reports that it indexed file(s) + chunk(s).
    cc_in(root)
        .arg("index")
        .assert()
        .success()
        // The handler prints "Indexed N file(s), M chunk(s) in T ms"; pin the report wording
        // and the deterministic counts for the 1-file / 3-chunk enriched fixture.
        .stdout(contains("file"))
        .stdout(contains("chunk"))
        .stdout(contains("1"))
        .stdout(contains("3"));

    // query "hash_password" (default text) → exit 0; stdout carries the symbol AND a
    // `module.py:<start>-<end>` locator (proves Retriever → text formatter end-to-end).
    cc_in(root)
        .args(["query", "hash_password"])
        .assert()
        .success()
        .stdout(contains("hash_password"))
        .stdout(contains("module.py:"));
}

// ───────────────────────────────────────────────────────────────────────────
// 2. JSON output is parseable END-TO-END through the binary, and its `chunks[]`
//    array contains the queried symbol — proving the formatter → stdout path,
//    not just the library-level formatter unit tests.
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn e2e_query_json_is_parseable_end_to_end() {
    let tmp = temp_project();
    let root = tmp.path();

    cc_in(root).arg("init").assert().success();
    cc_in(root).arg("index").assert().success();

    let output = cc_in(root)
        .args(["query", "hash_password", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("query --format json stdout must be valid UTF-8");
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("query --format json must emit parseable JSON");

    // The §6.4.2 schema carries the results under a `chunks` array.
    let chunks = value
        .get("chunks")
        .and_then(|c| c.as_array())
        .expect("JSON output must have a `chunks` array (§6.4.2)");
    assert!(
        !chunks.is_empty(),
        "querying `hash_password` against the indexed fixture must return at least one chunk; got: {value}"
    );

    // At least one chunk must carry the queried symbol name.
    let has_symbol = chunks.iter().any(|chunk| {
        chunk
            .get("symbol_name")
            .and_then(|s| s.as_str())
            .map(|s| s.contains("hash_password"))
            .unwrap_or(false)
    });
    assert!(
        has_symbol,
        "JSON `chunks[]` must contain a chunk whose symbol_name is `hash_password`; got: {value}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// 3. FAILURE PATH — query before init. In a fresh temp dir with NO `.codecache/`,
//    `query` has no index to open and must exit NONZERO with a stderr message
//    (never a silent empty-success). Pins the "not initialized" → nonzero contract.
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn e2e_query_before_init_errors() {
    let tmp = empty_temp_dir();
    let root = tmp.path();

    // Sanity: there is genuinely no index here yet.
    assert!(
        !root.join(".codecache").join("index.db").exists(),
        "precondition: the temp dir must have no index db"
    );

    cc_in(root)
        .args(["query", "anything"])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not())
        // A clean error, never a Rust panic / segfault.
        .stderr(contains("panicked").not())
        .stdout(contains("panicked").not());
}

// ───────────────────────────────────────────────────────────────────────────
// 4. FAILURE PATH — an operation needing an index, pointed at a NONEXISTENT
//    db-path, exits NONZERO with a message and does not panic. `status` against
//    a `--db-path` whose parent directory does not exist cannot open a database,
//    so it must fail cleanly (deterministic: the db file can never be created).
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn e2e_bad_query_or_missing_db_exits_nonzero() {
    let tmp = empty_temp_dir();
    let root = tmp.path();

    // A db-path inside a directory that does not exist — opening it must fail (the parent
    // can't be auto-created), giving a deterministic nonzero with no index present.
    let missing_db = root
        .join("nonexistent_dir")
        .join("index.db")
        .to_string_lossy()
        .into_owned();

    cc_in(root)
        .args(["status", "--db-path", &missing_db])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not())
        .stderr(contains("panicked").not())
        .stdout(contains("panicked").not());
}

// ───────────────────────────────────────────────────────────────────────────
// 5. (optional) Incrementality through the binary: running `index` a second
//    time on unchanged sources still exits 0 (idempotent re-index doesn't choke).
//    The indexer's idempotency is unit/integration-covered; this just proves the
//    binary path survives a re-index and the symbol stays queryable afterward.
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn e2e_index_is_incremental_on_rerun() {
    let tmp = temp_project();
    let root = tmp.path();

    cc_in(root).arg("init").assert().success();
    cc_in(root).arg("index").assert().success();

    // Second index on unchanged sources: still exits 0.
    cc_in(root).arg("index").assert().success();

    // The symbol remains queryable after the re-index.
    cc_in(root)
        .args(["query", "hash_password"])
        .assert()
        .success()
        .stdout(contains("hash_password"));
}

// ───────────────────────────────────────────────────────────────────────────
// 6. (M8.1 cross-cutting) serve --transport sse is cleanly UNSUPPORTED in v0.1.
//    v0.1 ships stdio JSON-RPC only; SSE/HTTP are the deferred D4 adapter seam.
//    Invoking `serve --transport sse` must NOT silently succeed and must NOT
//    panic — it returns a clean "unsupported in v0.1" error and exits NONZERO.
//    Tested at the binary level (assert_cmd, precedent D17 / this file) because
//    that is where the exit-code + stderr contract lives; it is the lightest
//    place to pin "transport is parsed but rejected" end-to-end.
//
//    NOTE for eng-lead: `--transport sse` is currently the M7 serve STUB, which
//    prints a notice and exits 0. M8.1 GREEN must replace that so a non-stdio
//    transport (sse, or any future non-stdio value) errors cleanly + nonzero.
//    The stdio path is NOT asserted here (it would block on stdin); the
//    framing/handshake behavior of the stdio path is covered by mcp_tests.rs
//    against the in-memory `serve(reader, writer, server)` seam.
// ───────────────────────────────────────────────────────────────────────────

// ───────────────────────────────────────────────────────────────────────────
// 7. (D33) `--file-filter` is a GLOB match, not exact-path equality. Before D33
//    ANY `--file-filter` value dropped EVERY result (the CLI wrapped the raw
//    string as a literal `PathBuf` and the retriever compared it for equality
//    against absolute stored paths). These two e2e tests pin the fix end-to-end
//    through the built binary:
//      (a) a matching glob `*.py` over the indexed `.py` fixture KEEPS the hit
//          (was the 0-results bug);
//      (b) a MALFORMED glob `a/[` exits NONZERO with a stderr message and never
//          panics (typed InvalidFilter → clean CLI error).
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn e2e_query_file_filter_glob_keeps_matching_python_hits() {
    let tmp = temp_project();
    let root = tmp.path();

    cc_in(root).arg("init").assert().success();
    cc_in(root).arg("index").assert().success();

    // `*.py` (suffix-anchored to `**/*.py`) matches the indexed `module.py`, so the symbol must
    // still surface — this is the exact case that returned 0 results under the pre-D33 bug.
    cc_in(root)
        .args(["query", "hash_password", "--file-filter", "*.py"])
        .assert()
        .success()
        .stdout(contains("hash_password"))
        .stdout(contains("module.py:"));
}

#[test]
fn e2e_query_malformed_file_filter_glob_exits_nonzero() {
    let tmp = temp_project();
    let root = tmp.path();

    cc_in(root).arg("init").assert().success();
    cc_in(root).arg("index").assert().success();

    // A malformed glob (unclosed character class) must fail cleanly: nonzero exit, a stderr
    // message, and NO Rust panic / segfault. (Pre-D33 this path didn't glob at all, so there was
    // no validation point; D33 routes the typed InvalidFilter to a clean nonzero CLI exit.)
    cc_in(root)
        .args(["query", "hash_password", "--file-filter", "a/["])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not())
        .stderr(contains("panicked").not())
        .stdout(contains("panicked").not());
}

#[test]
fn e2e_serve_unsupported_transport_sse_errors_cleanly() {
    let tmp = temp_project();
    let root = tmp.path();

    // An initialized project so the failure is specifically about the transport, not a missing db.
    cc_in(root).arg("init").assert().success();

    cc_in(root)
        .args(["serve", "--transport", "sse"])
        .assert()
        .failure()
        // A clean, user-facing message — not a Rust panic / segfault.
        .stderr(predicate::str::is_empty().not())
        .stderr(contains("panicked").not())
        .stdout(contains("panicked").not())
        // The message must name the v0.1 limitation (case-insensitive "unsupported").
        .stderr(
            contains("unsupported")
                .or(contains("Unsupported"))
                .or(contains("not supported")),
        );
}
