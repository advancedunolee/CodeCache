# src/types/ — CLAUDE.md

**Module:** `types` · **Owner:** `principal-engineering-lead` · **Milestone:** M1 (stub at M0).

## Purpose
Shared, dependency-free core types: `Chunk`, `Language`, `SymbolType`, `FileMeta`. Lives here
(not in `parser`) per **Decision Log D5** so `storage` need not depend on `parser` and the
bottom-up build order stays acyclic.

## API anchor
`docs/project_plan.md` §4.3 (`Chunk`/`SymbolType`/`Language`) and §3.2.2 (`FileMeta`). `Chunk`
carries `start_line`/`end_line` (D7) and the enrichment fields `parent_symbol`/`file_docstring`/
`imports`/`cross_references` (D3).

## Tests / scenarios
Unit tests in-module. No dedicated TEST_STRATEGY row — these types are exercised through
`storage`, `parser`, and `chunker` scenarios.

## Shipped API (M1)
- `Chunk` (Debug/Clone/PartialEq/Eq) — §4.3 fields incl. `start_line`/`end_line` (D7, 1-based
  inclusive) + D3 enrichment (`parent_symbol`/`file_docstring`/`imports`/`cross_references`).
- `SymbolType` { Function, Class, Method, Struct } and `Language` { Python, TypeScript, Go } —
  both Copy/PartialEq/Eq/Hash with `as_str()` + `from_str_lenient(&str) -> Option<Self>` (total,
  reversible, `None` on unknown → no panic) for text persistence in §4.1. `Language` also derives
  serde (`rename_all = "lowercase"`) so it parses from `config.toml` `languages = [...]`.
- `FileMeta` (D6) { content_hash, mtime, file_size, language, chunk_count }.
- `SymbolOutline` (**D19**, M8.3) { symbol_name, symbol_type, parent_symbol, file_path, start_line,
  end_line } (Debug/Clone/PartialEq/Eq) — the slim per-symbol projection backing
  `Storage::symbols_for_path` / the `codecache_outline` MCP tool. Carries only the skeleton fields
  (no `chunk_text`/enrichment) so the outline stays within the §11.2 budget. Dependency-free (D5).

## Status
**M1: DONE (2026-06-10).** All four gates green on Rust 1.85.0. Dependency-free, as required by
D5. First consumer is `storage`.
