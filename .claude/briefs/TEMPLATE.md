# BRIEF — <milestone> / <slice>

- **Milestone:** <e.g. M1 — config + storage>  ·  **Module(s):** <e.g. storage>
- **Owner (manager):** principal-engineering-manager  ·  **Created:** <YYYY-MM-DD>
- **Status:** RED ▢  GREEN ▢  REVIEW ▢  DONE ▢
- **Links:** docs/ROADMAP.md#<milestone> · docs/TEST_STRATEGY.md#<module>

## Goal
<one or two sentences: the behavior this slice delivers>

## Scope (in / out)
- In: <…>
- Out: <…>  (defer to: <milestone/decision>)

## Scenarios to cover (from TEST_STRATEGY)
- [ ] happy path: <…>
- [ ] edge: <…>
- [ ] error: <…>

## Definition of Done
- [ ] Tests written first, now green · clippy -D warnings clean · fmt clean
- [ ] API matches project_plan §3.2 · in-scope Decision Log behaviors honored
- [ ] Perf budget respected (if applicable) · reviewer APPROVED
- [ ] docs/TODO.md + module CLAUDE.md updated

---
## RED — test lead
<tests added; failing output; anything the impl must satisfy>

## GREEN — engineering lead
<what was implemented; how tests pass; any plan deviation raised>

## Specialist / Perf notes
<tree-sitter/FTS5 edge cases; bench numbers vs budget>

## REVIEW — code reviewer
<APPROVE / BLOCK + findings: severity — file:line — problem — fix>

## OUTCOME — manager
<aligned? TODO updated? slice marked done? follow-ups created?>
