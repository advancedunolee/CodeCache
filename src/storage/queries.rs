//! Prepared SQL and row mapping for `storage`. Kept separate from the `Storage` facade so the
//! SQL strings live next to the schema they target (`schema.rs`).
//!
//! All statements are parameterized; no string interpolation of user input. The FTS5 column
//! order here must match `schema::CREATE_SYMBOLS`.

/// Insert one row into the `symbols` FTS5 table. Column order matches `schema::CREATE_SYMBOLS`.
pub const INSERT_CHUNK: &str = "\
INSERT INTO symbols (
    symbol_name, symbol_type, chunk_text, parent_symbol, imports, cross_references, file_docstring,
    file_path, start_byte, end_byte, start_line, end_line, language
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);";

/// Delete every `symbols` row for one file.
pub const DELETE_CHUNKS_FOR_FILE: &str = "DELETE FROM symbols WHERE file_path = ?1;";

/// Delete a single file's `files_metadata` row (deletion reconciliation, §5.2).
pub const DELETE_FILE_META: &str = "DELETE FROM files_metadata WHERE file_path = ?1;";

/// Enumerate every indexed file path (drives deletion reconciliation against disk, §5.2).
pub const ALL_INDEXED_FILES: &str = "SELECT file_path FROM files_metadata;";

/// Full-text search with BM25 ranking (§6.1). Column weights bias `symbol_name` (and to a lesser
/// degree the other indexed columns) above `chunk_text`, so a name match outranks a body-only
/// match (test `column_weighting_respected`). FTS5 `bm25()` is lower-is-better, so ascending
/// `ORDER BY` yields best-first; `rowid` is the deterministic tie-break.
///
/// Weight order matches the indexed-column order in `schema::CREATE_SYMBOLS`:
/// symbol_name, symbol_type, chunk_text, parent_symbol, imports, cross_references, file_docstring.
/// One explicit weight per indexed column (7) — `bm25()` only accepts weights for indexed columns
/// (UNINDEXED columns never contribute and are not counted), so the arity here is exactly the
/// indexed-column count. `file_docstring` is weighted 2.0 (same enrichment tier as imports /
/// cross_references), well below `symbol_name` (10.0) and `parent_symbol` (5.0).
pub const SEARCH: &str = "\
SELECT
    symbol_name, symbol_type, chunk_text, parent_symbol, imports, cross_references, file_docstring,
    file_path, start_byte, end_byte, start_line, end_line, language,
    bm25(symbols, 10.0, 1.0, 1.0, 5.0, 2.0, 2.0, 2.0) AS score
FROM symbols
WHERE symbols MATCH ?1
ORDER BY score ASC, rowid ASC
LIMIT ?2;";

/// Path-scoped symbol skeleton for the `codecache_outline` tool (Decision Log **D19**). A plain
/// column `SELECT` over the contentful `symbols` FTS5 table reading the UNINDEXED line columns
/// (zero source reads — D7). Returns the symbols of an EXACT file (`file_path = ?1`) OR every
/// symbol under a directory prefix (`file_path LIKE ?2`, where `?2 = "<dir>/%"`). `?2`'s path
/// portion has its SQL `LIKE` wildcards (`%`, `_`) and the escape char (`\`) escaped via
/// `ESCAPE '\'`, so a sibling file that merely shares the prefix string never over-matches.
/// Ordered deterministically by `(file_path, start_line, end_line)` ascending.
pub const SYMBOLS_FOR_PATH: &str = "\
SELECT symbol_name, symbol_type, parent_symbol, file_path, start_line, end_line
FROM symbols
WHERE file_path = ?1 OR file_path LIKE ?2 ESCAPE '\\'
ORDER BY file_path, start_line, end_line;";

/// Read a single file's stored content hash.
pub const GET_FILE_HASH: &str = "SELECT content_hash FROM files_metadata WHERE file_path = ?1;";

/// Read a single file's full metadata row.
pub const GET_FILE_META: &str = "\
SELECT content_hash, mtime, file_size, language, chunk_count
FROM files_metadata WHERE file_path = ?1;";

/// Upsert a `files_metadata` row (D6). `indexed_at` is set to now on every write.
pub const UPSERT_FILE_META: &str = "\
INSERT INTO files_metadata (file_path, content_hash, mtime, file_size, language, chunk_count, indexed_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%s','now'))
ON CONFLICT(file_path) DO UPDATE SET
    content_hash = excluded.content_hash,
    mtime        = excluded.mtime,
    file_size    = excluded.file_size,
    language     = excluded.language,
    chunk_count  = excluded.chunk_count,
    indexed_at   = excluded.indexed_at;";

/// Read one `index_state` value by key.
pub const GET_INDEX_STATE: &str = "SELECT value FROM index_state WHERE key = ?1;";

/// Upsert one `index_state` key/value pair.
pub const SET_INDEX_STATE: &str = "\
INSERT INTO index_state (key, value) VALUES (?1, ?2)
ON CONFLICT(key) DO UPDATE SET value = excluded.value;";
