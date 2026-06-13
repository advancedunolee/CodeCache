//! Shared, dependency-free core types.
//!
//! Home of `Chunk`, `Language`, `SymbolType`, and `FileMeta` (Decision Log **D5**) so that both
//! `storage` and `parser`/`chunker` can depend on them without `storage` depending on `parser`,
//! keeping the bottom-up build order acyclic. See `project_plan.md` §4.3 / §3.2.2.
//!
//! Types land at M1 (`storage` needs them).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A semantic unit extracted from source (function, class, method, struct) plus the metadata
/// enrichment used to lift retrieval recall.
///
/// Fields mirror `project_plan.md` §4.3. `start_line`/`end_line` are 1-based and inclusive
/// (Decision Log **D7**); the enrichment fields are Decision Log **D3**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Symbol name, e.g. `authenticate_user`.
    pub symbol_name: String,
    /// Whether this is a function, class, method, or struct.
    pub symbol_type: SymbolType,
    /// Source file the symbol came from.
    pub file_path: PathBuf,
    /// Byte offset of the symbol's first byte in the file.
    pub start_byte: usize,
    /// Byte offset one past the symbol's last byte.
    pub end_byte: usize,
    /// 1-based line of the symbol's first line (D7).
    pub start_line: usize,
    /// 1-based, inclusive line of the symbol's last line (D7).
    pub end_line: usize,
    /// Full source text of the symbol.
    pub chunk_text: String,
    /// Language the symbol was parsed from.
    pub language: Language,

    // Metadata enrichment (Decision Log D3):
    /// Enclosing class/function for methods and nested definitions.
    pub parent_symbol: Option<String>,
    /// Module/file-level docstring, if any.
    pub file_docstring: Option<String>,
    /// Import statements visible in the file.
    pub imports: Vec<String>,
    /// Referenced symbol names within the chunk.
    pub cross_references: Vec<String>,

    // Graceful degradation (Decision Log D2):
    /// `true` when this chunk came from the M4 line-heuristic fallback (the parser's ERROR rate
    /// exceeded `HEURISTIC_FALLBACK_THRESHOLD`) rather than the AST path. AST- and
    /// storage-reconstructed chunks are `false`.
    pub is_heuristic: bool,
}

/// The kind of semantic unit a [`Chunk`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolType {
    /// A free function.
    Function,
    /// A class definition.
    Class,
    /// A method (function defined inside a class).
    Method,
    /// A struct (Go/Rust).
    Struct,
}

impl SymbolType {
    /// The canonical lowercase string stored in the `symbols.symbol_type` column.
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolType::Function => "function",
            SymbolType::Class => "class",
            SymbolType::Method => "method",
            SymbolType::Struct => "struct",
        }
    }

    /// Parse a stored `symbol_type` string back into a [`SymbolType`]. Returns `None` for any
    /// unrecognized value rather than panicking, so corrupt rows degrade gracefully.
    pub fn from_str_lenient(s: &str) -> Option<SymbolType> {
        match s {
            "function" => Some(SymbolType::Function),
            "class" => Some(SymbolType::Class),
            "method" => Some(SymbolType::Method),
            "struct" => Some(SymbolType::Struct),
            _ => None,
        }
    }
}

/// A source language CodeCache can index. v0.1 targets Python, TypeScript, and Go.
///
/// Serializes to/from its canonical lowercase name (`"python"`, `"typescript"`, `"go"`) so it
/// can be used directly in `config.toml` `languages = [...]` entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Python.
    Python,
    /// TypeScript.
    TypeScript,
    /// Go.
    Go,
}

impl Language {
    /// The canonical lowercase string stored in the `*.language` columns and config files.
    pub fn as_str(self) -> &'static str {
        match self {
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::Go => "go",
        }
    }

    /// Parse a stored/config `language` string into a [`Language`]. Returns `None` for any
    /// unrecognized value rather than panicking.
    pub fn from_str_lenient(s: &str) -> Option<Language> {
        match s {
            "python" => Some(Language::Python),
            "typescript" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            _ => None,
        }
    }
}

/// A slim, path-scoped symbol projection backing the `codecache_outline` MCP tool (Decision Log
/// **D19**, plan §3.2.2 / §8.2). Holds only the skeleton fields — name, type, parent, and the D7
/// 1-based inclusive line range — with no `chunk_text`/`imports`, so an outline of a whole
/// directory stays within the §11.2 budget. Produced by [`crate::storage::Storage::symbols_for_path`]
/// straight off the stored columns (zero source reads, D7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolOutline {
    /// Symbol name, e.g. `authenticate_user`.
    pub symbol_name: String,
    /// Whether this is a function, class, method, or struct.
    pub symbol_type: SymbolType,
    /// Enclosing class/function for methods and nested definitions, if any.
    pub parent_symbol: Option<String>,
    /// Source file the symbol came from.
    pub file_path: PathBuf,
    /// 1-based line of the symbol's first line (D7).
    pub start_line: usize,
    /// 1-based, inclusive line of the symbol's last line (D7).
    pub end_line: usize,
}

/// Write-side metadata bundle for a `files_metadata` row (Decision Log **D6**).
///
/// Carries everything `Storage::update_file_hash` needs to persist a file's row in one call, so
/// M5's incremental indexer records the hash *and* the §4.1 bookkeeping fields together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    /// xxHash3-128 of the file content, as a 32-char hex string.
    pub content_hash: String,
    /// File modification time, Unix epoch seconds.
    pub mtime: u64,
    /// File size in bytes.
    pub file_size: u64,
    /// Language the file was indexed as.
    pub language: Language,
    /// Number of symbols extracted from the file.
    pub chunk_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn chunk_carries_all_documented_fields_incl_line_range_and_enrichment() {
        // Pins the §4.3 / D7 / D3 shape: byte span + 1-based inclusive line range + enrichment.
        let c = Chunk {
            symbol_name: "authenticate_user".to_string(),
            symbol_type: SymbolType::Function,
            file_path: PathBuf::from("src/auth/handlers.py"),
            start_byte: 1234,
            end_byte: 1789,
            start_line: 45,
            end_line: 67,
            chunk_text: "def authenticate_user(): ...".to_string(),
            language: Language::Python,
            parent_symbol: Some("AuthService".to_string()),
            file_docstring: Some("auth handlers".to_string()),
            imports: vec!["bcrypt".to_string()],
            cross_references: vec!["verify_password".to_string()],
            is_heuristic: false,
        };
        assert_eq!(c.start_line, 45);
        assert_eq!(c.end_line, 67);
        assert_eq!(c.symbol_type, SymbolType::Function);
        assert_eq!(c.language, Language::Python);
        assert_eq!(c.parent_symbol.as_deref(), Some("AuthService"));
    }

    #[test]
    fn file_meta_carries_d6_write_side_bundle() {
        let m = FileMeta {
            content_hash: "0123456789abcdef0123456789abcdef".to_string(),
            mtime: 1_700_000_000,
            file_size: 2048,
            language: Language::Go,
            chunk_count: 7,
        };
        assert_eq!(m.chunk_count, 7);
        assert_eq!(m.language, Language::Go);
    }

    #[test]
    fn language_str_round_trips_for_schema_storage() {
        // storage persists language as text ('python'/'typescript'/'go'); the mapping must be
        // total and reversible.
        for lang in [Language::Python, Language::TypeScript, Language::Go] {
            let s = lang.as_str();
            assert_eq!(Language::from_str_lenient(s), Some(lang));
        }
        assert_eq!(Language::Python.as_str(), "python");
        assert_eq!(Language::TypeScript.as_str(), "typescript");
        assert_eq!(Language::Go.as_str(), "go");
    }

    #[test]
    fn symbol_type_str_round_trips_for_schema_storage() {
        for st in [
            SymbolType::Function,
            SymbolType::Class,
            SymbolType::Method,
            SymbolType::Struct,
        ] {
            let s = st.as_str();
            assert_eq!(SymbolType::from_str_lenient(s), Some(st));
        }
    }
}
