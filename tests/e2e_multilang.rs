//! End-to-end cross-language tests for the public `init → index` surface — slice **M9.3**.
//!
//! This is a **validation** slice. M5 discovery (`detect_language`) already maps `.py/.ts/.go`,
//! M9.1 wired the TypeScript parser, and M9.2 wired the Go parser, so the full pipeline
//! (discovery → parse → chunk → store → search) is expected to handle all three v0.1 languages
//! already. These tests do not drive new production code; they PROVE the validation by indexing a
//! mixed Python/TypeScript/Go repo through the public library surface and asserting real values
//! (file counts, the pinned chunk total, and the searchable signature symbol of each language).
//! If they pass on first run the validation is confirmed (GREEN by construction, which is fine for
//! a validation milestone); if any fails, that is a genuine gap to escalate — not to paper over.
//!
//! Scenarios from `.claude/briefs/BRIEF-M9-typescript-go.md` (slice M9.3) +
//! `docs/TEST_STRATEGY.md`. The helpers (`temp_repo`/`write_file`/`default_db_path`/
//! `searchable_symbols`) are copied from `tests/e2e_index.rs` deliberately — per the brief, do NOT
//! refactor `e2e_index.rs` into a shared module; each e2e file stands alone.
//!
//! Repos are built at runtime under a `tempfile::TempDir` (no committed fixtures), so the tests are
//! deterministic and parallel-safe. Every search-set assertion sorts + dedups before comparing.

use std::fs;
use std::path::{Path, PathBuf};

use codecache::storage::Storage;
use codecache::{index, init, IndexStats};
use tempfile::TempDir;

// ───────────────────────────── fixture helpers ─────────────────────────────
// (copied verbatim from tests/e2e_index.rs — same public-surface discipline.)

/// Create a temp project root for one test. Removed when the returned `TempDir` is dropped.
fn temp_repo() -> TempDir {
    tempfile::tempdir().expect("create temp repo dir")
}

/// Write `contents` to `root/rel`, creating parent directories as needed.
fn write_file(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(&path, contents).expect("write fixture file");
}

/// The default DB location `init`/`index` resolve `config.storage.db_path` to under `root`.
fn default_db_path(root: &Path) -> PathBuf {
    root.join(".codecache").join("index.db")
}

/// Re-open the resulting on-disk DB via the public `Storage` API and collect the sorted, deduped
/// symbol names matching `query` — a stable observable for "is this symbol queryable?".
fn searchable_symbols(db_path: &Path, query: &str) -> Vec<String> {
    let storage = Storage::new(db_path).expect("re-open indexed db");
    let mut names: Vec<String> = storage
        .search(query, 100)
        .expect("search must succeed on a populated db")
        .into_iter()
        .map(|h| h.chunk.symbol_name)
        .collect();
    names.sort();
    names.dedup();
    names
}

// ───────────────────────────── mixed-repo fixture ─────────────────────────────

/// Build a tiny, fully-controlled mixed-language repo: exactly one source file per v0.1 language,
/// each with exactly ONE top-level definition. Keeping every file to a single top-level symbol
/// pins the expected chunk total to 3 (one chunk per file):
///
/// - `auth.py`   — `def authenticate_user(): ...`            ⇒ 1 chunk (Python function).
/// - `handler.ts`— `function handleRequest(...) {...}`        ⇒ 1 chunk (TypeScript function).
/// - `server.go` — `package main` + `func StartServer() {...}`⇒ 1 chunk (Go function; the
///   `package main` clause produces no chunk, per M9.2 `package_and_imports_handled`).
///
/// Total: 3 files / 3 chunks. Each file's signature symbol is the language's observable that the
/// full pipeline carried it through discovery → parse → chunk → store → search.
fn build_mixed_repo(root: &Path) {
    write_file(
        root,
        "auth.py",
        "def authenticate_user():\n    return True\n",
    );
    write_file(
        root,
        "handler.ts",
        "function handleRequest(path: string): string {\n  return \"ok \" + path;\n}\n",
    );
    write_file(
        root,
        "server.go",
        "package main\n\nfunc StartServer() string {\n\treturn \"up\"\n}\n",
    );
}

// ══════════════════ M9.3 — cross-language integration through the indexer ══════════════════

#[test]
fn index_mixed_repo_indexes_python_ts_and_go_files() {
    // Build a mixed Python/TypeScript/Go repo, init + index it through the public surface, and
    // assert that all three languages flow through discovery → parse → chunk → store → search:
    // the file count, the pinned chunk total, and each language's signature symbol being queryable.
    let repo = temp_repo();
    let root = repo.path();
    build_mixed_repo(root);

    init(root).expect("init must succeed on a fresh mixed-language project");
    let stats: IndexStats = index(root).expect("index must succeed on a mixed-language repo");

    // All three source files (one per language) are discovered and processed.
    assert_eq!(
        stats.files_processed, 3,
        "the Python, TypeScript, and Go source files must all be discovered + indexed"
    );

    // Chunk arithmetic (pinned by the single-top-level-symbol fixtures):
    //   auth.py    → 1 (authenticate_user)
    //   handler.ts → 1 (handleRequest)
    //   server.go  → 1 (StartServer; `package main` emits no chunk)
    //   total      = 3
    assert_eq!(
        stats.chunks_indexed, 3,
        "1 (Python fn) + 1 (TS fn) + 1 (Go fn) = 3 chunks; package/clauses emit none"
    );

    // Each language's signature symbol must be searchable through the re-opened DB. This is the
    // proof that TS and Go chunks are stored in the identical, searchable shape as Python's.
    let db_path = default_db_path(root);
    assert_eq!(
        searchable_symbols(&db_path, "authenticate_user"),
        vec!["authenticate_user".to_string()],
        "the Python function must be searchable in the indexed db"
    );
    assert_eq!(
        searchable_symbols(&db_path, "handleRequest"),
        vec!["handleRequest".to_string()],
        "the TypeScript function must be searchable in the indexed db"
    );
    assert_eq!(
        searchable_symbols(&db_path, "StartServer"),
        vec!["StartServer".to_string()],
        "the Go function must be searchable in the indexed db"
    );
}

#[test]
fn language_filter_in_config_restricts_indexed_languages() {
    // The SAME mixed repo, but the config restricts `languages = ["python"]`. Discovery must filter
    // the TypeScript and Go files out BEFORE parsing, so only the Python file is processed and only
    // its symbol is searchable — proving `config.languages` gates the whole pipeline.
    let repo = temp_repo();
    let root = repo.path();
    build_mixed_repo(root);

    // `init` writes the default config (all three languages) and creates the DB. We then overwrite
    // `.codecache/config.toml` with a Python-only `languages` list, reusing the exact default schema
    // and changing only the `languages` line, so the loader accepts it unchanged otherwise.
    init(root).expect("init must succeed");

    let config_path = root.join(".codecache").join("config.toml");
    let default_config = fs::read_to_string(&config_path).expect("read default config init wrote");
    let restricted = restrict_languages_to_python(&default_config);
    assert_ne!(
        restricted, default_config,
        "the restricted config must actually differ from the default (the `languages` line changed)"
    );
    assert!(
        restricted.contains("\"python\"") && !restricted.contains("\"typescript\""),
        "the restricted config must list only python: {restricted}"
    );
    fs::write(&config_path, &restricted).expect("overwrite config with python-only languages");

    let stats: IndexStats = index(root).expect("index must succeed with a python-only filter");

    // Only the Python file survives discovery's language filter.
    assert_eq!(
        stats.files_processed, 1,
        "with languages = [\"python\"], only auth.py is processed (TS + Go filtered out)"
    );
    assert_eq!(
        stats.chunks_indexed, 1,
        "only the single Python function chunk is indexed"
    );

    // The Python symbol IS searchable; the TS and Go symbols are NOT (never parsed/stored).
    let db_path = default_db_path(root);
    assert_eq!(
        searchable_symbols(&db_path, "authenticate_user"),
        vec!["authenticate_user".to_string()],
        "the Python function must be searchable under a python-only filter"
    );
    assert!(
        searchable_symbols(&db_path, "handleRequest").is_empty(),
        "the TypeScript symbol must NOT be searchable — its file was filtered out before parsing"
    );
    assert!(
        searchable_symbols(&db_path, "StartServer").is_empty(),
        "the Go symbol must NOT be searchable — its file was filtered out before parsing"
    );
}

/// Rewrite the `languages = [...]` array in a TOML config to Python-only, preserving every other
/// line of the default config the loader produced. Splitting on the array's brackets keeps this
/// robust to single-line (`languages = ["python", "typescript", "go"]`) or any spacing the TOML
/// serializer emits. Asserts the array was found so the test fails loudly if the schema changes.
fn restrict_languages_to_python(default_config: &str) -> String {
    let key = "languages = [";
    let start = default_config
        .find(key)
        .expect("default config must contain a `languages = [` array");
    let after_open = start + key.len();
    let close_rel = default_config[after_open..]
        .find(']')
        .expect("the `languages` array must be closed with `]`");
    let close = after_open + close_rel;

    let mut out = String::with_capacity(default_config.len());
    out.push_str(&default_config[..start]);
    out.push_str("languages = [\"python\"]");
    out.push_str(&default_config[close + 1..]);
    out
}
