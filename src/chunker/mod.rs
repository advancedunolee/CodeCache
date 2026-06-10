//! Chunker: turn AST nodes into enriched `Chunk`s (`parent_symbol`, `file_docstring`,
//! `imports`, `cross_references` — Decision Log D3); non-overlapping within file bounds.
//!
//! API anchor: `project_plan.md` §4.3. Owner: `principal-engineering-lead` +
//! `rust-treesitter-specialist`. Scenarios: `docs/TEST_STRATEGY.md#chunker`. M0: empty stub;
//! implemented at M4.

#[cfg(test)]
mod tests {}
