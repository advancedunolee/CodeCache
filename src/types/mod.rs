//! Shared, dependency-free core types.
//!
//! Home of `Chunk`, `Language`, `SymbolType`, and `FileMeta` (Decision Log **D5**) so that both
//! `storage` and `parser`/`chunker` can depend on them without `storage` depending on `parser`,
//! keeping the bottom-up build order acyclic. See `project_plan.md` §4.3 / §3.2.2.
//!
//! M0: empty stub. Types land at M1 (`storage` needs them).

#[cfg(test)]
mod tests {}
