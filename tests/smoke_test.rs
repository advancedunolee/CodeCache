//! M0 scaffolding smoke test (slice M0.1).
//!
//! Asserts the crate links and the only valued public symbol introduced at M0,
//! `codecache::VERSION`, equals the Cargo package version. This is the RED test that gates
//! the GREEN scaffolding: it cannot compile until `Cargo.toml` + `src/lib.rs` (with
//! `pub const VERSION`) exist. See docs/plans/M0-scaffolding.md and TEST_STRATEGY.md
//! ("Definition of good test coverage for a slice").

#[test]
fn crate_links_and_lib_is_callable() {
    // The library crate must link, and its advertised VERSION must match Cargo's package
    // version (single source of truth). No production logic is exercised at M0.
    assert_eq!(codecache::VERSION, env!("CARGO_PKG_VERSION"));
}
