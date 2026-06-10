---
name: standup
description: >
  Manager status report for CodeCache: summarize current milestone, what's in progress vs
  next, quality-gate health (fmt/clippy/test/bench), open briefs, and blockers. Use for a
  quick "where are we" check or at the start/end of a work session. Invoke as /standup.
---

# Standup — CodeCache

Produce a concise status report. Read-only; gather facts, don't change anything.

## Gather
1. **Milestone & tasks** — read `docs/TODO.md`: current phase, `[~]` in-progress, next `[ ]`.
2. **Open briefs** — list `.claude/briefs/BRIEF-*.md` and their Status line (RED/GREEN/REVIEW/DONE).
3. **Gate health** — if `Cargo.toml` exists, run and report:
   - `cargo fmt --check`  (formatted?)
   - `cargo clippy --all-targets -- -D warnings`  (lint clean?)
   - `cargo test --quiet`  (green?)
   Otherwise note "pre-scaffolding (M0 not done)".
4. **Roadmap context** — the current milestone's exit criteria from `docs/ROADMAP.md`.

## Report format
```
CodeCache standup — <date>
Milestone:   <Mx — name>   (<n>/<total> phase items done)
In progress: <items / owners>
Next up:     <top 1–3 items>
Open briefs: <BRIEF-… : status>
Gates:       fmt <ok/✗>  clippy <ok/✗>  tests <ok/✗ n passed>
Blockers:    <none | …>
Exit criteria remaining: <…>
```

Keep it to ~10 lines. Flag anything red and recommend the next concrete action (usually:
hand the next slice to the test lead via a new brief).
