# M2 — hasher (xxHash3-128)

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m2--hasher),
> [`../project_plan.md`](../project_plan.md) §4.4 / §5.4, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#hasher).

## Goal / acceptance criteria
Compute a stable xxHash3-128 over file content (+mtime) and detect change vs the cached hash.
**Exit (from ROADMAP):**
- [ ] Deterministic hash for identical content; differs on a 1-byte change.
- [ ] Change detection: unchanged ⇒ "same"; modified ⇒ "changed".
- [ ] Large files and binary files handled without panic.

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/hasher/mod.rs` | `compute_file_hash`, `compute_content_hash`, change-detection helper. | eng-lead |
| `tests/hasher_tests.rs` | Integration: determinism, sensitivity, binary/large, change detection. | test-lead |
| `src/hasher/CLAUDE.md` | Updated with shipped API + hash format note. | manager |

## Dependencies
- **Prior:** M0 (`xxhash-rust` with `xxh3`). M1 only for the *integration* check that a hash
  round-trips through `files_metadata` — keep that as a storage-side test, not a hasher dep.
- `hasher` itself depends on nothing in-tree (leaf module).

## Ordered slices

### Slice M2.1 — content hash (pure, deterministic)
- **RED (test-lead):**
  - `same_bytes_expects_same_hash`
  - `one_byte_change_expects_different_hash`
  - `hash_is_32_hex_chars` (128-bit ⇒ `{:032x}`, §4.4)
  - `empty_content_expects_stable_hash`
  - `binary_content_with_nulls_expects_no_panic_and_stable_hash`
- **GREEN (eng-lead):** `compute_content_hash(bytes: &[u8]) -> String` using `Xxh3::digest128`,
  formatted `{:032x}`. Pure — no fs — so it's unit-testable and fast.

### Slice M2.2 — file hash (content + mtime) and change detection
- **RED:**
  - `file_hash_matches_content_hash_of_its_bytes_plus_mtime`
  - `touching_mtime_without_content_change_changes_hash` (§4.4 hashes mtime too)
  - `missing_file_expects_typed_error_not_panic`
  - `unchanged_vs_cached_expects_same` / `modified_vs_cached_expects_changed`
  - `large_file_1mb_hashes_within_budget` (sanity, not a CI-strict timing assert)
- **GREEN:** `compute_file_hash(path) -> Result<String>` (read bytes + mtime, hash both per
  §4.4); `is_changed(path, cached: Option<&str>) -> Result<bool>` for the incremental path M5
  will call. Read via streaming or full-read; full-read is acceptable for v0.1 file sizes.
- **REVIEW:** no `unwrap`; error on unreadable/missing; mtime acquisition handles platforms.

## API contracts / data structures (from `../project_plan.md` §4.4)
```rust
pub fn compute_content_hash(bytes: &[u8]) -> String;            // {:032x}
pub fn compute_file_hash(path: &Path) -> Result<String>;        // content + mtime (§4.4)
pub fn is_changed(path: &Path, cached: Option<&str>) -> Result<bool>;
```
Hash string is the **same 32-hex format** stored in `files_metadata.content_hash` (M1 §4.1).

## Performance budgets (from `../project_plan.md` §5.4 / §11.4)
- **Hash computation (1K files, ~500 LOC each): < 500ms** (§5.4). Validated rigorously by the
  M10 criterion bench, not by a flaky timing assert in unit tests.
- xxHash3 throughput target ~10GB/s (§11.4) — informs M5's "incremental < 2s" budget. Avoid
  re-reading files or double-hashing in the M5 caller.

## Decision Log bindings
- **D2 (graceful degradation):** hasher must never panic on binary/unreadable content — it
  hashes bytes opaquely; language/parse concerns are downstream (M3/M4).

## Definition of Done (this phase)
- [ ] M2.1–M2.2 slices green; determinism + 1-byte sensitivity asserted with real hex values.
- [ ] Binary + large-file paths exercised; missing file → typed error.
- [ ] Hash format identical to `files_metadata.content_hash` (M1).
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 2 + `src/hasher/CLAUDE.md` updated.
- [ ] Perf bench stub deferred to M10 but the < 500ms target noted in `src/hasher/CLAUDE.md`.
