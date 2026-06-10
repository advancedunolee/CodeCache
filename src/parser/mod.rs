//! Parser: Tree-sitter integration — load grammars, run `.scm` queries, extract AST nodes with
//! exact byte spans; ERROR-node detection (graceful degradation, Decision Log D2).
//!
//! API anchor: `project_plan.md` §3.2.1 / §5.3. Owner: `principal-engineering-lead` +
//! `rust-treesitter-specialist`. Scenarios: `docs/TEST_STRATEGY.md#parser-python--ts--go`.
//! M0: empty stub; Python lands at M3, TS/Go at M9.

#[cfg(test)]
mod tests {}
