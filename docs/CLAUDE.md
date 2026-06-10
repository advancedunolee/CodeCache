# docs/ — CLAUDE.md

**Purpose:** all project documentation. **Owner agent:** `principal-engineering-manager`.

## Doc map
| File | Role |
|---|---|
| `project_plan.md` | Product + architecture spec (the *what/why*). Images in `assets/`. |
| `ENGINEERING_PLAN.md` | Team, module ownership, build order, TDD workflow, Definition of Done, quality gates. |
| `ROADMAP.md` | M0–M10 milestones with entry/exit criteria + the **Decision Log**. |
| `TEST_STRATEGY.md` | Per-module test scenario matrix (test-lead's worklist). |
| `TODO.md` | Living, manager-owned checklist mirroring the roadmap. |
| `plans/` | Per-milestone phase plans (M0–M10): slices, API contracts, budgets, DoD. Index in `plans/README.md`. |
| `assets/` | Extracted PNG diagrams referenced by `project_plan.md`. |

## Update rule (contract)
- Any code change ⇒ update `TODO.md` and the relevant module `CLAUDE.md` in the **same** change.
- Any API/architecture change ⇒ update `project_plan.md` **first**, then implement.
- New design decision/critique ⇒ record in the `ROADMAP.md` Decision Log.
- Keep `project_plan.md` lean: never re-embed base64 images — put binaries in `assets/` and reference them.

## Conventions
- Markdown, relative links between docs. Tables for matrices. Keep each doc scannable.
