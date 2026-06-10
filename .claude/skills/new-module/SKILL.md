---
name: new-module
description: >
  Scaffold a new CodeCache Rust module with its test entry and a CLAUDE.md, wired to the module
  responsibility table in docs/project_plan.md (§3.1) and the build order in
  docs/ENGINEERING_PLAN.md. Use when starting a module that doesn't exist yet. Invoke as
  /new-module <module-name>.
---

# New Module Scaffold — CodeCache

Create a cohesive module that matches the documented architecture. Always tests-first.

## Steps
1. **Confirm the module** is in the responsibility table (`docs/project_plan.md` §3.1):
   one of `cli, indexer, parser, chunker, hasher, storage, retriever, formatter, mcp_server,
   config`. Note its responsibility, dependencies, and target LOC.
2. **Verify build order** (`docs/ENGINEERING_PLAN.md`): its dependencies must already exist.
3. **Create the layout** (match `docs/project_plan.md` §10.4):
   - `src/<module>/mod.rs` with the public API skeleton from §3.2 (signatures only / `todo!()`
     bodies so tests compile and **fail**, not stubs that fake success).
   - `src/<module>/CLAUDE.md` from the standard skeleton (purpose, owner agent, key files,
     conventions, the TDD rule, commands, update rule).
   - Register the module in `src/lib.rs` / `src/main.rs`.
4. **Seed tests** (hand to `principal-test-engineering-lead`): a `tests/<module>_tests.rs`
   (integration) and/or `#[cfg(test)] mod tests` (unit) covering the §`docs/TEST_STRATEGY.md`
   scenarios for this module — all failing initially (RED).
5. **Update tracking**: add the module's checklist items to `docs/TODO.md`; the manager
   confirms placement in the roadmap.

## Rules
- Skeletons must make tests fail honestly (`todo!()` / `unimplemented!()`), never return fake
  passing values.
- Keep the public API aligned with §3.2; deviations go through the manager → update the plan first.
- A module is not "created" until it has both a `CLAUDE.md` and failing tests.
