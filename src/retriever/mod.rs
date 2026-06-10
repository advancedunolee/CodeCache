//! Retriever: FTS5 BM25 search, snippet extraction, token counting, greedy token-budget packing.
//!
//! API anchor: `project_plan.md` §3.2.3 / §6. Kept behind a trait so a `HybridRetriever` can
//! wrap it in v0.2 (Decision Log D1). Owner: `principal-engineering-lead`. Scenarios:
//! `docs/TEST_STRATEGY.md#retriever`. M0: empty stub; implemented at M6.

#[cfg(test)]
mod tests {}
