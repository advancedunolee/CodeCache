//! CLI: argument parsing, command dispatch, user-facing errors.
//!
//! Public API anchor: `project_plan.md` §3.2 / §7 (commands: init/index/update/query/status/
//! config/serve). Owner: `principal-engineering-lead`. Tests live in `tests/`; scenarios in
//! `docs/TEST_STRATEGY.md#cli`.
//!
//! M0: stub `run()` that prints name + version so the binary links and is invocable. Real
//! `clap`-based dispatch lands at M7.

use anyhow::Result;

/// Entry point invoked by `main`. M0 stub: prints the package name and version.
pub fn run() -> Result<()> {
    println!("{} {}", env!("CARGO_PKG_NAME"), crate::VERSION);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_succeeds() {
        assert!(super::run().is_ok());
    }
}
