//! Integration tests for the `indexer` module — slice M5.1 (discovery + language detection).
//!
//! TDD RED: written before `src/indexer/discovery.rs` exists. Scenarios from
//! `docs/plans/M5-indexer.md` (slice M5.1) + `docs/TEST_STRATEGY.md#indexer` +
//! `.claude/briefs/BRIEF-M5-indexer.md`.
//!
//! The public API under test (free functions in the `indexer` module — the "discovery.rs" split
//! per the plan; promoted to `pub` so integration tests can reach the seam):
//! ```ignore
//! pub fn detect_language(path: &Path) -> Option<Language>;
//! pub fn discover_files(config: &Config, root: &Path) -> Result<Vec<PathBuf>, IndexError>;
//! ```
//! `detect_language` maps a path's extension to a [`Language`] (`.py`→Python, `.ts`→TypeScript,
//! `.go`→Go), returning `None` for non-source extensions. `discover_files` walks `config
//! .index_paths` resolved against `root` (defaulting to `root` itself when `index_paths` is
//! empty), honors `.gitignore`, applies `config.ignore_patterns`, and restricts results to
//! `config.languages`.
//!
//! Discovery order is filesystem-dependent, so every assertion sorts results first (determinism).
//! Repos are built at runtime under a `tempfile::TempDir` (no committed fixture tree needed —
//! `.gitignore` is created in-test), keeping the repo clean and tests parallel-safe.

use std::fs;
use std::path::Path;

use codecache::config::Config;
use codecache::indexer::{detect_language, discover_files};
use codecache::types::Language;
use tempfile::TempDir;

// ───────────────────────────── fixture helpers ─────────────────────────────

/// Create a temp repo root for one test. The directory (and everything under it) is removed when
/// the returned `TempDir` is dropped.
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

/// A `Config` whose `index_paths` is empty (⇒ discovery defaults to walking `root` itself) and
/// whose `ignore_patterns` is empty, with the given language set.
fn config_with_languages(languages: Vec<Language>) -> Config {
    Config {
        languages,
        ..Config::default()
    }
}

/// The set of file names (last path component) discovered, sorted for deterministic comparison.
fn discovered_file_names(config: &Config, root: &Path) -> Vec<String> {
    let mut names: Vec<String> = discover_files(config, root)
        .expect("discover_files must succeed on a readable repo")
        .into_iter()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect();
    names.sort();
    names
}

/// Sorted, root-relative path strings (forward-slash normalized) for path-sensitive assertions.
fn discovered_rel_paths(config: &Config, root: &Path) -> Vec<String> {
    let mut rels: Vec<String> = discover_files(config, root)
        .expect("discover_files must succeed on a readable repo")
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(root).unwrap_or(&p);
            rel.to_string_lossy().replace('\\', "/")
        })
        .collect();
    rels.sort();
    rels
}

// ═══════════════ Slice M5.1 — discovery + language detection ═══════════════

#[test]
fn language_detected_from_extension() {
    // `.py`→Python, `.ts`→TypeScript, `.go`→Go; a non-source extension (and no extension) → None.
    assert_eq!(
        detect_language(Path::new("foo/bar.py")),
        Some(Language::Python),
        ".py must detect as Python"
    );
    assert_eq!(
        detect_language(Path::new("foo/bar.ts")),
        Some(Language::TypeScript),
        ".ts must detect as TypeScript"
    );
    assert_eq!(
        detect_language(Path::new("foo/bar.go")),
        Some(Language::Go),
        ".go must detect as Go"
    );
    assert_eq!(
        detect_language(Path::new("README.md")),
        None,
        "a non-source extension must detect as None"
    );
    assert_eq!(
        detect_language(Path::new("Makefile")),
        None,
        "an extension-less file must detect as None"
    );
}

#[test]
fn discovery_only_returns_configured_languages() {
    // languages = [Python]: a repo with a.py, b.ts, c.go returns only a.py.
    let repo = temp_repo();
    let root = repo.path();
    write_file(root, "a.py", "def a():\n    return 1\n");
    write_file(root, "b.ts", "export const b = () => 1;\n");
    write_file(root, "c.go", "package main\nfunc c() int { return 1 }\n");

    let config = config_with_languages(vec![Language::Python]);

    assert_eq!(
        discovered_file_names(&config, root),
        vec!["a.py".to_string()],
        "with languages=[Python], only the .py file is discovered (.ts/.go skipped)"
    );
}

#[test]
fn discovery_respects_gitignore() {
    // A file matched by a `.gitignore` entry must not be returned.
    let repo = temp_repo();
    let root = repo.path();
    write_file(root, "kept.py", "def kept():\n    return 1\n");
    write_file(root, "ignored.py", "def ignored():\n    return 1\n");
    write_file(root, ".gitignore", "ignored.py\n");

    let config = config_with_languages(vec![Language::Python]);

    assert_eq!(
        discovered_file_names(&config, root),
        vec!["kept.py".to_string()],
        "a .gitignore'd file must be excluded from discovery"
    );
}

#[test]
fn discovery_respects_extra_ignore_patterns_from_config() {
    // config.ignore_patterns excludes matching files in addition to .gitignore.
    let repo = temp_repo();
    let root = repo.path();
    write_file(root, "keep.py", "def keep():\n    return 1\n");
    write_file(root, "schema_generated.py", "GENERATED = True\n");
    write_file(root, "vendor/dep.py", "def dep():\n    return 1\n");

    let config = Config {
        languages: vec![Language::Python],
        ignore_patterns: vec!["*_generated.py".to_string(), "vendor/**".to_string()],
        ..Config::default()
    };

    assert_eq!(
        discovered_rel_paths(&config, root),
        vec!["keep.py".to_string()],
        "config.ignore_patterns must exclude *_generated.py and everything under vendor/**"
    );
}

#[test]
fn non_source_files_skipped() {
    // .md, .txt, and an extension-less file are not source files and must not be returned.
    let repo = temp_repo();
    let root = repo.path();
    write_file(root, "code.py", "def code():\n    return 1\n");
    write_file(root, "README.md", "# readme\n");
    write_file(root, "notes.txt", "just notes\n");
    write_file(root, "LICENSE", "MIT\n");

    let config = config_with_languages(vec![Language::Python]);

    assert_eq!(
        discovered_file_names(&config, root),
        vec!["code.py".to_string()],
        "non-source files (.md, .txt, extension-less) must be skipped"
    );
}
