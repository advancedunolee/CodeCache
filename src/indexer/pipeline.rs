//! Per-file indexing pipeline (slice M5.2).
//!
//! API anchor: `project_plan.md` §5.1 (step 3a–3e). Owner: `principal-engineering-lead`.
//! Scenarios: `docs/TEST_STRATEGY.md#indexer` (full-index rows + D2).
//!
//! [`index_file`] performs the per-file work of a full index for one discovered source file:
//! compute hash → read content → detect language → parse → chunk → `insert_chunks` → write the
//! `files_metadata` row. It returns the number of chunks inserted so the caller can accumulate
//! [`IndexStats`](super::IndexStats). Each fallible step surfaces a typed [`IndexError`] via `?`;
//! the caller (`index_all`) wraps the whole call so a single file's error is counted/skipped and
//! the batch continues (D2 degrade-and-continue) — there is no reachable `unwrap`/`expect`/`panic`.

use std::path::{Path, PathBuf};

use crate::chunker;
use crate::hasher;
use crate::parser::Parser;
use crate::storage::Storage;
use crate::types::{FileMeta, Language};

use super::{detect_language, IndexError};

/// Index one source file: hash, read, parse, chunk, store its chunks, and upsert its
/// `files_metadata` row (`project_plan.md` §5.1 step 3a–3e). Returns the count of chunks inserted.
///
/// The chunker handles a malformed tree gracefully (heuristic fallback or empty), so a syntactically
/// broken file does not error here; it is the unreadable/unsupported-language/storage failures that
/// surface an [`IndexError`] for the caller to isolate (D2).
///
/// # Errors
/// Returns [`IndexError::Hash`] if hashing fails, [`IndexError::File`] if the content/metadata
/// cannot be read, [`IndexError::Parser`]/[`IndexError::Chunker`] on parse/chunk failure, and
/// [`IndexError::Storage`] if the chunk insert or metadata upsert fails.
pub fn index_file(
    parser: &mut Parser,
    storage: &Storage,
    path: &Path,
) -> Result<usize, IndexError> {
    // §5.1 step 3a: content+mtime hash (the value stored in files_metadata.content_hash).
    let content_hash = hasher::compute_file_hash(path).map_err(IndexError::Hash)?;

    // §5.1 step 3b: read source + filesystem metadata (size, mtime) in one place.
    let content = std::fs::read_to_string(path).map_err(|source| IndexError::File {
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = std::fs::metadata(path).map_err(|source| IndexError::File {
        path: path.to_path_buf(),
        source,
    })?;
    let file_size = metadata.len();
    let mtime = file_mtime_secs(&metadata);

    // Language is known from discovery (only configured-language files are returned); recompute it
    // defensively so the pipeline is self-contained and the FileMeta language is correct.
    let language = detect_language(path).unwrap_or(Language::Python);

    // §5.1 step 3b–3c: parse → chunk. The chunker degrades a malformed tree internally (D2).
    let tree = parser
        .parse_file(path, &content, language)
        .map_err(IndexError::Parser)?;
    let mut chunks = chunker::chunk(&tree, &content, language).map_err(IndexError::Chunker)?;

    // The parser/chunker leave file_path empty (they are file-agnostic); stamp it so stored chunks
    // and the files_metadata row share the same key the tests query by (absolute-under-root path).
    for chunk in &mut chunks {
        chunk.file_path = path.to_path_buf();
    }

    let chunk_count = chunks.len();

    // §5.1 step 3d: store chunks (single transaction inside insert_chunks).
    storage
        .insert_chunks(&chunks)
        .map_err(IndexError::Storage)?;

    // §5.1 step 3e: upsert the file's metadata row (D6 bundle).
    let meta = FileMeta {
        content_hash,
        mtime,
        file_size,
        language,
        chunk_count,
    };
    storage
        .update_file_hash(path, &meta)
        .map_err(IndexError::Storage)?;

    Ok(chunk_count)
}

/// Re-index a file whose content changed: delete its existing chunks first (so the previous
/// symbols do not linger or duplicate), then run the normal per-file [`index_file`] path. The
/// `files_metadata` row is upserted by `index_file`, so the stored hash/mtime/chunk_count are
/// replaced. Returns the count of chunks inserted (§5.2).
///
/// # Errors
/// Propagates [`IndexError::Storage`] if the delete fails, plus any [`index_file`] error.
pub fn reindex_file(
    parser: &mut Parser,
    storage: &Storage,
    path: &Path,
) -> Result<usize, IndexError> {
    // Delete-first avoids duplicate/stale chunks for the file across re-indexes.
    storage
        .delete_chunks_for_file(path)
        .map_err(IndexError::Storage)?;
    index_file(parser, storage, path)
}

/// Of the candidate `files`, return those whose on-disk content hash differs from the stored hash
/// (`files_metadata.content_hash`) — i.e. files that are new (no stored hash) or changed (§5.2).
/// Unchanged files are skipped, which is what makes a re-index of an untouched repo a no-op.
///
/// A file whose hash cannot be computed (e.g. it vanished between discovery and here) is treated as
/// *changed* so the caller's per-file path can attempt it and isolate any failure (D2), rather than
/// being silently dropped from change detection.
///
/// # Errors
/// Propagates [`IndexError::Storage`] only if reading a stored hash fails (not isolatable per-file).
pub fn detect_changed_files(
    storage: &Storage,
    files: &[PathBuf],
) -> Result<Vec<PathBuf>, IndexError> {
    let mut changed = Vec::new();
    for path in files {
        let stored = storage.get_file_hash(path).map_err(IndexError::Storage)?;
        match hasher::compute_file_hash(path) {
            Ok(current) if stored.as_deref() == Some(current.as_str()) => {
                // Unchanged: stored hash equals the freshly-computed content+mtime hash → skip.
            }
            _ => changed.push(path.clone()),
        }
    }
    Ok(changed)
}

/// Modification time of `metadata` as Unix epoch seconds, or `0` when it is unavailable or predates
/// the epoch. The hash already encodes mtime authoritatively; the stored `FileMeta.mtime` is
/// bookkeeping, so a defensive `0` here is preferable to failing the whole file on a clock quirk.
fn file_mtime_secs(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lock the load-bearing no-write guarantee at the unit level: after indexing a file, a second
    /// `detect_changed_files` over the same untouched file reports nothing changed — so the skip
    /// path (no delete/insert) is taken on a re-index of an unchanged repo (idempotency).
    #[test]
    fn detect_changed_files_empty_for_unchanged_repo() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        let file = root.join("solo.py");
        std::fs::write(&file, "def solo_fn():\n    return 1\n").expect("write fixture");

        let storage = Storage::new(&root.join("index.db")).expect("open storage");
        storage.init_schema().expect("init schema");
        let mut parser = Parser::new().expect("build parser");

        // Index once: stores the content+mtime hash that detect_changed_files will compare against.
        index_file(&mut parser, &storage, &file).expect("index_file");

        let changed =
            detect_changed_files(&storage, &[file.clone()]).expect("detect_changed_files");
        assert!(
            changed.is_empty(),
            "an unchanged file must not be reported as changed (no-write skip path), got {changed:?}"
        );
    }
}
