//! CodeCache binary entry point.
//!
//! M0 stub: delegates to [`codecache::cli`], which currently only prints help/version. Real
//! command dispatch lands at M7. The entry point returns `anyhow::Result<()>` so errors surface
//! with a non-zero exit code and no reachable `unwrap`/`expect`/`panic!`.

use anyhow::Result;

fn main() -> Result<()> {
    codecache::cli::run()
}
