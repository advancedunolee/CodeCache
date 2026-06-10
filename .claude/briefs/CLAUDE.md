# .claude/briefs/ — durable hand-off briefs

**Purpose:** carry state across the manager → test-lead → engineering-lead → reviewer
hand-off so coordination survives beyond a single conversation/subagent. **Owner:**
`principal-engineering-manager` (creates); every agent reads and appends its section.

## Why briefs exist (harness engineering)
Subagents start cold and don't share conversation memory. A brief is the **shared blackboard**
for one slice: the manager writes the goal + scenarios up front, each agent appends what it
did, and the next agent picks up from disk instead of re-deriving context. This makes
multi-agent hand-offs deterministic and auditable.

## Protocol
1. **Manager** creates `BRIEF-<milestone>-<slice>.md` from `TEMPLATE.md` when starting a slice
   (e.g. `BRIEF-M1-storage-schema.md`). Fills Goal, Scope, Scenarios, Definition of Done.
2. **Test lead** appends the RED section: tests written + the failing output.
3. **Engineering lead** appends the GREEN section: what was implemented + how it passes.
4. **Specialist/Perf** append notes if engaged.
5. **Reviewer** appends the verdict (APPROVE/BLOCK + findings).
6. **Manager** appends Outcome, updates `docs/TODO.md`, marks the slice done.

## Conventions
- One brief per slice. Keep it short — it's a coordination record, not a design doc.
- Briefs are **tracked in git** (they document how the project was built). Stale/superseded
  briefs may be moved to `archive/` by the manager.
- Cross-link the relevant `docs/ROADMAP.md` milestone and `docs/TEST_STRATEGY.md` rows.
