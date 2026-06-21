# BRIEF — discovery / default-ignore patterns

- **Milestone:** post-M10 hardening (touches M5 `indexer` + M1 `config`)  ·  **Module(s):** `config`, `indexer` (`discovery.rs`)
- **Owner (manager):** principal-engineering-manager  ·  **Created:** 2026-06-20
- **Status:** RED ▣  GREEN ▣  REVIEW ▣  DONE ▣
- **Links:** docs/ROADMAP.md#d32 · docs/project_plan.md §7.3 / §5.1 · docs/TEST_STRATEGY.md#indexer / #config

## Goal
Make file discovery exclude common dependency/virtualenv/build directories **by default**, so a
target repo with **no `.gitignore`** is not swamped by vendored noise (the verified `env/`
virtualenv case: 716 files / 12 356 chunks, 100 % venv, real source never surfaced). The defaults
are a separate, disable-able knob — not a silent merge into `ignore_patterns`.

## Design (ratified — Decision Log D32; do NOT diverge without re-opening the plan)
- New `Config` field **`use_default_ignores: bool`**, `#[serde(default = default_use_default_ignores)]`
  → **`true`**. Added to `Config` (+ its `Default` impl). Mirrors §7.3.
- Module-level const **`DEFAULT_IGNORE_PATTERNS: &[&str]`** in the `indexer` module (the only
  consumer; place near `discovery.rs`'s `build_ignore_patterns`). Exact set (gitignore-style globs):
  `env/`, `.venv/`, `venv/`, `node_modules/`, `__pycache__/`, `*.pyc`, `target/`, `dist/`, `build/`,
  `.git/`.
- `discovery.rs::build_ignore_patterns(config, root)`: when `config.use_default_ignores`, add each
  `DEFAULT_IGNORE_PATTERNS` entry to the `GitignoreBuilder` **first**, THEN the user's
  `config.ignore_patterns` (so user patterns extend, never replace). When `false`, skip the defaults
  entirely (only user patterns feed the matcher; `.gitignore` is still honored by `WalkBuilder`).
- No reachable `unwrap()/expect()/panic!`. A bad default glob would surface as `IndexError::Glob`
  exactly like a bad user pattern — but the defaults are static and tested, so this never fires in
  practice. `Cargo.toml` untouched (the `ignore` crate is already a dep).

## Scope (in / out)
- In: the `Config` field + default; the `DEFAULT_IGNORE_PATTERNS` const; the `build_ignore_patterns`
  fold; tests at the discovery integration seam + a config default unit assertion.
- Out: a `--no-default-ignore` CLI flag (config knob suffices; would be undriven CLI surface — defer
  to v0.1.x if requested). Out: re-indexing the verified external repo (that was the bug report, not
  a test fixture). Out: any change to `WalkBuilder`'s hidden-skip behavior (`.git/` already excluded).

## Scenarios to cover (from TEST_STRATEGY #indexer / #config)
- [ ] happy path (the bug): a temp repo with a fake `env/` virtualenv tree (`env/lib/site-packages/pip/_x.py`)
      + real source (`backbone.py`) + **no `.gitignore`**, default config ⇒ `discover_files` returns
      ONLY the real source, the `env/` files excluded. (This is the regression test for D32.)
- [ ] edge (other defaults): `node_modules/x.ts`, `__pycache__/x.py`, `build/x.py`, `dist/x.py`,
      `target/x.py`, a stray `x.pyc` are all excluded by default; a sibling real source survives.
- [ ] opt-out: same `env/` repo with `use_default_ignores = false` ⇒ the `env/` source files ARE
      returned (defaults re-included; proves the knob disables them).
- [ ] extension (not replacement): `use_default_ignores = true` (default) + a user
      `ignore_patterns = ["*_generated.py"]` ⇒ BOTH the venv files AND `*_generated.py` are excluded,
      and a plain real file survives (user patterns extend the defaults).
- [ ] config default: `Config::default().use_default_ignores == true`; a TOML omitting the key loads
      as `true`; `use_default_ignores = false` in TOML round-trips. (config unit / load test.)

## Definition of Done
- [ ] Tests written first, now green · clippy -D warnings clean · fmt clean
- [ ] API matches project_plan §7.3 (new field) + §5.1 (discovery note) · D32 honored
- [ ] No perf budget regression (discovery only adds ~10 static globs to one matcher build — negligible)
- [ ] reviewer APPROVED
- [ ] docs/TODO.md + src/indexer/CLAUDE.md + src/config/CLAUDE.md updated in the same change

---
## RED — test lead (2026-06-20)
**Status:** RED confirmed — fails to compile for the right reason (missing `Config.use_default_ignores`).

**Files touched (tests only — no production code):**
- `tests/indexer_tests.rs` — new section `═══ Default ignore patterns (D32) ═══` (4 integration tests).
- `src/config/mod.rs` — `#[cfg(test)] mod tests` only: extended `default_config_matches_documented_defaults` + 2 new focused TOML-load tests.

**Tests added:**
- `tests/indexer_tests.rs`
  - `discovery_excludes_fake_virtualenv_by_default_with_no_gitignore` — the bug regression: `env/lib/site-packages/pip/_internal.py` + `backbone.py`, NO `.gitignore`, default config ⇒ only `backbone.py`.
  - `discovery_excludes_all_default_ignored_dirs_and_pyc_by_default` — `node_modules/`, `__pycache__/`, `build/`, `dist/`, `target/` (with .ts/.py/.go content so the DIRECTORY default, not the language filter, excludes) + stray `stale.pyc` ⇒ only `keep.py`.
  - `discovery_includes_virtualenv_when_default_ignores_disabled` — opt-out: `Config { use_default_ignores: false, ..config_with_languages([Python]) }` ⇒ `env/...py` IS returned alongside `backbone.py`.
  - `discovery_user_patterns_extend_default_ignores` — extension-not-replacement: default + `ignore_patterns=["*_generated.py"]` over `env/lib/x.py` + `schema_generated.py` + `real.py` ⇒ only `real.py` (BOTH defaults AND user pattern apply).
- `src/config/mod.rs::tests`
  - `default_config_matches_documented_defaults` (extended) — adds `assert!(cfg.use_default_ignores)`.
  - `toml_omitting_use_default_ignores_loads_true` — `toml::from_str("version = \"0.1.0\"\n")` ⇒ `use_default_ignores == true`.
  - `toml_use_default_ignores_false_loads_false` — `toml::from_str("use_default_ignores = false\n")` ⇒ `false`.

**Captured RED (`cargo test --no-run`):**
```
error[E0609]: no field `use_default_ignores` on type `config::Config`
   --> src/config/mod.rs:265:17
error[E0609]: no field `use_default_ignores` on type `config::Config`
   --> src/config/mod.rs:277:17
error[E0609]: no field `use_default_ignores` on type `config::Config`
   --> src/config/mod.rs:288:18
error: could not compile `codecache-rs` (lib test) due to 3 previous errors
```
(The integration tests in `indexer_tests.rs` also reference the missing field via `Config { use_default_ignores: false, .. }`; the build gates on the lib-test error first. Once the field exists, the behavioral assertions become the next RED until `build_ignore_patterns` folds in `DEFAULT_IGNORE_PATTERNS`.)

**What the impl must satisfy (pins for engineering lead):**
- Add `pub use_default_ignores: bool` to `Config` with `#[serde(default = "…")]` ⇒ `true`, and set it in the `Default` impl.
- `discovery.rs::build_ignore_patterns`: when `config.use_default_ignores`, add each of `DEFAULT_IGNORE_PATTERNS` (`env/ .venv/ venv/ node_modules/ __pycache__/ *.pyc target/ dist/ build/ .git/`) to the `GitignoreBuilder` FIRST, then the user `config.ignore_patterns` (extend, never replace). When `false`, skip the defaults entirely.
- `discover_files(config, root)` signature unchanged.

## GREEN — engineering lead (2026-06-20)
**Status:** GREEN — all four gates clean; `cargo test --all` **238 passed / 0 failed** (231 prior
+ 7 new; the +7 = 4 `indexer_tests` D32 + 3 `config::tests` use_default_ignores). Exactly the
ratified design — **no deviation**.

**Files changed (production only — no test touched):**
- `src/config/mod.rs` — (1) new field `pub use_default_ignores: bool` on `Config` with
  `#[serde(default = "default_use_default_ignores")]`, placed right after `ignore_patterns` with a
  §7.3-mirroring doc comment; (2) `fn default_use_default_ignores() -> bool { true }` alongside the
  other `default_*` fns; (3) `use_default_ignores: default_use_default_ignores()` in
  `impl Default for Config`.
- `src/indexer/discovery.rs` — (1) module-level `pub(crate) const DEFAULT_IGNORE_PATTERNS: &[&str]`
  with the exact 10-glob set (`env/ .venv/ venv/ node_modules/ __pycache__/ *.pyc target/ dist/
  build/ .git/`); (2) `build_ignore_patterns` now, when `config.use_default_ignores`, loops the const
  and `builder.add_line(None, pattern)` for each FIRST (mapping a bad glob to
  `IndexError::Glob { pattern: pattern.to_string(), source }`, same shape as the user loop — `pattern`
  here is `&&str`, deref-coerced to `&str` by `add_line` and `.to_string()`), THEN the unchanged user
  `config.ignore_patterns` loop and `builder.build()`; when `false` the defaults block is skipped.
  Doc comments on the file header, `discover_files`, and `build_ignore_patterns` updated to mention the
  default-ignore fold + extend-not-replace semantics. `detect_language`/`discover_files` signatures
  unchanged.
- Docs (same change, per project contract): `src/config/CLAUDE.md`, `src/indexer/CLAUDE.md`,
  `docs/TODO.md` (post-M10 hardening entry).

**Const + field added (verbatim):**
- `Config.use_default_ignores: bool` (`#[serde(default = "default_use_default_ignores")]` ⇒ `true`).
- `DEFAULT_IGNORE_PATTERNS = &["env/", ".venv/", "venv/", "node_modules/", "__pycache__/", "*.pyc",
  "target/", "dist/", "build/", ".git/"]`.

**How each RED test now passes:**
- `config::tests::default_config_matches_documented_defaults` — `Default` sets `true` ⇒ `assert!`.
- `toml_omitting_use_default_ignores_loads_true` — `#[serde(default = …)]` ⇒ omitted key folds to `true`.
- `toml_use_default_ignores_false_loads_false` — the explicit `false` deserializes through verbatim.
- `discovery_excludes_fake_virtualenv_by_default_with_no_gitignore` — default config has the knob on,
  so the `env/` glob is in the matcher; `matched_path_or_any_parents` ignores `env/lib/.../_internal.py`;
  only `backbone.py` survives.
- `discovery_excludes_all_default_ignored_dirs_and_pyc_by_default` — `node_modules/ __pycache__/ build/
  dist/ target/` dir-globs + `*.pyc` all match ⇒ only `keep.py`.
- `discovery_includes_virtualenv_when_default_ignores_disabled` — `use_default_ignores: false` skips the
  defaults; no `.gitignore`/user patterns ⇒ `env/...py` returned alongside `backbone.py`.
- `discovery_user_patterns_extend_default_ignores` — defaults on (`env/` excluded) AND user
  `*_generated.py` appended after ⇒ both excluded; only `real.py`.

**No reachable panic:** the default-glob loop uses the same `.map_err(|source| IndexError::Glob{..})?`
pattern as the user loop; the static set is valid, so it never fires in practice.

**Note (environment, not code):** the first `cargo test --all` run hit a transient linker OOM
(`ld: final link failed: Cannot allocate memory`) building many test binaries in parallel on this
WSL2 box — not a code defect (clippy compiled the same code cleanly). Re-running with `-j 2
--test-threads=2` linked fine: **238 passed / 0 failed**. `cargo build` also green.

## Specialist / Perf notes
<ignore-crate edge cases if any; otherwise n/a>

## REVIEW — code reviewer
<APPROVE / BLOCK + findings: severity — file:line — problem — fix>

## OUTCOME — manager (2026-06-20)
**Aligned + DONE.** Plan-first honored (project_plan §7.3 schema + §5.1 discovery note + ROADMAP
Decision Log D32 all written before any code). RED→GREEN→REVIEW followed the TDD loop with one
agent per role; no test weakened. Reviewer APPROVED with 0 blockers (verdict below). I bundled the
reviewer's one non-blocking nit into this same change — the stale §5.1 pseudocode now reads
`discover_files(config, root)` with a default-ignore note (project_plan.md ~line 772), so no
follow-up is needed. Docs updated in-change: `docs/project_plan.md` (§7.3 + §5.1 + §3.2.4 note),
`docs/ROADMAP.md` (D32), `docs/TODO.md` (post-M10 hardening entry), `src/config/CLAUDE.md`,
`src/indexer/CLAUDE.md`. Final: **238 tests pass / 0 fail**, all four gates green (Rust 1.85).
No new follow-ups. (Optional v0.1.x: a `--no-default-ignore` CLI flag — explicitly out of scope
here per D32; only build it if a per-invocation override is requested.)

---

**Verdict (code-reviewer, 2026-06-20): APPROVE.**

Gates (this WSL2/Linux box, Rust 1.85): `cargo fmt --all -- --check` clean · `cargo clippy
--all-targets -- -D warnings` clean · `cargo test --all -j 2 -- --test-threads=2` **238 passed /
0 failed** (231 prior + 7 new = 4 indexer D32 + 3 config). All 7 new tests observed green by name.

Reviewed against all six criteria:
1. **Correctness** — `build_ignore_patterns` adds `DEFAULT_IGNORE_PATTERNS` FIRST, then user
   `config.ignore_patterns` (extend, never replace); the `if config.use_default_ignores` guard
   skips the defaults block when `false`. Anchored-matcher behavior is empirically pinned by the
   opt-out/regression test pair: the regression test would fail if `env/` did not match the nested
   `env/lib/site-packages/pip/_internal.py` via `matched_path_or_any_parents`, and the opt-out
   test would fail if it matched for any reason other than the default. `*.pyc` file-glob and the
   five dir-globs all verified by the all-dirs test. No over/under-match found.
2. **No reachable panic** — the default-glob loop maps a bad glob to `IndexError::Glob { pattern:
   pattern.to_string(), source }`, identical shape to the user loop; the static set is valid so it
   never fires. No `unwrap/expect/panic` introduced.
3. **Idiomatic Rust + clippy** — `&&str` deref-coerces cleanly through `add_line`/`.to_string()`;
   clippy -D warnings clean. No needless clone/mut.
4. **Test adequacy** — the 4 integration + 3 config tests pin the full contract: regression,
   all-other-dirs+pyc, opt-out (proves the venv file is RETURNED, not merely no-error), and
   extend-not-replace (both default AND user glob apply). Config: default==true, omitted-key TOML
   loads true, explicit false round-trips. All assertions are value-equality on sorted paths, not
   `is_ok()`. Diff of the test files is purely additive (126 insertions in `indexer_tests.rs`, 0
   removed lines in either test file) — no existing test weakened.
5. **Alignment** — field `use_default_ignores: bool`, default `true`, matches §7.3 (project_plan
   line 1453) and D32 exactly. No scope creep; no new deps (`ignore` already present). Docs
   (project_plan §7.3/§5.1, ROADMAP D32, both module CLAUDE.md, TODO) updated in the same change.
6. Gate results above.

Non-blocking nit (pre-existing, NOT introduced by this slice, out of scope): project_plan.md:772
still shows a stale `discover_files(&config.index_paths, &config.ignore_patterns)` two-arg snippet;
the real shipped signature `discover_files(config, root)` is correct in code and in the §3.2.4 note
this slice updated. Recommend the manager file a doc-cleanup follow-up; does not gate this slice.

Slice is DONE-ready: APPROVE.
