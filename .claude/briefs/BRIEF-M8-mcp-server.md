# BRIEF — M8 — mcp_server / M8.0 (D15 entry evaluation)

- **Milestone:** M8 — mcp_server  ·  **Module(s):** mcp_server, cli/serve
- **Owner (manager):** principal-engineering-manager  ·  **Created:** 2026-06-12
- **Status:** D15 EVAL ✓  ·  DEP DECISION: **RESOLVED (2026-06-12) — HAND-ROLL JSON-RPC over stdio; serde/serde_json only, no new runtime dep. Human-ratified.**  ·  M8.1 ✓DONE (149) · M8.2 ✓DONE (154) · M8.3 RED ✓ GREEN ✓ REVIEW ✓ DONE ✓ (162 tests green) · M8.4 RED ▢
- **Links:** docs/ROADMAP.md#m8--mcp_server (D8/D13/D14/D15) · docs/plans/M8-mcp-server.md · docs/project_plan.md §8, §10.2, §10.3 · project_overview.md §2.5

## Goal
Expose CodeCache as an MCP server over stdio JSON-RPC with three tools (`codecache_search`,
`codecache_update`, `codecache_outline` — D13) plus self-healing search (D14), wired to
`codecache serve`. **This slice (M8.0) is the D15 entry gate ONLY:** decide `rmcp` vs hand-rolled
JSON-RPC and pin the choice before any RED/GREEN. No code lands this turn.

---
## M8.0 — D15 evaluation (rmcp vs hand-rolled JSON-RPC over stdio)

### Facts on `rmcp` as it stands (verified 2026-06-12)
- **Version / maturity:** latest `1.7.0` (2026-05-13). Post-1.0 (1.0.0 = 2026-03-03), biweekly minor
  cadence, ~12.4M total downloads, officially maintained by the modelcontextprotocol org. MIT/Apache-2.0.
- **Protocol:** targets MCP spec `2025-11-25`.
- **MSRV vs our 1.85.0 pin — the decisive friction.** rmcp declares **no `rust-version` (MSRV)**
  field, but uses **`edition = "2024"`**, so Rust **1.85 is the absolute hard floor** (edition 2024
  stabilized in 1.85). Secondary signals point *above* 1.85: the repo's `rust-toolchain.toml` pins
  **1.92**, DeepWiki's prerequisites state **1.90 minimum**, docs.rs built 1.7.0 on a 1.97 nightly.
  Net: **1.85.0 is unverified and probably too old**; with no declared MSRV and an active 1.92 dev
  toolchain, a `cargo update` can pull rmcp (or its deps) past 1.85 at any time — directly fighting
  our deliberate D10 MSRV contract.
- **Async runtime:** **tokio-required, async-only.** `tokio ^1` is a hard (non-optional) dep
  (`sync,macros,rt,time`); `serve()` is `async`, tool methods are `async fn`, transports are
  `AsyncRead`/`AsyncWrite`. Server must run under a tokio runtime.
- **stdio transport:** built-in via `transport-io` feature (`stdio()` → (stdin,stdout) pair).
  One-line server stand-up: `let svc = MyService::new().serve(stdio()).await?; svc.waiting().await?;`
- **Tool registration:** attribute macros `#[tool_router]` / `#[tool(description=...)]` /
  `#[tool_handler]`; **inputSchema is derived from a param struct via `schemars::JsonSchema`** (not
  hand-written serde_json). Hand-written schemas are possible but off the blessed path.
- **Dep weight (minimal stdio server `["server","transport-io"(,"macros")]`):** floor is
  **tokio + serde(+derive) + serde_json + async-trait + schemars 1.0 + pastey + rmcp-macros
  (syn/quote/proc-macro2) + base64**. hyper/axum/reqwest/tower/uuid are optional (NOT pulled for
  stdio). Estimated **~40–70 transitive crates** (not exactly verified — needs `cargo tree`).
  Cannot drop tokio; cannot drop schemars if using the macros.
- **Health:** active (last commit 2026-05-13), ~36 open issues / ~12 PRs, maintained 1.x migration guide.

### Hand-rolled JSON-RPC over stdio (the incumbent §10.2 plan)
- **Deps:** `serde` + `serde_json` only — **already in the tree**. Zero new runtime crates.
- **Scope is genuinely modest:** stdio + JSON-RPC 2.0 framing + `initialize` handshake + `tools/list`
  + `tools/call` for exactly 3 tools + strict error mapping (-32700/-32601/-32602). No SSE/HTTP (D4
  deferred). Estimated ~250–450 LOC of framing/dispatch under `mcp_server/`, fully under test.
- **Sync Storage fits natively:** D8's `Arc<Mutex<Connection>>` `Storage` is synchronous; a blocking
  stdin read-loop calls `Retriever`/`Indexer` directly with no runtime bridging. No tokio, no
  `block_on`, no async colouring of our otherwise-sync codebase.
- **Cost:** we own protocol-version drift (must hand-track MCP spec changes) and write/maintain the
  framing + schema JSON ourselves. Risk is bounded — the v0.1 surface is frozen at 3 tools + stdio.

### Sync/async Storage boundary (D8) under each path
- **Hand-roll:** no boundary — blocking loop → sync `Storage`/`Retriever`/`Indexer`. Clean.
- **rmcp:** forces a tokio runtime over a **synchronous** `Arc<Mutex<Connection>>`. Each async tool
  handler must call sync, blocking SQLite work — correct usage requires `spawn_blocking` (or accept
  blocking the async worker) to avoid stalling the reactor. Adds async/sync bridging complexity to a
  codebase that is otherwise deliberately synchronous. Net friction, not a fit.

### RECOMMENDATION (manager) — **HAND-ROLL JSON-RPC over stdio for v0.1; do NOT adopt rmcp now.**
Decisive reasons, in priority order:
1. **MSRV conflict with the deliberate 1.85.0 pin (D10).** rmcp has no declared MSRV, is developed on
   1.92, and is documented at 1.90 minimum. Adopting it either breaks the 1.85 contract or forces a
   pin bump chasing the ecosystem — exactly the whack-a-mole D10 rejected.
2. **Zero-dependency identity (D12 / §10.3).** rmcp drags in tokio + schemars + async-trait +
   proc-macro trees (~dozens of crates). CodeCache's one durable wedge is "zero-dependency,
   deterministic, single static binary, air-gapped." A heavy async SDK on the *one* optional surface
   is the wrong trade for v0.1.
3. **Async-over-sync friction (D8).** rmcp forces tokio onto a synchronous SQLite core; correct use
   needs `spawn_blocking` bridging. Hand-roll has zero boundary.
4. **Modest, frozen scope.** stdio + 3 tools + handshake is ~250–450 LOC over serde_json we already
   ship — well within our TDD discipline and cheaper to own than to bridge.

**Re-evaluate `rmcp` at v0.2**, when SSE/HTTP transports (D4) and richer protocol features make the
SDK's transport/codegen breadth pay for itself, and when an MSRV bump can be a deliberate decision
rather than forced. Keep `mcp_server` behind the D4 transport-agnostic seam so swapping in rmcp later
is an adapter change, not a refactor. **If the human prefers rmcp**, the entry condition is: a verified
`cargo +1.85.0 build` of rmcp 1.7 (or an agreed MSRV bump), acceptance of the tokio/schemars dep set
in §10.3, and a `spawn_blocking` boundary spec for D8.

> **DEP DECISION STATUS: RESOLVED (2026-06-12) — HAND-ROLL.** Human ratified hand-rolling JSON-RPC
> over stdio for v0.1 (serde/serde_json only; no new runtime dep; no `rmcp`). ROADMAP D15 flipped to
> RESOLVED; `project_plan.md` §10.2 updated; §10.3 confirmed needs no new runtime dep. RED/GREEN may
> proceed on the slice plan below.

### Proposed M8 slice breakdown (same shape either path; framing layer differs)
- **M8.1 — JSON-RPC framing + handshake.** `initialize` → server capabilities; malformed → -32700;
  unknown method → -32601; missing param → -32602; no panic. *(Hand-roll: we write the loop. rmcp:
  collapses into SDK; tests target our `ServerHandler`.)*
- **M8.2 — tool registration (tools/list).** All three tools with exact §8.2 inputSchema
  (search: query/max_tokens/file_filter · update: files[] · outline: path/max_tokens — D13).
- **M8.3 — tools/call round-trip.** `handle_search`→`Retriever::query`→agent-first markdown (D13);
  `handle_update`→`Indexer::update_files`→stats; `handle_outline`→storage symbol lookup by path
  prefix→skeleton from stored start/end lines (zero source reads — D7); bad args → -32602.
- **M8.4 — self-healing search (D14).** hash-check result files (`hasher::is_changed` vs
  `files_metadata`) → re-index changed → re-run query once → format; clean files = no writes; deleted
  file dropped, no panic; record staleness-window metric hook.
- **Cross-cutting (resolve before M8.1 GREEN):** D8 storage ownership (`Arc<Mutex<Connection>>` lent to
  retriever+indexer); `serve --transport stdio` replaces M7 stub; `--transport sse`/`--port` parse but
  return "unsupported in v0.1" (D4 seam kept).

---
## Definition of Done (M8 phase — enforced by manager)
- [ ] M8.0 D15 decision recorded in ROADMAP; dep pinned or manual path confirmed (BLOCKED on human signoff).
- [ ] M8.1–M8.4 green vs mock client; handshake + tools/list + tools/call round-trip.
- [ ] All three tool schemas match §8.2 exactly (D13); search output agent-first ordered.
- [ ] Self-healing search proven (D14 staleness tests).
- [ ] Malformed/unknown/invalid-params → correct JSON-RPC error codes, no panic.
- [ ] D8 ownership resolved; serve stub replaced; SSE/HTTP cleanly unsupported.
- [ ] clippy/fmt clean; reviewer APPROVED; docs/TODO.md Phase 8 + src/mcp_server/CLAUDE.md updated.

---
## RED — test lead

**Slice M8.1 — JSON-RPC framing over stdio + `initialize` handshake.** Dep sign-off RESOLVED
(hand-roll, serde/serde_json only). Tests written **first**; both files confirmed RED for the
right reason (compile error / unexpected success), not a typo.

### Files
- `tests/mcp_tests.rs` — NEW. 6 integration tests driving the server over an **in-memory**
  `serve(reader, writer, server)` seam (in-memory `Cursor`/`Vec<u8>`, no real stdio, no subprocess).
- `tests/e2e_cli.rs` — appended 1 cross-cutting `assert_cmd` E2E (test #6) for the serve transport.

### Pinned decisions (eng-lead + reviewer MUST honor — the tests are the contract)
1. **Framing = line-delimited JSON (newline-framed).** Exactly one JSON-RPC object per line, each
   request and response terminated by a single `\n`. No `Content-Length` headers. (Plan §8 says
   "newline/length-framed"; we pick newline for v0.1 simplicity.) Tests assert on the **raw bytes**:
   output ends with `\n`, one request ⇒ exactly one response line, no embedded newline in a frame.
2. **protocolVersion = `"2024-11-05"`** (stable MCP revision; plan §8 pins none, so this is the
   M8.1 decision). Hard-coded as `PROTOCOL_VERSION` in `mcp_tests.rs`; the `initialize` result must
   echo it. Change in lock-step if ever revised.
3. **Error codes:** parse error `-32700`, method-not-found `-32601`, invalid-params `-32602`.
   Every malformed/edge input → a structured JSON-RPC `error` object; the loop **never panics** and
   returns `Ok(())` at clean EOF.
4. **`initialize` result shape:** `{ protocolVersion, capabilities: {object}, serverInfo: { name:
   non-empty string, version: string } }` under `result`; response carries `jsonrpc:"2.0"` + echoed
   `id`; no `error` on the happy path.
5. **D8 storage seam:** `CodeCacheServer::new(Storage)` takes one shared `Storage`
   (`Arc<Mutex<Connection>>` clone) — proven to compile in the harness (`test_server()`). The
   handshake path itself does not read storage; the constructor takes it now so M8.2–M8.4 reuse the
   same seam unchanged. (This is the D8 confirmation — a dedicated redundant test is not needed; the
   harness constructing the real server is the structural proof.)

### REQUIRED entry-point signature (GREEN target — make these exist so `mcp_tests.rs` compiles)
```rust
// src/mcp_server/mod.rs
pub struct CodeCacheServer { /* holds Storage; Retriever/Indexer wired in M8.3 */ }

impl CodeCacheServer {
    /// D8: one shared Storage (Arc<Mutex<Connection>>) lent onward to Retriever/Indexer later.
    pub fn new(storage: codecache::storage::Storage) -> Self;
    // (intra-crate this is `crate::storage::Storage`; the test imports `codecache::storage::Storage`)
}

/// Transport-agnostic (D4) read→dispatch→write loop. Reads line-delimited JSON-RPC requests from
/// `reader`, writes line-delimited (`\n`-terminated) JSON-RPC responses to `writer`. Returns
/// Ok(()) at clean EOF; NEVER panics on malformed input. Generic R/W so tests inject in-memory
/// pipes; the real `serve` CLI handler calls `serve(stdin.lock(), stdout.lock(), server)`.
pub fn serve<R: std::io::BufRead, W: std::io::Write>(
    reader: R,
    writer: W,
    server: CodeCacheServer,
) -> anyhow::Result<()>;
```
Both `serve` and `CodeCacheServer` must be `pub` and re-exported from `mcp_server` (the test does
`use codecache::mcp_server::{serve, CodeCacheServer};`). Do **not** read tools/list, tools/call, or
self-healing in this slice — those are M8.2–M8.4.

### Tests (all RED now)
`tests/mcp_tests.rs`:
1. `initialize_request_returns_server_capabilities` — handshake → `result` with pinned
   protocolVersion + `capabilities` object + `serverInfo{name,version}`, echoed id, jsonrpc 2.0.
2. `malformed_json_returns_parse_error` — garbage line → `error.code == -32700`, jsonrpc 2.0, has
   `message`; no panic.
3. `unknown_method_returns_method_not_found` — valid envelope, unknown `method` → `-32601`, echoed id.
4. `missing_required_param_returns_invalid_params` — `initialize` with no `params` → `-32602`, echoed id.
5. `response_is_a_single_newline_terminated_json_line` — **framing**: raw bytes are one
   `\n`-terminated line, no embedded newline, round-trips as JSON-RPC 2.0 with echoed id 42.
6. `malformed_stream_never_panics_and_each_response_is_structured` — adversarial stream (non-json,
   bare array, scalar, unknown method, a good `initialize`, truncated object): `serve` returns Ok,
   every emitted line is independently parseable JSON-RPC, and the good `initialize` (id 100) still
   yields a success `result` — proving recovery after errors (the **no-panic-ever** guarantee).

`tests/e2e_cli.rs` (cross-cutting):
7. `e2e_serve_unsupported_transport_sse_errors_cleanly` — `serve --transport sse` on an initialized
   project exits **NONZERO** with a clean stderr naming the v0.1 limitation ("unsupported"/"not
   supported"), no `panicked` on either stream. (Chosen at the binary level via `assert_cmd` —
   precedent D17 / `e2e_cli.rs` — because exit-code + stderr is the contract under test, and it is
   lighter than asserting the blocking stdio path. The stdio happy-path is covered by the in-memory
   seam in `mcp_tests.rs`, which can't block on real stdin.)

### Confirmed RED output (Rust 1.85, this session)
- `cargo test --test mcp_tests --no-run`:
  ```
  error[E0432]: unresolved imports `codecache::mcp_server::serve`,
  `codecache::mcp_server::CodeCacheServer`
   --> tests\mcp_tests.rs:57:29
     | no `serve` in `mcp_server`  /  no `CodeCacheServer` in `mcp_server`
  error: could not compile `codecache` (test "mcp_tests") due to 1 previous error
  ```
  → correct reason: the M0 stub exports neither symbol yet (the GREEN target).
- `cargo test --test e2e_cli e2e_serve_unsupported_transport_sse_errors_cleanly`:
  ```
  test e2e_serve_unsupported_transport_sse_errors_cleanly ... FAILED
  panicked at ...: Unexpected success
  command=`...codecache.exe "serve" "--transport" "sse"`  code=0
  stdout="serve: the MCP server is not implemented yet (M8).\n"  stderr=""
  ```
  → correct reason: the M7 serve stub exits 0; GREEN must reject non-stdio transports nonzero.

### Run command
`cargo test --test mcp_tests` (and `cargo test --test e2e_cli` for the cross-cutting one).

### Notes / open items handed to eng-lead
- The `serve` CLI handler (`src/cli/serve.rs`) currently takes no args and ignores `--transport`.
  GREEN must thread `transport`/`db_path` through `dispatch` (`Command::Serve { transport, port,
  db_path }` → `serve::run(transport, port, &db_path)`): `stdio` → build `CodeCacheServer` from the
  resolved db + `serve(stdin.lock(), stdout.lock(), server)`; `sse` (or `port` set) → return a clean
  `anyhow::Error` "unsupported in v0.1" (D4 seam). No reachable `unwrap/expect/panic` in the handler.
- `notifications/initialized` and other post-handshake notifications are **out of scope** for M8.1
  (no test pins them); add when a later slice needs them.
- Did not need new fixtures or new deps (serde_json + tempfile + assert_cmd already in the tree).

## GREEN — engineering lead

**Slice M8.1 GREEN (2026-06-12).** Hand-rolled JSON-RPC 2.0 over a generic reader/writer per the
RED pin. serde/serde_json/anyhow only — no new deps, no rmcp, no tokio. All five gates green.

### Files changed
- `src/mcp_server/mod.rs` — implemented the server (was an empty stub). ~170 LOC incl. one unit test.
- `src/cli/serve.rs` — replaced the M7 stub: `run(transport, db_path)` → stdio loop or clean SSE error.
- `src/cli/mod.rs` — `dispatch` now threads `Command::Serve { transport, port: _, db_path }`
  through to `serve::run(transport, &db_path)` (was `serve::run()` dropping all args).
- `src/lib.rs` — no change needed; `pub mod mcp_server;` already declared, so
  `codecache::mcp_server::{serve, CodeCacheServer}` resolve.

### Final public API (matches the RED signature EXACTLY)
```rust
// src/mcp_server/mod.rs
pub struct CodeCacheServer { /* holds Storage (D8); Retriever/Indexer wired in M8.3 */ }
impl CodeCacheServer {
    pub fn new(storage: codecache::storage::Storage) -> Self;
}
pub fn serve<R: std::io::BufRead, W: std::io::Write>(
    reader: R, writer: W, server: CodeCacheServer,
) -> anyhow::Result<()>;
```
`CodeCacheServer` holds `Storage` behind `#[allow(dead_code)]` (handshake never reads it; the
constructor freezes the D8 seam for M8.2–M8.4 unchanged).

### Framing / protocol constants
- `PROTOCOL_VERSION = "2024-11-05"` (matches `mcp_tests.rs::PROTOCOL_VERSION`).
- `SERVER_NAME = "codecache"`; `serverInfo.version = crate::VERSION` (= `env!("CARGO_PKG_VERSION")`).
- Error codes: `PARSE_ERROR -32700`, `METHOD_NOT_FOUND -32601`, `INVALID_PARAMS -32602`.
- Framing: `serve` iterates `reader.lines()`; blank/whitespace lines are skipped (no frame
  emitted); each answered line writes exactly one `\n`-terminated JSON object via `write_frame`
  (`serde_json::to_string` + `push('\n')` + `write_all`). EOF → `writer.flush()` → `Ok(())`.

### How each RED test passes
1. `initialize_request_returns_server_capabilities` — `handle_initialize` returns
   `{ protocolVersion, capabilities:{}, serverInfo:{name,version} }` under `result`; envelope echoes
   `id` and carries `jsonrpc:"2.0"`, no `error`.
2. `malformed_json_returns_parse_error` — `serde_json::from_str` Err → `error_response(Null, -32700, …)`.
3. `unknown_method_returns_method_not_found` — `dispatch` default arm → `-32601`, id echoed.
4. `missing_required_param_returns_invalid_params` — `initialize` with no `params` →
   `-32602` (also rejects a `params` object missing `protocolVersion`).
5. `response_is_a_single_newline_terminated_json_line` — `write_frame` emits exactly one
   `\n`-terminated line, no embedded newline; round-trips with id 42.
6. `malformed_stream_never_panics_and_each_response_is_structured` — non-json → -32700; bare array
   / scalar → `as_object()` guard → -32700; unknown method → -32601; good `initialize` (id 100) →
   success `result`; truncated object → -32700. No reachable unwrap/expect/panic; loop returns
   `Ok(())` at EOF. The only `?`-propagated errors are reader/writer IO errors (real EOF is not an
   error — `lines()` ends the iterator).

### Cross-cutting (e2e #7)
`e2e_serve_unsupported_transport_sse_errors_cleanly` — `serve::run` matches `Transport::Sse` →
`bail!("SSE transport is not supported in v0.1 (stdio only)")` → nonzero exit, clean stderr, no
panic. `Transport::Stdio` resolves db (`paths::resolve` + `Storage::new`), builds the server, and
calls `serve(stdin().lock(), stdout().lock(), server)`.

### Deviations / notes
- **`--port` is NOT used to reject** in this slice. The brief body mentioned "sse (or a non-default
  port intent)", but `--port` has a clap default of 3000 (always present) and no test pins port
  behavior; rejecting on the default would be wrong. Only `--transport sse` errors (the exact e2e
  contract). `port` is bound as `port: _` in dispatch — available for a future SSE slice. Flagging
  for manager visibility; no plan/spec change made.
- `tests/mcp_tests.rs` (test-lead's untracked RED file) is NOT `cargo fmt`-clean as committed; I did
  not touch it (TDD: tests are the contract). All **production** files I changed are fmt-clean —
  `cargo fmt --check` shows diffs only in `tests/mcp_tests.rs`. Heads-up for the manager/CI: either
  the test lead reformats that file or CI's fmt gate will flag it independently of this slice.

### Gates (all green)
- `cargo test --test mcp_tests` → 6/6 pass.
- `cargo test --test e2e_cli` → 6/6 pass (incl. the SSE cross-cutting test).
- `cargo clippy --all-targets -- -D warnings` → clean.
- `cargo build` → clean.
- `cargo test` (full suite) → **149 passed, 0 failed** (27 lib unit + 122 integration).
- `cargo fmt --check` → production files clean; only `tests/mcp_tests.rs` (test-lead's file) differs.

## Specialist / Perf notes
_(framing overhead must be < few ms; search call bounded by M6 p95 < 500ms budget)_

## REVIEW — code reviewer

**VERDICT: BLOCK** (reviewed 2026-06-12, Rust 1.85). One blocker: `cargo fmt --check` is NOT
clean. Correctness, no-panic, dependency, and D4-seam properties all verified and good — the
block is hygiene-only and a one-command fix.

### Gate results
- `cargo build` → clean (exit 0).
- `cargo clippy --all-targets -- -D warnings` → clean (exit 0).
- `cargo test` → **149 passed, 0 failed** (27 lib unit + 122 integration; mcp_tests 6/6, e2e_cli 6/6).
- `cargo fmt --check` → **FAILS**: 5 unformatted hunks remain, all in `tests/mcp_tests.rs`
  (lines 96, 119, 176, 222, 255). Production `src/` is fmt-clean.

### Findings
- **blocker — tests/mcp_tests.rs:96,119,176,222,255 — `cargo fmt --check` is not clean.**
  The REVIEW prompt stated the manager applied "two fmt-only line-wrapping fixes" to make this
  file clean, but the on-disk file still has 5 rustfmt diffs (re-wrapping `.expect(...)` chains
  and `format!(...)` calls). The root CLAUDE.md hygiene gate and CI both require
  `cargo fmt --check` clean across the whole tree; CI would reject this as-is. The GREEN note
  itself flagged the file as not fmt-clean. **Fix:** run `cargo fmt` (formats the 5 hunks; no
  assertion text changes — verified the pending diff only re-wraps existing calls), then re-run
  `cargo fmt --check` to confirm clean. This touches only test scaffolding (`.expect`/`format!`
  layout), not any assertion, so it does not weaken the contract.

### What I verified GOOD (no change needed)
- **Framing/dispatch/write loop correctness** (mod.rs:89-106): reads line-delimited via
  `reader.lines()` (strips `
`/`
`), skips blank/whitespace lines without emitting a frame,
  writes exactly one `
`-terminated JSON object per answered line, flushes, returns `Ok(())` at
  clean EOF. Loop recovers after a malformed line (per-line `handle_line` never aborts the stream).
- **Error mapping exact** (mod.rs:27-29,110-136): parse/non-object → -32700; missing `method` →
  -32602; unknown method → -32601 (dispatch default arm); `initialize` missing `params` OR
  `params` lacking string `protocolVersion` → -32602. Matches the pinned contract.
- **`initialize` result shape** (mod.rs:73-80): `{ protocolVersion:"2024-11-05", capabilities:{},
  serverInfo:{ name:"codecache", version: crate::VERSION } }`; envelope echoes `id` verbatim
  (mod.rs:126, `.cloned().unwrap_or(Value::Null)` — correct null fallback) and carries
  `jsonrpc:"2.0"`. PROTOCOL_VERSION matches `mcp_tests.rs::PROTOCOL_VERSION`.
- **No reachable unwrap/expect/panic in production paths.** Scanned src/mcp_server/mod.rs and
  src/cli/serve.rs — none. Only `?`-propagated IO errors (line read / write_all / flush) and
  serde serialization error in `write_frame`. serve.rs maps `StorageError` via
  `anyhow::Error::new` (+`with_context`) — `StorageError` implements `std::error::Error`, so this
  is sound; lock-poison is `StorageError::LockPoisoned`, not a panic.
- **No new dependencies.** Cargo.toml `[dependencies]` unchanged; serde/serde_json/anyhow only.
  No rmcp, no tokio, no async. Honors D15 RESOLVED.
- **D4 transport seam clean** (serve.rs:20-25): `Transport::Sse` → `bail!` clean anyhow error →
  nonzero exit, no panic (e2e #7 asserts stderr names the limitation, no "panicked" on either
  stream). `serve` core is generic over `BufRead`/`Write` (D4); CLI passes `stdin/stdout` locks.
- **Tests not weakened.** The 6 mcp_tests assertions and the e2e test are intact; the pending
  rustfmt changes only re-wrap call layout, no assertion/expected-value edits.
- **Idiomatic Rust / clippy clean.** `let-else` guards, `ok_or_else`, borrowing not cloning on the
  hot path (only the necessarily-owned `id` is cloned), `#[allow(dead_code)]` on `storage` is
  justified (freezes the D8 seam for M8.2-M8.4).

### Re-review
Run `cargo fmt`, confirm `cargo fmt --check` is clean, then this is an **APPROVE** — no other
findings. (Recommend the manager also note the `--port` non-rejection deviation in M8.2+ planning;
it is correct for this slice since no test pins port behavior, so it is not a block.)


## OUTCOME — manager
D15 evaluation complete. Recommendation: **HAND-ROLL** (do not adopt rmcp for v0.1). Awaiting human
dep sign-off before sequencing RED. No code, no Cargo.toml, no ROADMAP disposition change this turn.

---
## RED — test lead (M8.2 — `tools/list` returns all three tools with exact §8.2 schemas, D13)

**Slice M8.2.** 5 new integration tests appended to `tests/mcp_tests.rs` (the M8.1 file). The
M8.1 harness (`test_server`, `run_server`, `single_response`) is REUSED unchanged; the six M8.1
tests are untouched and still pass. RED confirmed for the right reason: the server returns
`-32601 method not found: tools/list` (no `tools/list` handler yet).

### Files
- `tests/mcp_tests.rs` — appended an M8.2 section: 4 helpers + 5 tests (tests #7–#11). New helpers:
  `tools_list_request_line(id)`, `tools_list(id)`, `tools_array(resp)`, `find_tool(resp, name)`,
  `input_schema_properties(tool)`, `input_schema_required(tool)`. No edits to M8.1 code.

### Tests added (all RED now)
7. `tools_list_returns_all_three_tools` — `result.tools` is an array of length 3; name set is
   EXACTLY {`codecache_search`, `codecache_update`, `codecache_outline`}; each tool has a non-empty
   `description` and an `inputSchema` of `type:"object"`; id echoed, jsonrpc 2.0.
8. `tools_list_includes_codecache_search_with_input_schema` — `query` (string), `max_tokens`
   (integer, `default` JSON number `4000`), `file_filter` (string, `default` JSON `null`);
   `required == ["query"]`.
9. `tools_list_includes_codecache_update_with_input_schema` — `files` (array, `items.type ==
   "string"`); `required == ["files"]`.
10. `tools_list_includes_codecache_outline_with_input_schema` (D13) — `path` (string),
    `max_tokens` (integer, `default` JSON number `2000`); `required == ["path"]`.
11. `tools_list_tool_order_is_stable_and_deterministic` — id echoed, jsonrpc 2.0, tools emitted in
    the FIXED order [search, update, outline], identical across two `tools/list` calls.

### Pinned decisions the eng-lead MUST honor (the tests are the contract)
1. **`tools/list` request:** `{ "jsonrpc":"2.0", "id":N, "method":"tools/list" }` — `params` is
   optional/absent (MCP allows it; the test omits it). Dispatch must accept `tools/list` with no
   `params` and NOT reject it as invalid-params.
2. **Result shape the eng-lead must emit:**
   ```json
   { "jsonrpc":"2.0", "id":N,
     "result": { "tools": [ { "name", "description", "inputSchema" }, ... ] } }
   ```
   i.e. `result.tools` is an ARRAY of tool objects, each `{ name, description, inputSchema }`.
3. **Exactly 3 tools**, names `codecache_search`, `codecache_update`, `codecache_outline`. Each
   `description` non-empty; each `inputSchema.type == "object"`.
4. **`default` representation = JSON value of the property's own type.** `max_tokens` defaults are
   JSON NUMBERS (`4000` / `2000`), asserted via both `as_i64()` and `is_number()` — emitting them
   as strings (`"4000"`) FAILS. `file_filter`'s default is JSON `null` (`is_null()`), not the
   string `"null"` and not an omitted key.
5. **`required` arrays exact** (order asserted as written): search `["query"]`, update `["files"]`,
   outline `["path"]`.
6. **TOOL ORDER is fixed and deterministic: [`codecache_search`, `codecache_update`,
   `codecache_outline`].** Test #11 asserts this order AND that it is identical across two calls.
   The eng-lead must emit the tools in this stable order (e.g. a fixed array / `IndexMap`, not a
   `HashMap` iteration). §8.2 lists them Tool 1=search, Tool 2=update, Tool 3=outline — that is the
   pinned order.

### §8.2 schema fields asserted (verbatim from project_plan.md §8.2, lines ~1331–1427)
- **codecache_search.inputSchema.properties:** `query{type:string}`,
  `max_tokens{type:integer, default:4000}`, `file_filter{type:string, default:null}`;
  `required:["query"]`. (Property `description` strings are NOT asserted — only types/defaults/required.)
- **codecache_update.inputSchema.properties:** `files{type:array, items:{type:string}}`;
  `required:["files"]`.
- **codecache_outline.inputSchema.properties:** `path{type:string}`,
  `max_tokens{type:integer, default:2000}`; `required:["path"]`.

### Contract clarifications / what is NOT pinned (eng-lead latitude)
- **Per-property `description` strings are NOT asserted** by these tests (only tool-level
  `description` non-emptiness is). The eng-lead SHOULD still emit the §8.2 description text for
  client UX, but a wording change won't break M8.2 tests. Tool-level `description` MUST be non-empty.
- **No `additionalProperties` / `$schema` assertions.** The eng-lead may add them; the tests use
  `.get(...)` navigation (not strict equality on the whole schema), so extra keys are tolerated.
- **`tools/call` is OUT of scope** (M8.3). These tests only enumerate tools; they never invoke one.

### Confirmed RED output (Rust 1.85, this session)
`cargo test --test mcp_tests`:
```
test result: FAILED. 6 passed; 5 failed; 0 ignored
failures:
  tools_list_returns_all_three_tools
  tools_list_includes_codecache_search_with_input_schema
  tools_list_includes_codecache_update_with_input_schema
  tools_list_includes_codecache_outline_with_input_schema
  tools_list_tool_order_is_stable_and_deterministic
panicked: a well-formed tools/list must NOT produce an error object; got:
  {"error":{"code":-32601,"message":"method not found: tools/list"},"id":N,"jsonrpc":"2.0"}
```
→ correct reason: `dispatch`'s default arm returns -32601 for `tools/list`; no handler exists yet
(the GREEN target). The 6 M8.1 tests still pass (untouched, not weakened).

`cargo fmt --check` → clean (whole tree; `tests/mcp_tests.rs` formatted). The M8.1 fmt blocker is
not repeated.

### GREEN target for the eng-lead
Add a `"tools/list"` arm to `CodeCacheServer::dispatch` returning `Ok(json!({ "tools": [ … ] }))`
with the three tool objects in the fixed [search, update, outline] order and the exact §8.2
schemas above. No new deps; serde_json `json!` only. Keep `serve`/framing untouched.

## GREEN — engineering lead (M8.2)

**Slice M8.2 GREEN (2026-06-12).** `tools/list` now lists the three D13 tools with the exact
§8.2 inputSchemas in the pinned [search, update, outline] order. serde_json `json!` only — no
new deps, no rmcp, no tokio. All five gates green.

### Files changed
- `src/mcp_server/tools.rs` — **NEW.** Holds the three tool schemas as hand-written `json!`
  values mirroring §8.2 verbatim (incl. the real description text). `pub(crate) fn
  tool_definitions() -> Vec<Value>` returns them in the fixed order `vec![search_tool(),
  update_tool(), outline_tool()]` (a `Vec`, never `HashMap` iteration — guarantees determinism).
  One `fn` per tool keeps each schema readable.
- `src/mcp_server/mod.rs` — added `mod tools;`; added a `"tools/list"` arm to `dispatch` →
  `Ok(self.handle_tools_list())`; new `handle_tools_list(&self) -> Value` returns
  `json!({ "tools": tools::tool_definitions() })`. `serve`/framing/`initialize` untouched.

### Result shape emitted
```json
{ "jsonrpc":"2.0", "id":N,
  "result": { "tools": [
    { "name":"codecache_search",  "description":"…", "inputSchema":{…} },
    { "name":"codecache_update",  "description":"…", "inputSchema":{…} },
    { "name":"codecache_outline", "description":"…", "inputSchema":{…} } ] } }
```

### How each M8.2 test passes
7. `tools_list_returns_all_three_tools` — `tool_definitions()` returns exactly 3 objects, names
   {search, update, outline}, each with non-empty §8.2 `description` and `inputSchema.type ==
   "object"`. Envelope echoes `id`, `jsonrpc:"2.0"`, no `error` (dispatch returns `Ok`).
8. `..._codecache_search_...` — `query{string}`, `max_tokens{integer, default:4000}` (JSON
   number via `json!` literal `4000`), `file_filter{string, default:null}` (JSON `null`);
   `required:["query"]`.
9. `..._codecache_update_...` — `files{array, items:{type:"string"}}`; `required:["files"]`.
10. `..._codecache_outline_...` — `path{string}`, `max_tokens{integer, default:2000}` (JSON
    number); `required:["path"]`.
11. `tools_list_tool_order_is_stable_and_deterministic` — fixed `Vec` order [search, update,
    outline]; identical across two calls because `tool_definitions()` is a pure constructor.

### Deviations / notes
- None. `params` on `tools/list` is accepted-and-ignored (the arm takes no params); absent
  `params` is NOT rejected as invalid-params. `tools/call` execution remains out of scope (M8.3).
- Module split: schemas live in `src/mcp_server/tools.rs` (the plan names this file for
  schemas+handlers). `tool_definitions` is `pub(crate)`; only `mod.rs` consumes it.

### Gates (all green)
- `cargo test --test mcp_tests` → **11/11** (6 M8.1 + 5 M8.2).
- `cargo test` (full suite) → **154 passed, 0 failed** (was 149; +5 M8.2).
- `cargo clippy --all-targets -- -D warnings` → clean.
- `cargo fmt --check` → clean (whole tree).
- `cargo build` → clean.

## REVIEW — code reviewer (M8.2)

**VERDICT: APPROVE** (reviewed 2026-06-12, Rust 1.85). The three §8.2 tool schemas match the
plan EXACTLY; tool order is deterministic via `Vec`; `tools/list` accepts absent params and
echoes id; no reachable panic/unwrap/expect; no new deps; all four gates green.

### Gate results
- `cargo fmt --check` → clean (whole tree; the M8.1 fmt blocker is NOT repeated).
- `cargo clippy --all-targets -- -D warnings` → clean (exit 0).
- `cargo test` → **154 passed, 0 failed** (27 lib unit + 127 integration; mcp_tests 11/11).
- `cargo build` → clean (exit 0).

### Schema fidelity to §8.2 (the crux) — verified EXACTLY, char-by-char
- **codecache_search**: `query{type:"string"}`; `max_tokens{type:"integer", default:4000}`
  (JSON number); `file_filter{type:"string", default:null}` (JSON null); `required:["query"]`.
- **codecache_update**: `files{type:"array", items:{type:"string"}}`; `required:["files"]`.
- **codecache_outline (D13)**: `path{type:"string"}`; `max_tokens{type:"integer", default:2000}`
  (JSON number); `required:["path"]`.
- All three tool-level `description`s and all six property `description`s are the real §8.2 text
  verbatim (string-equality checked against project_plan.md — no placeholders).

### What I verified GOOD
- **Deterministic order via Vec** (tools.rs:15-17): `tool_definitions()` returns
  `vec![search_tool(), update_tool(), outline_tool()]` — fixed `[search, update, outline]`, never
  HashMap iteration. Test #11 confirms identical order across two calls.
- **tools/list accepts absent params** (mod.rs:51, 89-91): dispatch arm takes no params and is
  NOT routed through invalid-params; id echoed via the shared `handle_line` path; result shape is
  `{ tools: [...] }`. Minimal 3-line diff to mod.rs; framing/`initialize` untouched.
- **No reachable panic/unwrap/expect** in tools.rs or the mod.rs additions; pure `json!`
  constructors, no IO, no fallible calls.
- **No new deps**: `git diff HEAD -- Cargo.toml` empty; serde_json `json!` only.
- **Tests not weakened**: all 6 M8.1 + 5 M8.2 present and meaningful (assert on types, defaults
  via both `as_i64()` and `is_number()`, null via `is_null()`, exact `required`, stable order).

### Minor (non-blocking — manager close-out, brief protocol step 6)
- `docs/TODO.md:216` still shows M8.2 as `[ ]`, and `src/mcp_server/CLAUDE.md:42,47` still read
  "M8.2–M8.4 pending" / "149 tests". The root golden rule ties doc updates to the code change;
  these should be flipped to DONE / "154 tests" at manager close-out. Not a code-correctness
  block — the source+test contract is complete and correct.

---
## RED — test lead (M8.3 — `tools/call` round-trip: search / update / outline + D19 `symbols_for_path`)

**Slice M8.3.** Tests written **first**, split across two files. Existing M8.1+M8.2 tests and
harness REUSED unchanged (untouched, all 11 still pass). RED confirmed for the right reason.

### Files
- `tests/storage_tests.rs` — appended a **D19 section**: helper `outline_chunk(...)`,
  `seed_outline_storage()`, and **3 tests** driving the new `Storage::symbols_for_path` +
  `types::SymbolOutline`. RED = **compile error** (the method + type do not exist yet).
- `tests/mcp_tests.rs` — appended an **M8.3 section**: helpers `seed_chunk(...)`,
  `test_server_seeded(&[Chunk])`, `tools_call_request_line`, `tools_call(server,id,tool,args)`,
  `call_result_text(resp)`, `assert_error_code(resp,code,id)`, and **4 tests** (#12–#15) driving
  `tools/call`. RED = **runtime failure** (`tools/call` unhandled → dispatch default arm returns
  `-32601 method not found: tools/call`). These use only EXISTING public APIs (`serve`,
  `CodeCacheServer::new`, `codecache::{init,index}`, `Storage`), so they compile today; the server
  internals (Retriever/Indexer wiring + handlers) are the GREEN target inside the crate.

### Tests added (all RED now)
`tests/storage_tests.rs` (D19, compile-RED):
- `symbols_for_path_exact_file_returns_its_symbols_ordered` — exact `src/a.py` → only its 3
  symbols, ordered by start_line (1,3,10); asserts slim projection round-trips `symbol_type`
  (typed enum), `parent_symbol` (`Some("a_class")` for the method), D7 line ranges.
- `symbols_for_path_directory_prefix_returns_all_under_it` — `src` (dir) → `src/a.py` +
  `src/sub/b.py` symbols, NOT `other.py`; ordered by `(file_path, start_line, end_line)`.
- `symbols_for_path_unknown_path_returns_empty` — unknown path → empty `Vec`, not an error.

`tests/mcp_tests.rs` (M8.3, runtime-RED):
12. `call_codecache_search_returns_formatted_results` — seeded index; `tools/call codecache_search
    {query:"authenticate user"}` → `result.content[0]{type:"text",text}`; text contains the seeded
    symbol `authenticate_user`, the locator `src/auth.py:45-67`, and the agent-first header
    (`Query:` echo + `Found`). Substring asserts (not full-string) so wording stays the eng-lead's.
13. `call_codecache_update_reindexes_and_reports_stats` — builds a REAL project via
    `codecache::init` + writes `mod.py` + `codecache::index`, then a server over that DB;
    `tools/call codecache_update {files:[<abs mod.py>]}` → text reports stats (substrings
    `"1 file"` and `"chunk"`, mirroring §8.3 "Updated N files, indexed M chunks").
14. `call_codecache_outline_returns_symbol_skeleton` — seeded symbols; FILE path `src/a.py` →
    skeleton listing `Greeter`/`greet` with `src/a.py:1-20`, and NOT `src/sub/b.py`; DIRECTORY
    path `src` → spans both files (`Greeter` + `helper`, `src/a.py:1-20` + `src/sub/b.py:1-4`).
15. `call_with_bad_arguments_returns_invalid_params` — search w/o `query`, outline w/o `path`,
    update w/o `files`, AND an unknown tool name → all `-32602`, id echoed.

### Pinned decisions (the eng-lead MUST honor — the tests are the contract)
- **`symbols_for_path` signature:** `pub fn symbols_for_path(&self, path: &Path) ->
  storage::Result<Vec<SymbolOutline>>`. **`SymbolOutline`** lives in `codecache::types` with fields
  `{ symbol_name:String, symbol_type:SymbolType, parent_symbol:Option<String>, file_path:PathBuf,
  start_line:usize, end_line:usize }` (typed `SymbolType`, 1-based inclusive lines D7); derive at
  least `Debug+Clone+PartialEq+Eq`. **Semantics:** exact `file_path = ?` OR directory prefix
  `<dir>/%` (escape SQL `LIKE` wildcards `%`/`_` in the path); **ordering** `(file_path, start_line,
  end_line)` ascending; unknown path → empty `Vec`, never an error. A plain column `SELECT` on the
  contentful FTS5 `symbols` table reading the UNINDEXED line columns — zero source reads (D7).
- **`tools/call` envelope:** request `params:{ name, arguments }`; success `result` =
  `{ content:[ { type:"text", text:<string> } ] }` (non-empty array, first elem text). Pinned
  exactly by `call_result_text`.
- **search handler:** `arguments.query` (required) → `Retriever::query` → §6.4.3 text formatter
  (D13 agent-first). `max_tokens` optional (default 4000 per §8.2). Reuse `formatter::format(…,
  Format::Text)` so the locator/header shape matches the M7 goldens.
- **update handler:** `arguments.files` (required, array of strings) → `Indexer::update_files`
  over the project root → text "Updated {files_processed} files, indexed {chunks_indexed}
  chunks…" (§8.3). Requires the server to build an `Indexer` (needs `Config` + root); the test
  drives it through a real on-disk `init`/`index`ed project.
- **outline handler:** `arguments.path` (required) → `Storage::symbols_for_path` → D13 text
  skeleton line per symbol carrying `<symbol> … <file>:<start>-<end>`. File OR directory path.
- **error mapping (PINNED, decision #5):** missing/wrong-typed required arg → `-32602`; **unknown
  tool name → `-32602`** (the tool `name` is a *param* of `tools/call`, so a bad name is an invalid
  param, NOT `-32601`). `-32601` stays reserved for an unknown top-level JSON-RPC `method`.

### Required production API surface (GREEN target for the eng-lead)
- `codecache::types::SymbolOutline` (new struct, fields above).
- `Storage::symbols_for_path(&self, &Path) -> Result<Vec<SymbolOutline>>` (new, additive; new
  `queries::SYMBOLS_FOR_PATH` column SELECT + LIKE-escape helper — hand to rust-treesitter-specialist
  for the FTS5 `SELECT`/`LIKE`-escape detail per D19).
- `mcp_server`: a `"tools/call"` arm in `CodeCacheServer::dispatch` → `handle_call(&mut self, name:
  &str, arguments: &Value) -> Result<Value, RpcError>` routing to `handle_search`/`handle_update`/
  `handle_outline`; unknown name → invalid-params (-32602). `CodeCacheServer` must now hold/lazily
  build a `Retriever` + `Indexer` over its shared `Storage` (D8) — the `#[allow(dead_code)]` on
  `storage` from M8.1 comes off here. `handle_update` mutates (Indexer::update_files), so the call
  path needs `&mut self` — confirm `serve`/`dispatch` thread `&mut` (M8.1 `handle_line` may need a
  `&mut` upgrade; no test pins immutability).

### Note for the specialist (could not fixture cheaply)
- **SQL `LIKE` wildcard guard:** a stored/queried path containing `%` or `_` must not over-match in
  the directory-prefix branch. I did not add a dedicated fixture (constructing a path literally named
  `a%b/` is awkward + platform-touchy on Windows). The eng-lead/specialist MUST escape `%`/`_` in the
  prefix (e.g. `LIKE ?2 ESCAPE '\'`) per D19; a unit test in `storage` covering the escape is the
  right home. Flagging so it is not silently skipped.

### Confirmed RED output (Rust 1.85, this session)
- `cargo test --test storage_tests --no-run`:
  ```
  error[E0432]: unresolved import `codecache::types::SymbolOutline`
  error[E0599]: no method named `symbols_for_path` found for struct `codecache::storage::Storage` …
  error: could not compile `codecache` (test "storage_tests") due to 4 previous errors
  ```
  → correct reason: the D19 method + type are the GREEN target.
- `cargo test --test mcp_tests`:
  ```
  test result: FAILED. 11 passed; 4 failed; 0 ignored
  failures: call_codecache_search_returns_formatted_results,
    call_codecache_update_reindexes_and_reports_stats,
    call_codecache_outline_returns_symbol_skeleton,
    call_with_bad_arguments_returns_invalid_params
  panicked: a successful tools/call must NOT carry an error; got:
    {"error":{"code":-32601,"message":"method not found: tools/call"},...}
  ```
  → correct reason: `dispatch` default arm returns -32601 for `tools/call`; no handler yet. The 11
  M8.1+M8.2 tests still pass (untouched, not weakened).
- `cargo fmt --check` → clean (whole tree). The M8.1 fmt blocker is not repeated.

### Run command
`cargo test --test mcp_tests` (+ `cargo test --test storage_tests` once the D19 method compiles).

## GREEN — engineering lead (M8.3)

**Slice M8.3 GREEN (2026-06-12).** `tools/call` round-trip (search / update / outline) + the
additive D19 `Storage::symbols_for_path` + `types::SymbolOutline`. serde/serde_json/anyhow only —
no new deps, no rmcp, no tokio. Did NOT consult the rust-treesitter-specialist: the FTS5
`LIKE`/`ESCAPE` detail was unambiguous from D19 + §3.2.2 (a plain column SELECT on the contentful
table; `ESCAPE '\'` with the path-portion wildcards escaped). All five gates green; 162 tests.

### Files changed
- `src/types/mod.rs` — added `pub struct SymbolOutline { symbol_name, symbol_type, parent_symbol,
  file_path, start_line, end_line }` (Debug+Clone+PartialEq+Eq; dependency-free per D5).
- `src/storage/queries.rs` — added `SYMBOLS_FOR_PATH` (column SELECT, `file_path = ?1 OR file_path
  LIKE ?2 ESCAPE '\'`, ORDER BY `(file_path, start_line, end_line)`).
- `src/storage/mod.rs` — added `symbols_for_path(&self, &Path) -> Result<Vec<SymbolOutline>>`, the
  private `escape_like` LIKE-wildcard escaper (`\`→`\\` first, then `%`→`\%`, `_`→`\_`), the
  `map_outline_row` mapper (typed `SymbolType::from_str_lenient`; unknown → `CorruptRow`, no
  panic), and a unit test `escape_like_escapes_wildcards_and_backslash`.
- `src/mcp_server/mod.rs` — `storage` field's `#[allow(dead_code)]` removed; `dispatch`/`handle_line`/
  `serve` upgraded to `&mut self`/`&mut server` (handle_update mutates the index); added the
  `"tools/call"` dispatch arm → `handle_tools_call` (parses `params.name`+`arguments`; routes to the
  three handlers; unknown name → -32602; success → `{content:[{type:"text",text}]}`). Framing /
  initialize / tools-list behavior unchanged (11 prior tests still green).
- `src/mcp_server/handlers.rs` — **NEW.** `handle_search`/`handle_update`/`handle_outline` over the
  shared `Storage` (D8 `.clone()`), each returning the text payload or `(code,message)`. Arg parsing
  helpers (`require_str`, `optional_usize`) map missing/mistyped required args → -32602; internal
  failures → -32603. `render_skeleton` emits the D13 `[n] <qualified> (<type>) file:s-e` locator
  line per symbol with a soft `max_tokens` cap.

### New public API
- `codecache::types::SymbolOutline` (struct above).
- `Storage::symbols_for_path(&self, path: &Path) -> storage::Result<Vec<SymbolOutline>>` — exact
  file OR `<dir>/%` prefix (wildcards escaped), ordered `(file_path, start_line, end_line)`, unknown
  path → empty Vec, zero source reads (D7).
- `mcp_server`: `tools/call` now handled; `serve` loop is `&mut server` (signature unchanged — the
  `serve<R,W>(reader, writer, server)` shape is identical; only the internal binding became `mut`).

### How each test passes
- D19 `symbols_for_path_exact_file_returns_its_symbols_ordered` — `file_path = ?1` matches `src/a.py`
  only; ORDER BY start_line yields a_class(1)/a_method(3)/b_func(10); slim projection round-trips
  typed `SymbolType`, `parent_symbol`, D7 lines.
- D19 `..._directory_prefix_returns_all_under_it` — `?2 = "src/%"` matches `src/a.py` + `src/sub/b.py`
  but NOT `other.py`; ordered by `(file_path, start_line)`.
- D19 `..._unknown_path_returns_empty` — neither branch matches → empty Vec via the row loop.
- #12 search — `handle_search` → `Retriever::query` → `formatter::format(.., Format::Text)`; text
  carries `authenticate_user`, `src/auth.py:45-67`, `Query:` echo, `Found`.
- #13 update — `handle_update` clears each file's `files_metadata` row (so the unchanged-but-
  explicitly-named file registers as changed) then `Indexer::update_files`; output `"Updated 1 file,
  indexed N chunks in Tms"` contains `"1 file"` + `"chunk"`.
- #14 outline — `symbols_for_path` for `src/a.py` (file) lists Greeter/greet with `src/a.py:1-20`,
  excludes `src/sub/b.py`; for `src` (dir) spans both files (`src/a.py:1-20` + `src/sub/b.py:1-4`).
- #15 bad args — missing `query`/`path`/`files` → -32602; unknown tool name → -32602 (per pinned
  decision #5, the `name` is a param of `tools/call`).

### Deviations / decisions (flag for manager + reviewer)
- **handle_update forces re-index of explicitly-named files.** `Indexer::update_files` skips files
  whose on-disk hash equals the stored hash (M5.3 idempotency). Test #13 indexes the file, then calls
  `codecache_update` on the SAME unchanged content and asserts `1 file` processed. The MCP `update`
  tool is an explicit "re-index these now" request, so the handler deletes each named file's
  `files_metadata` row first (public `Storage::delete_file_meta`), making it look new to
  `detect_changed_files`. This does **not** weaken `update_files`' own idempotency contract (the M5.3
  tests are untouched and green) — it is the handler's documented semantic. No plan/spec change.
- **Internal-error code -32603.** Retrieval/index/storage failures inside a handler surface as the
  JSON-RPC standard internal-error code -32603 (no test exercises this path; -32602 stays reserved for
  argument-shape failures per the RED pins). Flagging for visibility; no test pins -32603.
- **Indexer root for handle_update.** Built with `Config::default()` + root `"."`; `update_files`
  re-indexes the explicit paths and never walks `root`, so the root value is inert here.
- **tests/storage_tests.rs hygiene fix (test-lead's file).** The committed RED file had a dead
  `use codecache::types::SymbolOutline;` (line 496) — the type is referenced only in comments, never
  in the test body, so `cargo clippy --all-targets -D warnings` (a required gate) failed on
  `unused-imports`. I removed ONLY that one dead import line. This changes no assertion, no coverage,
  no test behavior (provably dead code — the standard `cargo fix` suggestion); it is the same class of
  test-file hygiene fix the M8.1 reviewer required (fmt). All 7 new + 11 prior assertions are intact.
  Flagging explicitly per the no-modify-tests rule so the manager/test-lead/reviewer can confirm.

### Gates (all green, Rust 1.85)
- `cargo test --test mcp_tests` → 15/15 (6 M8.1 + 5 M8.2 + 4 M8.3).
- `cargo test --test storage_tests` → 21/21 (18 prior + 3 D19).
- `cargo test` (full suite) → **162 passed, 0 failed** (was 154; +3 D19 + 4 tools/call + 1 new
  `escape_like` storage unit test).
- `cargo clippy --all-targets -- -D warnings` → clean.
- `cargo fmt --check` → clean (whole tree).
- `cargo build` → clean.

---
## REVIEW — code reviewer (M8.3)

**VERDICT: APPROVE** (reviewed 2026-06-12, Rust 1.85). The D19 `symbols_for_path` SQL +
`escape_like` are correct and injection-safe; `tools/call` dispatch + the three handlers match
§8.2/§8.3 and the brief's pinned error mapping; no reachable panic/unwrap/expect in new production
code; no new deps; all four gates green at 162 tests. Both flagged test-file deviations verified
benign.

### Gate results
- `cargo fmt --check` → clean (exit 0, whole tree).
- `cargo clippy --all-targets -- -D warnings` → clean (exit 0).
- `cargo test` → **162 passed, 0 failed** (28 lib unit + 134 integration; mcp_tests 15/15,
  storage_tests 21/21, indexer_tests 15/15). Matches expected 162.
- `cargo build` → clean (exit 0).

### D19 `symbols_for_path` SQL + escaping — VERIFIED CORRECT
- **Parameterized, no interpolation.** `queries::SYMBOLS_FOR_PATH` binds `?1` (exact) and `?2`
  (prefix) via `params![exact, prefix]`; the path text never enters the SQL string. Injection-safe.
- **Exact-vs-prefix is correct.** `WHERE file_path = ?1 OR file_path LIKE ?2 ESCAPE '\'` with
  `?2 = "<escaped path>/%"`. Querying `src` builds prefix `src/%`: a sibling `srcfoo.py` is `!= 'src'`
  and does NOT match `src/%` (no `/` after `src`), while `src/a.py` and `src/sub/b.py` do. The
  test `symbols_for_path_directory_prefix_returns_all_under_it` proves `other.py` is excluded and
  the two `src/` files included; exact-file test confirms a file query returns only its own symbols.
- **Escaping is correct and ordered.** `escape_like` replaces `\`→`\` FIRST, then `%`→`\%`,
  `_`→`\_`, so a literal `%`/`_` in a path becomes a literal under `ESCAPE '\'` and cannot
  over-match; the caller-appended `/%` stays an unescaped wildcard. The unit test
  `escape_like_escapes_wildcards_and_backslash` pins all three cases incl. the
  escape-the-escape-char-first ordering (`a\%b` → `a\\%b`).
- **Deterministic ordering.** `ORDER BY file_path, start_line, end_line` — the seed test inserts
  `src/a.py` rows out of order (10,1,3) and asserts they come back (1,3,10), proving the SQL sort,
  not insert echo.
- **Zero source reads (D7).** A plain column SELECT over the contentful `symbols` table reading the
  stored UNINDEXED line columns — no `std::fs`, no re-parse anywhere in the path.
- **Slim projection.** Returns `SymbolOutline {symbol_name, symbol_type, parent_symbol, file_path,
  start_line, end_line}` — no `chunk_text`/imports. Matches §3.2.2/D19.
- **No panic on corrupt row.** `map_outline_row` defers `SymbolType::from_str_lenient` into the
  inner `Result`, mapping an unknown stored `symbol_type` to `StorageError::CorruptRow` (same
  pattern as `map_search_row`). Unknown path → empty `Vec` (test confirms), never an error.

### tools/call dispatch + handlers — VERIFIED
- **Error mapping matches the brief.** Missing/mistyped required args (search `query` via
  `require_str`, outline `path` via `require_str`, update `files` via the array/string checks) →
  `-32602`; an unknown tool name → `-32602` (the `other =>` arm), NOT `-32601`. Test #15 (a–d)
  covers all four. Internal retrieval/index/storage failures → `-32603` via `?` on the mapped
  `Result` — reasonable and not papering over a panic (every `map_err` wraps a real typed error).
- **Success envelope exact.** `handle_tools_call` returns
  `json!({ "content": [ { "type":"text", "text": text } ] })` — matches §8.2 and test
  `call_result_text` (non-empty array, first elem `{type:"text", text:<string>}`).
- **`&mut self` serve-loop change did not regress M8.1/M8.2.** `dispatch`/`handle_tools_call` take
  `&mut self` (needed because update mutates the index); `serve` now takes `mut server` and
  `handle_line(&mut server, ...)`. `initialize`/`tools/list` are unchanged behaviorally — all 11
  M8.1+M8.2 tests still pass (framing, handshake, error codes, no-panic recovery, stable tool order).

### Two flagged test-file deviations — BOTH VERIFIED BENIGN
- **(a) `handle_update` deletes each file's `files_metadata` row before `update_files`.** This uses
  the public `Storage::delete_file_meta` (M5.3 API) in the MCP handler only; it does NOT touch
  `Indexer::update_files`/`detect_changed_files`. The "re-index these NOW" tool semantic is
  defensible (an agent calling update expects work even on byte-identical content), and the M5.3
  idempotency contract is untouched — all 15 `indexer_tests` (incl. the no-write idempotency tests)
  pass unchanged, since none route through the MCP path. Not a hack hiding a bug.
- **(b) the "removed dead import" in `tests/storage_tests.rs`.** Verified: `git diff HEAD --
  tests/storage_tests.rs` shows **ZERO deletions** (191 insertions, 0 deletions) — likewise
  `mcp_tests.rs` (399 insertions, 0 deletions). Both test files are purely additive. The only
  `SymbolOutline` occurrences in storage_tests.rs are in the comment block (lines 472–486); the 3
  D19 tests construct/inspect `SymbolOutline` via field access only, so no `use` import was ever
  needed. The transient dead-import never landed. No assertion or coverage was weakened; the 3 D19
  tests assert all six fields (`symbol_name`/`file_path` via the names+scoping checks,
  `symbol_type`, `parent_symbol`, `start_line`, `end_line` via direct field asserts).

### No new deps / idiomatic
- `git diff HEAD -- Cargo.toml` empty (not shown changed in the diff stat). serde/serde_json only.
- No reachable `unwrap`/`expect`/`panic!` in `handlers.rs`, the `mod.rs` additions, or
  `storage/mod.rs` `symbols_for_path`/`escape_like`/`map_outline_row` (all fallible steps via `?`
  or `map_err`; `writeln!` into a `String` is infallible and its `Result` is `let _`-discarded).

### Minor (non-blocking — manager close-out)
- `optional_usize` accepts `max_tokens: 0` (→ `Some(0)`), which makes the outline soft-cap emit
  only the header. This is a benign caller choice, not a bug, and no test pins it; noting only so
  the manager is aware the budget has no documented floor.
- Doc close-out per protocol step 6: flip M8.3 to DONE and update `src/mcp_server/CLAUDE.md`
  (still reads "M8.3–M8.4 pending") + `src/storage/CLAUDE.md` (add `symbols_for_path`) +
  `src/types/CLAUDE.md` (add `SymbolOutline`) + `docs/TODO.md` to "162 tests". Not a code block.
