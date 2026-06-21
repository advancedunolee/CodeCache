//! File discovery + language detection for the indexer (slice M5.1).
//!
//! API anchor: `project_plan.md` §3.2.4 / §5.1. Owner: `principal-engineering-lead`.
//! Scenarios: `docs/TEST_STRATEGY.md#indexer` (discovery rows) + `.claude/briefs/BRIEF-M5-indexer.md`.
//!
//! [`discover_files`] walks `config.index_paths` resolved against `root` (defaulting to `root`
//! itself when `index_paths` is empty), honoring `.gitignore` automatically via
//! [`ignore::WalkBuilder`], folding in the built-in [`DEFAULT_IGNORE_PATTERNS`] when
//! `config.use_default_ignores` is set (§7.3 / D32) and applying `config.ignore_patterns` as
//! additional gitignore-style globs (which extend, never replace, the defaults), and restricting
//! results to source files whose [`detect_language`] is in `config.languages`. No reachable
//! `unwrap()/expect()/panic!` — every fallible step surfaces an [`IndexError`].

use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::WalkBuilder;

use crate::config::Config;
use crate::types::Language;

use super::IndexError;

/// Detect a source [`Language`] from a path's file extension.
///
/// Maps the three v0.1 languages by extension (`.py`→Python, `.ts`→TypeScript, `.go`→Go) and
/// returns `None` for every other extension and for extension-less paths.
pub fn detect_language(path: &Path) -> Option<Language> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("py") => Some(Language::Python),
        Some("ts") => Some(Language::TypeScript),
        Some("go") => Some(Language::Go),
        _ => None,
    }
}

/// Discover source files under `root` that should be indexed.
///
/// Walks each entry of `config.index_paths` resolved against `root` (or `root` itself when
/// `index_paths` is empty), honoring `.gitignore` via [`ignore::WalkBuilder`]. A file is returned
/// only when it survives the ignore matcher — the built-in [`DEFAULT_IGNORE_PATTERNS`] (when
/// `config.use_default_ignores`) plus `config.ignore_patterns`, both applied as gitignore-style
/// globs — **and** its [`detect_language`] is one of `config.languages`. Returned paths are joined
/// under `root`.
///
/// # Errors
/// Returns [`IndexError::Io`] if a walk entry cannot be read, or [`IndexError::Glob`] if a
/// `config.ignore_patterns` (or built-in default) entry is not a valid gitignore-style glob.
pub fn discover_files(config: &Config, root: &Path) -> Result<Vec<PathBuf>, IndexError> {
    let ignore = build_ignore_patterns(config, root)?;

    let mut files = Vec::new();
    for walk_root in resolve_walk_roots(config, root) {
        // `require_git(false)` so `.gitignore` rules are honored even when the tree is not inside
        // a git repository (the indexer indexes plain source trees, not just checkouts).
        let walker = WalkBuilder::new(&walk_root).require_git(false).build();
        for entry in walker {
            let entry = entry.map_err(|source| IndexError::Io {
                path: walk_root.clone(),
                source,
            })?;
            let path = entry.path();
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            if !is_configured_language(config, path) {
                continue;
            }
            if ignore.matched_path_or_any_parents(path, false).is_ignore() {
                continue;
            }
            files.push(path.to_path_buf());
        }
    }
    Ok(files)
}

/// The set of roots to walk: each `config.index_paths` entry joined under `root`, or `root` itself
/// when `index_paths` is empty (the `Config::default()` case).
fn resolve_walk_roots(config: &Config, root: &Path) -> Vec<PathBuf> {
    if config.index_paths.is_empty() {
        vec![root.to_path_buf()]
    } else {
        config.index_paths.iter().map(|p| root.join(p)).collect()
    }
}

/// Built-in default ignore globs applied during discovery when `config.use_default_ignores`
/// is true (the default) — independent of `.gitignore` and of `config.ignore_patterns`, which
/// EXTENDS this set. See project_plan.md §7.3 / Decision Log D32. `.git/` is harmless (already
/// hidden-skipped by `WalkBuilder`) but listed for explicitness.
pub(crate) const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "env/",
    ".venv/",
    "venv/",
    "node_modules/",
    "__pycache__/",
    "*.pyc",
    "target/",
    "dist/",
    "build/",
    ".git/",
];

/// Build a [`Gitignore`] matcher anchored at `root` so relative globs (`vendor/**`,
/// `*_generated.py`) match the same way `.gitignore` entries would. When
/// `config.use_default_ignores` is true (the default), [`DEFAULT_IGNORE_PATTERNS`] are folded in
/// first (§7.3 / D32), then `config.ignore_patterns` extends — never replaces — that set.
fn build_ignore_patterns(config: &Config, root: &Path) -> Result<Gitignore, IndexError> {
    let mut builder = GitignoreBuilder::new(root);
    if config.use_default_ignores {
        for pattern in DEFAULT_IGNORE_PATTERNS {
            builder
                .add_line(None, pattern)
                .map_err(|source| IndexError::Glob {
                    pattern: pattern.to_string(),
                    source,
                })?;
        }
    }
    for pattern in &config.ignore_patterns {
        builder
            .add_line(None, pattern)
            .map_err(|source| IndexError::Glob {
                pattern: pattern.clone(),
                source,
            })?;
    }
    builder.build().map_err(|source| IndexError::Glob {
        pattern: config.ignore_patterns.join(", "),
        source,
    })
}

/// Whether `path` is a source file whose language is in `config.languages`.
fn is_configured_language(config: &Config, path: &Path) -> bool {
    detect_language(path).is_some_and(|lang| config.languages.contains(&lang))
}
