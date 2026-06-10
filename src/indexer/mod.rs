//! Indexer: orchestrate file discovery → parse → chunk → hash → store; incremental updates.
//!
//! API anchor: `project_plan.md` §3.2.4 / §5.1 / §5.2. Owner: `principal-engineering-lead`.
//! Scenarios: `docs/TEST_STRATEGY.md#indexer`. M0: empty stub; implemented at M5.
//!
//! Slice **M5.1** ships file discovery + language detection (see [`discovery`]); the full/
//! incremental pipeline (`index_all`/`update_files`) lands in M5.2+.

mod discovery;

pub use discovery::{detect_language, discover_files};

/// A typed indexer error. Wraps the failures that can occur while discovering and indexing files
/// (filesystem walk errors, invalid ignore-pattern globs). Never panics; carries enough context
/// to report what went wrong.
#[derive(Debug)]
pub enum IndexError {
    /// A filesystem walk entry under `path` could not be read (missing, unreadable, permissions, …).
    Io {
        /// The walk root whose traversal failed.
        path: std::path::PathBuf,
        /// The underlying walk error.
        source: ignore::Error,
    },
    /// A `config.ignore_patterns` entry is not a valid gitignore-style glob.
    Glob {
        /// The offending pattern (or the joined pattern set when the failure is build-wide).
        pattern: String,
        /// The underlying glob-compilation error.
        source: ignore::Error,
    },
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::Io { path, source } => {
                write!(f, "failed to walk '{}': {source}", path.display())
            }
            IndexError::Glob { pattern, source } => {
                write!(f, "invalid ignore pattern '{pattern}': {source}")
            }
        }
    }
}

impl std::error::Error for IndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IndexError::Io { source, .. } => Some(source),
            IndexError::Glob { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {}
