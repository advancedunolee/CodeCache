# .github/ — CLAUDE.md

CI/CD workflows. **Owner agent:** `devops-release-engineer`. CI must mirror the local quality
gates exactly (`../docs/ENGINEERING_PLAN.md` §5).

## Layout
| Path | Role | Milestone |
|---|---|---|
| `workflows/ci.yml` | fmt-check → clippy `-D warnings` → `cargo test --all`, with cargo caching. | M0 |
| `workflows/release.yml` | version bump, `v0.1.0` tag, crates.io publish, install smoke test. | M10 |
| `workflows/bench.yml` | scheduled criterion runs vs budgets. | M10 |

## CI parity contract (ENGINEERING_PLAN §5)
The `ci.yml` steps must use the **same flags** as the local hooks (`.claude/hooks/*.ps1`):
| Gate | Local hook | CI step |
|---|---|---|
| Format | `cargo fmt` on `.rs` edit | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` at Stop | same |
| Tests | `cargo test` at Stop | `cargo test --all` |

Toolchain is pinned by `../rust-toolchain.toml` (1.85.0) so local == CI; bump them in lockstep.
Caching is mandatory — `rusqlite` `bundled` + tree-sitter grammars compile C (slow cold build).

## Rules
- When local hooks change, update `ci.yml` in the **same** change to keep gates identical
  (`../.claude/CLAUDE.md` conventions).
- Keep the toolchain channel here, `rust-toolchain.toml`, and any hook references in sync.

## Status
M0: `ci.yml` present (single `gates` job, three steps). `release.yml`/`bench.yml` land at M10.
