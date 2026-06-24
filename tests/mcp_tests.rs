//! M8.1 — MCP server: JSON-RPC framing over stdio + `initialize` handshake (RED, test-lead).
//!
//! These tests drive the (not-yet-implemented) `mcp_server` over an **in-memory** reader/writer
//! pair — feeding it `impl BufRead` and capturing `impl Write` — so the read/dispatch/write loop
//! is exercised WITHOUT spawning the real process or touching real stdin/stdout. This requires the
//! engineering lead to expose a generic, unit-testable server entry point (see the REQUIRED
//! SIGNATURE note below); these tests are written against that contract and so currently fail to
//! compile (the `mcp_server` module is an empty stub). That compile failure IS the RED state.
//!
//! ── PINNED PROTOCOL DECISIONS (M8.1 — honor these in GREEN) ──────────────────────────────────
//!
//! 1. FRAMING = **line-delimited JSON** (newline-framed): exactly one JSON-RPC object per line,
//!    each request and each response terminated by a single `\n`. (The plan §8 says
//!    "newline/length-framed"; we pick newline framing for v0.1 simplicity. No `Content-Length`
//!    headers.) A response written to the writer must therefore be one `\n`-terminated line that
//!    parses as a JSON-RPC 2.0 object.
//!
//! 2. protocolVersion advertised by the server = **"2024-11-05"** (the stable MCP protocol revision;
//!    the project plan does not pin one, so this is the M8.1 decision the eng-lead must match in the
//!    `initialize` result). If the eng-lead must change it, change THIS constant in lock-step and
//!    note it in the brief — the test is the contract.
//!
//! 3. ERROR CODES (JSON-RPC 2.0): parse error = -32700; method not found = -32601; invalid params
//!    = -32602. Every malformed/edge input returns a structured error object; the server NEVER
//!    panics / unwinds across the loop.
//!
//! ── REQUIRED ENTRY-POINT SIGNATURE (what GREEN must expose) ──────────────────────────────────
//!
//! A generic, reader/writer-injected server loop plus a storage-backed context constructor:
//!
//! ```ignore
//! // src/mcp_server/mod.rs
//! pub struct CodeCacheServer { /* holds storage; retriever/indexer wired in M8.3 */ }
//!
//! impl CodeCacheServer {
//!     /// Build a server over a shared `Storage` (D8: one Arc<Mutex<Connection>> lent onward).
//!     pub fn new(storage: codecache::storage::Storage) -> Self;
//! }
//!
//! /// The transport-agnostic (D4) read→dispatch→write loop. Reads line-delimited JSON-RPC
//! /// requests from `reader`, writes line-delimited JSON-RPC responses to `writer`. Returns
//! /// Ok(()) at clean EOF; never panics on malformed input. `R`/`W` are generic so tests can
//! /// inject in-memory pipes instead of real stdio.
//! pub fn serve<R: std::io::BufRead, W: std::io::Write>(
//!     reader: R,
//!     writer: W,
//!     server: CodeCacheServer,
//! ) -> anyhow::Result<()>;
//! ```
//!
//! M8.1 only exercises framing + `initialize` + error mapping; it does NOT call tools/list or
//! tools/call (those are M8.2–M8.4). The handshake path does not touch storage, but the
//! constructor takes `Storage` now so the same seam serves the later slices unchanged.

use std::io::Cursor;

use codecache::mcp_server::{serve, CodeCacheServer};
use codecache::storage::Storage;

/// The protocol revision the server must advertise in its `initialize` result. Pinned by M8.1;
/// change in lock-step with the implementation (see header decision #2).
const PROTOCOL_VERSION: &str = "2024-11-05";

// ── Test harness ────────────────────────────────────────────────────────────────────────────

/// Build a `CodeCacheServer` over a fresh, schema-initialized in-temp-dir database. M8.1's
/// handshake path never reads storage, but constructing the real server (not a mock) keeps the
/// seam honest for M8.2–M8.4 and proves D8 storage-lending compiles.
fn test_server() -> (CodeCacheServer, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp.path().join("index.db");
    let storage = Storage::new(&db_path).expect("open storage");
    storage.init_schema().expect("init schema");
    (CodeCacheServer::new(storage), tmp)
}

/// Drive the server with `input` as the entire (line-delimited) request stream and return the raw
/// bytes the server wrote. Uses in-memory `Cursor`s so nothing touches real stdio (the generic
/// reader/writer seam the eng-lead must provide).
fn run_server(input: &str) -> Vec<u8> {
    let (server, _tmp) = test_server();
    let reader = Cursor::new(input.as_bytes().to_vec());
    let mut output: Vec<u8> = Vec::new();
    // The loop must terminate at EOF and must not panic on any input.
    serve(reader, &mut output, server).expect("serve loop returns Ok at clean EOF");
    output
}

/// Parse the server's output as exactly one line-delimited JSON-RPC response object (framing
/// contract: one `\n`-terminated JSON object). Asserts the line really is `\n`-terminated and
/// that there is exactly one response line, then returns the parsed `Value`.
fn single_response(output: &[u8]) -> serde_json::Value {
    let text = std::str::from_utf8(output).expect("server output must be valid UTF-8");
    assert!(
        text.ends_with('\n'),
        "line-delimited framing: every response line must be \\n-terminated; got: {text:?}"
    );
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let first = lines
        .next()
        .expect("server must write at least one response line");
    let value: serde_json::Value =
        serde_json::from_str(first).expect("each response line must be parseable JSON-RPC");
    assert!(
        lines.next().is_none(),
        "exactly one response expected for a single request; got extra lines in: {text:?}"
    );
    value
}

/// A well-formed JSON-RPC 2.0 `initialize` request line (newline-framed). Mirrors what an MCP
/// client (e.g. Claude Code) sends to open the session.
fn initialize_request_line(id: i64) -> String {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.0.0" }
        }
    });
    format!(
        "{}\n",
        serde_json::to_string(&req).expect("serialize request")
    )
}

// ── 1. initialize handshake → server capabilities ────────────────────────────────────────────

/// A well-formed `initialize` request yields a JSON-RPC 2.0 response with the SAME `id`,
/// `jsonrpc: "2.0"`, and a `result` carrying the pinned protocolVersion, a `capabilities` object,
/// and `serverInfo` (name + version). Pins the handshake response shape.
#[test]
fn initialize_request_returns_server_capabilities() {
    let output = run_server(&initialize_request_line(1));
    let resp = single_response(&output);

    assert_eq!(
        resp.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "response must carry jsonrpc \"2.0\"; got: {resp}"
    );
    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(1),
        "response id must echo the request id; got: {resp}"
    );
    assert!(
        resp.get("error").is_none(),
        "a well-formed initialize must NOT produce an error object; got: {resp}"
    );

    let result = resp
        .get("result")
        .expect("initialize response must carry a `result`");

    assert_eq!(
        result.get("protocolVersion").and_then(|v| v.as_str()),
        Some(PROTOCOL_VERSION),
        "server must advertise the pinned protocolVersion; got: {result}"
    );
    assert!(
        result
            .get("capabilities")
            .map(|c| c.is_object())
            .unwrap_or(false),
        "initialize result must carry a `capabilities` object; got: {result}"
    );

    let server_info = result
        .get("serverInfo")
        .expect("initialize result must carry `serverInfo`");
    assert!(
        server_info
            .get("name")
            .and_then(|v| v.as_str())
            .map(|n| !n.is_empty())
            .unwrap_or(false),
        "serverInfo.name must be a non-empty string; got: {server_info}"
    );
    assert!(
        server_info
            .get("version")
            .and_then(|v| v.as_str())
            .is_some(),
        "serverInfo.version must be a string; got: {server_info}"
    );
}

// ── 2. malformed JSON → parse error (-32700), no panic ───────────────────────────────────────

/// Unparseable input on a request line maps to a JSON-RPC error object with `code == -32700`
/// (Parse error) and `jsonrpc: "2.0"`. The server must not panic on garbage bytes.
#[test]
fn malformed_json_returns_parse_error() {
    // Not valid JSON at all.
    let output = run_server("this is not json at all{{{\n");
    let resp = single_response(&output);

    assert_eq!(
        resp.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "even an error response must carry jsonrpc \"2.0\"; got: {resp}"
    );
    let error = resp
        .get("error")
        .expect("malformed JSON must produce a JSON-RPC `error` object");
    assert_eq!(
        error.get("code").and_then(|v| v.as_i64()),
        Some(-32700),
        "parse error code must be -32700; got: {error}"
    );
    assert!(
        error.get("message").and_then(|v| v.as_str()).is_some(),
        "JSON-RPC error must carry a `message` string; got: {error}"
    );
}

// ── 3. unknown method → method not found (-32601) ────────────────────────────────────────────

/// A structurally valid JSON-RPC envelope naming a method the server does not implement maps to
/// `code == -32601` (Method not found), echoing the request id.
#[test]
fn unknown_method_returns_method_not_found() {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "definitely_not_a_real_method",
        "params": {}
    });
    let line = format!(
        "{}\n",
        serde_json::to_string(&req).expect("serialize request")
    );

    let output = run_server(&line);
    let resp = single_response(&output);

    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(7),
        "error response must echo the request id; got: {resp}"
    );
    let error = resp
        .get("error")
        .expect("unknown method must produce a JSON-RPC `error` object");
    assert_eq!(
        error.get("code").and_then(|v| v.as_i64()),
        Some(-32601),
        "method-not-found code must be -32601; got: {error}"
    );
}

// ── 4. missing required param → invalid params (-32602) ──────────────────────────────────────

/// A method that REQUIRES params, called with the required param(s) missing, maps to
/// `code == -32602` (Invalid params). `initialize` requires `protocolVersion` in its params;
/// omitting params entirely must be rejected as invalid params (not a panic, not a generic OK).
#[test]
fn missing_required_param_returns_invalid_params() {
    // `initialize` with NO params object at all — the required `protocolVersion` is absent.
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "initialize"
    });
    let line = format!(
        "{}\n",
        serde_json::to_string(&req).expect("serialize request")
    );

    let output = run_server(&line);
    let resp = single_response(&output);

    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(3),
        "error response must echo the request id; got: {resp}"
    );
    let error = resp
        .get("error")
        .expect("missing required param must produce a JSON-RPC `error` object");
    assert_eq!(
        error.get("code").and_then(|v| v.as_i64()),
        Some(-32602),
        "invalid-params code must be -32602; got: {error}"
    );
}

// ── 5. framing discipline: one request → one parseable, \n-terminated response line ──────────

/// Proves the read loop correctly FRAMES a single request and emits a single, line-delimited
/// (`\n`-terminated) response that parses as JSON-RPC 2.0. Asserts on the RAW bytes written: the
/// output is exactly one line, it ends with `\n`, contains no embedded newline before the
/// terminator, and round-trips through `serde_json`. This is the line-delimited framing contract.
#[test]
fn response_is_a_single_newline_terminated_json_line() {
    let output = run_server(&initialize_request_line(42));

    let text = std::str::from_utf8(&output).expect("output must be valid UTF-8");
    assert!(
        text.ends_with('\n'),
        "response must be newline-terminated (line-delimited framing); got: {text:?}"
    );
    // Exactly one line of content (one request ⇒ one response line).
    let content_lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        content_lines.len(),
        1,
        "one request must yield exactly one framed response line; got: {text:?}"
    );
    // The single line has no embedded newline (the only newline is the frame terminator).
    let line = content_lines[0];
    assert!(
        !line.contains('\n'),
        "a framed response line must not contain an embedded newline; got: {line:?}"
    );
    // And it round-trips as a JSON-RPC 2.0 object with the echoed id.
    let value: serde_json::Value =
        serde_json::from_str(line).expect("the framed line must be parseable JSON");
    assert_eq!(value.get("jsonrpc").and_then(|v| v.as_str()), Some("2.0"));
    assert_eq!(value.get("id").and_then(|v| v.as_i64()), Some(42));
}

// ── 6. no panic ever: a batch of malformed/edge lines each yields a structured error ─────────

/// The loop must survive a stream of adversarial lines without unwinding: empty line, blank
/// whitespace, a bare JSON array, a JSON value that is not an object, a truncated object, and a
/// valid-but-unknown method — interleaved with a good `initialize`. Every NON-blank line that the
/// server chooses to answer must produce a parseable JSON-RPC object (never a panic, never a torn
/// half-line). The good `initialize` in the middle must still get a success result, proving the
/// loop recovers after errors.
#[test]
fn malformed_stream_never_panics_and_each_response_is_structured() {
    let mut input = String::new();
    input.push_str("not json\n");
    input.push_str("[1, 2, 3]\n"); // valid JSON, but not a JSON-RPC request object
    input.push_str("12345\n"); // valid JSON scalar, not an object
    input.push_str("{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\"nope\"}\n"); // unknown method
    input.push_str(&initialize_request_line(100)); // a good one, after the bad ones
    input.push_str("{\"jsonrpc\":\"2.0\",\"id\":\n"); // truncated object

    let (server, _tmp) = test_server();
    let reader = Cursor::new(input.into_bytes());
    let mut output: Vec<u8> = Vec::new();
    // Must return Ok at EOF — no panic, no Err — despite the malformed lines.
    serve(reader, &mut output, server).expect("serve must survive a malformed stream and end Ok");

    let text = std::str::from_utf8(&output).expect("output must be valid UTF-8");
    // Every non-blank response line must be independently parseable JSON (no torn frames).
    let mut saw_initialize_result = false;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|_| {
            panic!("every emitted response line must be valid JSON; got: {line:?}")
        });
        assert_eq!(
            value.get("jsonrpc").and_then(|v| v.as_str()),
            Some("2.0"),
            "every response must carry jsonrpc \"2.0\"; got: {line:?}"
        );
        // The good initialize (id 100) must have produced a success result, not an error.
        if value.get("id").and_then(|v| v.as_i64()) == Some(100) {
            assert!(
                value.get("result").is_some() && value.get("error").is_none(),
                "the valid initialize amid the noise must succeed; got: {line:?}"
            );
            saw_initialize_result = true;
        }
    }
    assert!(
        saw_initialize_result,
        "the loop must recover after malformed lines and still answer the good initialize; output: {text:?}"
    );
}

// ── 6b. JSON-RPC notifications (no `id`) get NO response (spec: a server MUST NOT reply) ──────

/// Per JSON-RPC 2.0 a *Notification* is a request object with NO `id` member, and the server MUST
/// NOT send back any response — neither a success nor an error. The MCP `notifications/initialized`
/// message a client (e.g. Claude Code) sends right after the handshake is exactly such a
/// notification: the server must silently ignore it. (Previously the server wrongly answered it with
/// a `-32601` "method not found" error and `id: null`, which breaks strict client checks.)
#[test]
fn notification_initialized_gets_no_response() {
    let output = run_server("{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n");
    assert!(
        output.is_empty(),
        "a notification (a request with no `id`) must produce NO response frame; got: {:?}",
        std::str::from_utf8(&output)
    );
}

/// The discriminator is the ABSENCE of `id`, not the method name: a notification with an unknown
/// method is dropped too (no `-32601`). And a notification interleaved with real requests must not
/// desync the stream — only the two id-bearing requests are answered, in their original order.
#[test]
fn notifications_are_silently_dropped_amid_real_requests() {
    let mut input = String::new();
    input.push_str(&initialize_request_line(1));
    input.push_str("{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n"); // notification
    input.push_str("{\"jsonrpc\":\"2.0\",\"method\":\"some/unknown/notification\"}\n"); // unknown-method notification
    input.push_str(&tools_list_request_line(2));

    let output = run_server(&input);
    let text = std::str::from_utf8(&output).expect("output must be valid UTF-8");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        2,
        "exactly the two id-bearing requests must be answered; the two notifications emit no \
         frame; got: {text:?}"
    );
    let ids: Vec<i64> = lines
        .iter()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .expect("each response line must be valid JSON")
                .get("id")
                .and_then(|v| v.as_i64())
                .expect("each answered response must echo a numeric id")
        })
        .collect();
    assert_eq!(
        ids,
        vec![1, 2],
        "the answered responses must be initialize(1) then tools/list(2), in order; got: {text:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// M8.2 — `tools/list` returns all three MCP tools with the exact §8.2 inputSchemas (D13).
//
// These tests pin the D13 tool-registration contract an MCP client (Claude Code) consumes. A
// `tools/list` request is `{ "jsonrpc":"2.0", "id":N, "method":"tools/list" }` (params optional per
// MCP); the response `result` carries a `tools` array of `{ name, description, inputSchema }`
// objects. The schemas are asserted PRECISELY against `docs/project_plan.md` §8.2.
//
// PINNED M8.2 DECISIONS (the eng-lead must honor these — the tests are the contract):
//   - The three tool `name`s are EXACTLY {`codecache_search`, `codecache_update`,
//     `codecache_outline`}. Each tool carries a non-empty `description` and an `inputSchema`
//     object whose `type` is `"object"`.
//   - Property `type`s and `default`s are asserted against §8.2 verbatim. JSON Schema `default`
//     values are emitted as JSON values of the property's own type: `max_tokens` defaults are JSON
//     NUMBERS (integers `4000` / `2000`, NOT strings); `file_filter`'s default is JSON `null`.
//   - `required` arrays are exact: search → `["query"]`, update → `["files"]`,
//     outline → `["path"]`.
//   - TOOL ORDERING is fixed: the eng-lead MUST emit the tools in the stable order
//     [`codecache_search`, `codecache_update`, `codecache_outline`] so a client sees a
//     deterministic list across calls. `tools_list_tool_order_is_stable_and_deterministic`
//     asserts this order and asserts it is identical across two `tools/list` calls.
//
// Scope: M8.2 only lists the tools. It does NOT exercise tools/call execution (M8.3).
// These tests fail now because the server returns -32601 (method not found) for `tools/list`.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// A well-formed JSON-RPC 2.0 `tools/list` request line (newline-framed), params omitted (optional
/// per MCP). Mirrors what an MCP client sends to enumerate the server's tools.
fn tools_list_request_line(id: i64) -> String {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/list"
    });
    format!(
        "{}\n",
        serde_json::to_string(&req).expect("serialize request")
    )
}

/// Drive the server with a single `tools/list` request and return the parsed response value.
fn tools_list(id: i64) -> serde_json::Value {
    let output = run_server(&tools_list_request_line(id));
    single_response(&output)
}

/// Extract the `result.tools` array from a `tools/list` response, asserting the envelope shape
/// (`jsonrpc` 2.0, no `error`, a `result` carrying a `tools` array).
fn tools_array(resp: &serde_json::Value) -> Vec<serde_json::Value> {
    assert_eq!(
        resp.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "tools/list response must carry jsonrpc \"2.0\"; got: {resp}"
    );
    assert!(
        resp.get("error").is_none(),
        "a well-formed tools/list must NOT produce an error object; got: {resp}"
    );
    let result = resp
        .get("result")
        .expect("tools/list response must carry a `result`");
    result
        .get("tools")
        .and_then(|t| t.as_array())
        .unwrap_or_else(|| panic!("tools/list result must carry a `tools` array; got: {result}"))
        .clone()
}

/// Find the tool object named `name` in a `tools/list` response (asserting it is present).
fn find_tool(resp: &serde_json::Value, name: &str) -> serde_json::Value {
    tools_array(resp)
        .into_iter()
        .find(|t| t.get("name").and_then(|v| v.as_str()) == Some(name))
        .unwrap_or_else(|| panic!("tools/list must include a tool named {name:?}; got: {resp}"))
}

/// The `inputSchema.properties` map of a tool, asserting `inputSchema` is an object of
/// `type: "object"` carrying a `properties` object.
fn input_schema_properties(tool: &serde_json::Value) -> serde_json::Value {
    let schema = tool
        .get("inputSchema")
        .expect("tool must carry an `inputSchema`");
    assert_eq!(
        schema.get("type").and_then(|v| v.as_str()),
        Some("object"),
        "inputSchema.type must be \"object\"; got: {schema}"
    );
    schema
        .get("properties")
        .filter(|p| p.is_object())
        .cloned()
        .unwrap_or_else(|| panic!("inputSchema must carry a `properties` object; got: {schema}"))
}

/// The `inputSchema.required` array of a tool as a `Vec<String>` (asserting it is a string array).
fn input_schema_required(tool: &serde_json::Value) -> Vec<String> {
    let schema = tool
        .get("inputSchema")
        .expect("tool must carry an `inputSchema`");
    schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    v.as_str()
                        .unwrap_or_else(|| panic!("required entries must be strings; got: {v}"))
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_else(|| panic!("inputSchema must carry a `required` array; got: {schema}"))
}

// ── 7. tools/list lists exactly the three D13 tools ──────────────────────────────────────────

/// `tools/list` returns a `result.tools` array of length 3 whose names are EXACTLY the D13 set
/// {`codecache_search`, `codecache_update`, `codecache_outline`}. Each tool carries a non-empty
/// `description` and an `inputSchema` object of `type: "object"`. Echoes id, jsonrpc 2.0.
#[test]
fn tools_list_returns_all_three_tools() {
    let resp = tools_list(1);

    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(1),
        "tools/list response must echo the request id; got: {resp}"
    );

    let tools = tools_array(&resp);
    assert_eq!(
        tools.len(),
        3,
        "tools/list must return exactly 3 tools (D13); got: {resp}"
    );

    let mut names: Vec<&str> = tools
        .iter()
        .map(|t| {
            t.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("every tool must carry a string `name`; got: {t}"))
        })
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["codecache_outline", "codecache_search", "codecache_update"],
        "the tool name set must be exactly the three D13 tools; got: {resp}"
    );

    for tool in &tools {
        let name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("<none>");
        assert!(
            tool.get("description")
                .and_then(|v| v.as_str())
                .map(|d| !d.is_empty())
                .unwrap_or(false),
            "tool {name:?} must carry a non-empty `description`; got: {tool}"
        );
        assert_eq!(
            tool.get("inputSchema")
                .and_then(|s| s.get("type"))
                .and_then(|v| v.as_str()),
            Some("object"),
            "tool {name:?} must carry an inputSchema of type \"object\"; got: {tool}"
        );
    }
}

// ── 8. codecache_search inputSchema matches §8.2 exactly ──────────────────────────────────────

/// `codecache_search` inputSchema (§8.2): `properties.query` is `type: string`;
/// `properties.max_tokens` is `type: integer` with `default: 4000` (JSON number);
/// `properties.file_filter` is `type: string` with `default: null`; `required == ["query"]`.
#[test]
fn tools_list_includes_codecache_search_with_input_schema() {
    let resp = tools_list(2);
    let tool = find_tool(&resp, "codecache_search");
    let props = input_schema_properties(&tool);

    // query: string, required.
    let query = props
        .get("query")
        .expect("codecache_search.inputSchema.properties must include `query`");
    assert_eq!(
        query.get("type").and_then(|v| v.as_str()),
        Some("string"),
        "search `query` must be type string; got: {query}"
    );

    // max_tokens: integer, default 4000 (a JSON number, not a string).
    let max_tokens = props
        .get("max_tokens")
        .expect("codecache_search.inputSchema.properties must include `max_tokens`");
    assert_eq!(
        max_tokens.get("type").and_then(|v| v.as_str()),
        Some("integer"),
        "search `max_tokens` must be type integer; got: {max_tokens}"
    );
    assert_eq!(
        max_tokens.get("default").and_then(|v| v.as_i64()),
        Some(4000),
        "search `max_tokens` default must be the JSON number 4000; got: {max_tokens}"
    );
    assert!(
        max_tokens
            .get("default")
            .map(|v| v.is_number())
            .unwrap_or(false),
        "search `max_tokens` default must be a JSON number, not a string; got: {max_tokens}"
    );

    // file_filter: string, default null.
    let file_filter = props
        .get("file_filter")
        .expect("codecache_search.inputSchema.properties must include `file_filter`");
    assert_eq!(
        file_filter.get("type").and_then(|v| v.as_str()),
        Some("string"),
        "search `file_filter` must be type string; got: {file_filter}"
    );
    assert!(
        file_filter
            .get("default")
            .map(|v| v.is_null())
            .unwrap_or(false),
        "search `file_filter` default must be JSON null; got: {file_filter}"
    );

    // required is exactly ["query"].
    assert_eq!(
        input_schema_required(&tool),
        vec!["query".to_string()],
        "codecache_search required must be exactly [\"query\"]; got: {tool}"
    );
}

// ── 9. codecache_update inputSchema matches §8.2 exactly ──────────────────────────────────────

/// `codecache_update` inputSchema (§8.2): `properties.files` is `type: array` with
/// `items.type == "string"`; `required == ["files"]`.
#[test]
fn tools_list_includes_codecache_update_with_input_schema() {
    let resp = tools_list(3);
    let tool = find_tool(&resp, "codecache_update");
    let props = input_schema_properties(&tool);

    let files = props
        .get("files")
        .expect("codecache_update.inputSchema.properties must include `files`");
    assert_eq!(
        files.get("type").and_then(|v| v.as_str()),
        Some("array"),
        "update `files` must be type array; got: {files}"
    );
    assert_eq!(
        files
            .get("items")
            .and_then(|i| i.get("type"))
            .and_then(|v| v.as_str()),
        Some("string"),
        "update `files.items.type` must be \"string\"; got: {files}"
    );

    assert_eq!(
        input_schema_required(&tool),
        vec!["files".to_string()],
        "codecache_update required must be exactly [\"files\"]; got: {tool}"
    );
}

// ── 10. codecache_outline inputSchema matches §8.2 exactly (D13) ──────────────────────────────

/// `codecache_outline` inputSchema (§8.2 / D13): `properties.path` is `type: string`;
/// `properties.max_tokens` is `type: integer` with `default: 2000` (JSON number);
/// `required == ["path"]`.
#[test]
fn tools_list_includes_codecache_outline_with_input_schema() {
    let resp = tools_list(4);
    let tool = find_tool(&resp, "codecache_outline");
    let props = input_schema_properties(&tool);

    // path: string, required.
    let path = props
        .get("path")
        .expect("codecache_outline.inputSchema.properties must include `path`");
    assert_eq!(
        path.get("type").and_then(|v| v.as_str()),
        Some("string"),
        "outline `path` must be type string; got: {path}"
    );

    // max_tokens: integer, default 2000 (a JSON number, not a string).
    let max_tokens = props
        .get("max_tokens")
        .expect("codecache_outline.inputSchema.properties must include `max_tokens`");
    assert_eq!(
        max_tokens.get("type").and_then(|v| v.as_str()),
        Some("integer"),
        "outline `max_tokens` must be type integer; got: {max_tokens}"
    );
    assert_eq!(
        max_tokens.get("default").and_then(|v| v.as_i64()),
        Some(2000),
        "outline `max_tokens` default must be the JSON number 2000; got: {max_tokens}"
    );
    assert!(
        max_tokens
            .get("default")
            .map(|v| v.is_number())
            .unwrap_or(false),
        "outline `max_tokens` default must be a JSON number, not a string; got: {max_tokens}"
    );

    // required is exactly ["path"].
    assert_eq!(
        input_schema_required(&tool),
        vec!["path".to_string()],
        "codecache_outline required must be exactly [\"path\"]; got: {tool}"
    );
}

// ── 11. determinism: id echoed, jsonrpc 2.0, stable tool order across calls ───────────────────

/// `tools/list` is deterministic: the response echoes the request id, carries `jsonrpc: "2.0"`,
/// and the tools are emitted in a FIXED order [search, update, outline] that is identical across
/// two separate calls. (Pins the D13 contract: the eng-lead must emit a stable order so a client
/// sees a deterministic list.)
#[test]
fn tools_list_tool_order_is_stable_and_deterministic() {
    let resp_a = tools_list(11);
    assert_eq!(
        resp_a.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "tools/list must carry jsonrpc \"2.0\"; got: {resp_a}"
    );
    assert_eq!(
        resp_a.get("id").and_then(|v| v.as_i64()),
        Some(11),
        "tools/list must echo the request id; got: {resp_a}"
    );

    let order_of = |resp: &serde_json::Value| -> Vec<String> {
        tools_array(resp)
            .iter()
            .map(|t| {
                t.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<none>")
                    .to_string()
            })
            .collect()
    };

    let expected = vec![
        "codecache_search".to_string(),
        "codecache_update".to_string(),
        "codecache_outline".to_string(),
    ];
    assert_eq!(
        order_of(&resp_a),
        expected,
        "tools must be emitted in the fixed order [search, update, outline]; got: {resp_a}"
    );

    // Second call returns the same order — deterministic across invocations.
    let resp_b = tools_list(12);
    assert_eq!(
        order_of(&resp_b),
        order_of(&resp_a),
        "tool order must be identical across two tools/list calls (determinism)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// M8.3 — `tools/call` round-trip: search / update / outline (RED).
//
// A `tools/call` request is `{ jsonrpc, id, method:"tools/call", params:{ name, arguments } }`.
// On success the `result` carries the MCP content envelope `{ content: [ { type:"text",
// text:"<...>" } ] }` (project_plan §8.2 example responses). Bad arguments (missing required arg,
// wrong type) AND an unknown tool name → JSON-RPC error `-32602` (invalid params).
//
// PINNED M8.3 DECISIONS (the eng-lead must honor — the tests are the contract):
//   1. Response envelope on success: `result.content` is a non-empty ARRAY whose first element is
//      `{ "type":"text", "text":<string> }`. The text payload is what the agent consumes.
//   2. `codecache_search` text REUSES the §6.4.3 text formatter (D13 agent-first): it carries the
//      `Query: "<q>"` header, a `Found N results` line, and a `[n] <symbol> (<type>) file:s-e`
//      locator block per hit. The tests assert on STABLE SUBSTRINGS (seeded symbol name, the
//      `file:start-end` locator, the `Query:`/`Found` header lines) — not full-string equality —
//      so the eng-lead keeps latitude on exact wording while the agent-first shape is pinned.
//   3. `codecache_update` text reports stats mirroring §8.3: it MUST contain the count of files
//      processed and chunks indexed (substring `"1 file"` and `"chunk"`). The update test writes a
//      REAL file on disk and indexes it first (the server's Indexer re-indexes from disk).
//   4. `codecache_outline` text is a symbol SKELETON reusing the D13 text skeleton-line shape: one
//      line per symbol carrying the symbol name and its `file:start-end` range. A FILE path lists
//      that file's symbols; a DIRECTORY path lists every file's symbols under it.
//   5. ERROR MAPPING — pinned: a `tools/call` with a missing/wrong-typed required argument → `-32602`
//      (invalid params). An UNKNOWN TOOL NAME in `tools/call` → ALSO `-32602` (NOT `-32601`): per the
//      MCP convention the tool `name` is a *param* of the `tools/call` method, so a bad name is an
//      invalid param, not a missing JSON-RPC method. (`-32601` stays reserved for an unknown
//      top-level JSON-RPC `method`, e.g. M8.1's `unknown_method_returns_method_not_found`.)
//
// These tests fail now because `tools/call` is unhandled — `dispatch`'s default arm returns
// `-32601 method not found: tools/call`. The GREEN target: a `"tools/call"` dispatch arm that
// routes `params.name` to per-tool handlers, with `CodeCacheServer` now holding/lazily building a
// `Retriever` + `Indexer` over its shared `Storage` (D8).
// ═══════════════════════════════════════════════════════════════════════════════════════════════

use std::path::PathBuf;

use codecache::types::{Chunk, Language, SymbolType};

// ── M8.3 harness: seeding + tools/call drivers ───────────────────────────────────────────────

/// Build a `Chunk` for direct seeding (mirrors `tests/retriever_tests.rs`). `start_byte`/`end_byte`
/// are derived from `body` so dedup never collapses distinct symbols; line range is explicit so the
/// outline locator (`file:start-end`) is predictable.
#[allow(clippy::too_many_arguments)]
fn seed_chunk(
    file: &str,
    name: &str,
    symbol_type: SymbolType,
    parent: Option<&str>,
    body: &str,
    start_line: usize,
    end_line: usize,
) -> Chunk {
    Chunk {
        symbol_name: name.to_string(),
        symbol_type,
        file_path: PathBuf::from(file),
        start_byte: 0,
        end_byte: body.len(),
        start_line,
        end_line,
        chunk_text: body.to_string(),
        language: Language::Python,
        parent_symbol: parent.map(str::to_string),
        file_docstring: None,
        imports: Vec::new(),
        cross_references: Vec::new(),
        is_heuristic: false,
    }
}

/// Build a `CodeCacheServer` over a temp DB seeded with `chunks` (via `Storage::insert_chunks`),
/// returning the server + the live temp dir (keep it alive for the test). Unlike `test_server`
/// (empty DB), this lets the M8.3 search/outline round-trips actually retrieve something.
fn test_server_seeded(chunks: &[Chunk]) -> (CodeCacheServer, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp.path().join("index.db");
    let storage = Storage::new(&db_path).expect("open storage");
    storage.init_schema().expect("init schema");
    storage.insert_chunks(chunks).expect("seed chunks");
    (CodeCacheServer::new(storage), tmp)
}

/// A `tools/call` request line (newline-framed) naming `tool` with the given `arguments` object.
fn tools_call_request_line(id: i64, tool: &str, arguments: serde_json::Value) -> String {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": tool, "arguments": arguments }
    });
    format!(
        "{}\n",
        serde_json::to_string(&req).expect("serialize request")
    )
}

/// Drive `server` with a single `tools/call` request and return the parsed response value.
fn tools_call(
    server: CodeCacheServer,
    id: i64,
    tool: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let input = tools_call_request_line(id, tool, arguments);
    let reader = Cursor::new(input.into_bytes());
    let mut output: Vec<u8> = Vec::new();
    serve(reader, &mut output, server).expect("serve loop returns Ok at clean EOF");
    single_response(&output)
}

/// Assert the success envelope shape and return the `result.content[0].text` payload string. Pins
/// `result.content` = non-empty array whose first element is `{ type:"text", text:<string> }`.
fn call_result_text(resp: &serde_json::Value) -> String {
    assert_eq!(
        resp.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "tools/call response must carry jsonrpc \"2.0\"; got: {resp}"
    );
    assert!(
        resp.get("error").is_none(),
        "a successful tools/call must NOT carry an error; got: {resp}"
    );
    let result = resp
        .get("result")
        .expect("successful tools/call must carry a `result`");
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .unwrap_or_else(|| panic!("result.content must be an array; got: {result}"));
    assert!(
        !content.is_empty(),
        "result.content must be non-empty; got: {result}"
    );
    let first = &content[0];
    assert_eq!(
        first.get("type").and_then(|v| v.as_str()),
        Some("text"),
        "result.content[0].type must be \"text\"; got: {first}"
    );
    first
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("result.content[0].text must be a string; got: {first}"))
        .to_string()
}

/// Assert the response is a JSON-RPC error with `code`, echoing `id`.
fn assert_error_code(resp: &serde_json::Value, expected_code: i64, expected_id: i64) {
    assert_eq!(
        resp.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "error response must carry jsonrpc \"2.0\"; got: {resp}"
    );
    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(expected_id),
        "error response must echo the request id; got: {resp}"
    );
    let error = resp
        .get("error")
        .unwrap_or_else(|| panic!("expected a JSON-RPC error object; got: {resp}"));
    assert_eq!(
        error.get("code").and_then(|v| v.as_i64()),
        Some(expected_code),
        "error code must be {expected_code}; got: {error}"
    );
    assert!(
        error.get("message").and_then(|v| v.as_str()).is_some(),
        "JSON-RPC error must carry a `message` string; got: {error}"
    );
}

// ── 12. codecache_search round-trip → §6.4.3 agent-first text ────────────────────────────────

/// `tools/call codecache_search {query}` over a seeded index returns the success envelope
/// `result.content[0]{type:"text", text}`; the text reflects the §6.4.3 formatter — it names the
/// seeded symbol, carries the `file:start-end` locator, and the agent-first header (`Query:` echo +
/// `Found` count). Asserts on stable substrings, not full-string equality.
#[test]
fn call_codecache_search_returns_formatted_results() {
    let (server, _tmp) = test_server_seeded(&[
        seed_chunk(
            "src/auth.py",
            "authenticate_user",
            SymbolType::Function,
            None,
            "def authenticate_user(username, password):\n    return verify(username, password)",
            45,
            67,
        ),
        seed_chunk(
            "src/math.py",
            "compute_factorial",
            SymbolType::Function,
            None,
            "def compute_factorial(n):\n    return product(range(1, n + 1))",
            1,
            5,
        ),
    ]);

    let resp = tools_call(
        server,
        12,
        "codecache_search",
        serde_json::json!({ "query": "authenticate user" }),
    );
    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(12),
        "search response must echo the request id; got: {resp}"
    );

    let text = call_result_text(&resp);
    assert!(
        text.contains("authenticate_user"),
        "search text must name the seeded matching symbol; got: {text:?}"
    );
    assert!(
        text.contains("src/auth.py:45-67"),
        "search text must carry the agent-first file:start-end locator; got: {text:?}"
    );
    // Agent-first header (§6.4.3): query echo + a result count line precede the bodies.
    assert!(
        text.contains("Query:") && text.contains("authenticate user"),
        "search text must echo the query in its header; got: {text:?}"
    );
    assert!(
        text.contains("Found"),
        "search text must carry the `Found N results` header line; got: {text:?}"
    );
}

// ── 13. codecache_update round-trip → re-index + stats ───────────────────────────────────────

/// `tools/call codecache_update {files}` re-indexes a REAL on-disk file and returns a text result
/// reporting the stats (§8.3 "Updated N files, indexed M chunks"). The file is created and indexed
/// first so the server's Indexer has a project root + DB to update; the update then re-indexes it.
#[test]
fn call_codecache_update_reindexes_and_reports_stats() {
    // Build a real, initialized + indexed project on disk; the server points at its DB.
    let tmp = tempfile::tempdir().expect("temp project dir");
    let root = tmp.path();
    codecache::init(root).expect("init project");
    let rel = "mod.py";
    std::fs::write(
        root.join(rel),
        "def alpha():\n    return 1\n\ndef beta():\n    return 2\n",
    )
    .expect("write source file");
    codecache::index(root).expect("initial index");

    // The server runs over the same on-disk DB so its Indexer re-indexes from the project root.
    let db_path = root.join(".codecache").join("index.db");
    let storage = Storage::new(&db_path).expect("open indexed db");
    let server = CodeCacheServer::new(storage);

    let abs = root.join(rel);
    let resp = tools_call(
        server,
        13,
        "codecache_update",
        serde_json::json!({ "files": [abs.to_string_lossy()] }),
    );
    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(13),
        "update response must echo the request id; got: {resp}"
    );

    let text = call_result_text(&resp);
    // §8.3: "Updated {files_processed} files, indexed {chunks_indexed} chunks ...". The single
    // updated file and its chunk count must be reported. Assert on stable substrings.
    assert!(
        text.contains("1 file"),
        "update text must report one file processed; got: {text:?}"
    );
    assert!(
        text.contains("chunk"),
        "update text must report the chunk count; got: {text:?}"
    );
}

// ── 14. codecache_outline round-trip → symbol skeleton (file + directory) ────────────────────

/// `tools/call codecache_outline {path}` returns a symbol skeleton: one line per symbol carrying
/// the symbol name and its `file:start-end` range (D13 skeleton-line shape). A FILE path lists that
/// file's symbols; a DIRECTORY path lists symbols across every file under it.
#[test]
fn call_codecache_outline_returns_symbol_skeleton() {
    let chunks = [
        seed_chunk(
            "src/a.py",
            "Greeter",
            SymbolType::Class,
            None,
            "class Greeter:\n    ...",
            1,
            20,
        ),
        seed_chunk(
            "src/a.py",
            "greet",
            SymbolType::Method,
            Some("Greeter"),
            "def greet(self):\n    ...",
            5,
            12,
        ),
        seed_chunk(
            "src/sub/b.py",
            "helper",
            SymbolType::Function,
            None,
            "def helper():\n    ...",
            1,
            4,
        ),
    ];

    // (a) A FILE path lists exactly that file's symbols with their ranges.
    let (server, _tmp) = test_server_seeded(&chunks);
    let resp = tools_call(
        server,
        14,
        "codecache_outline",
        serde_json::json!({ "path": "src/a.py" }),
    );
    let text = call_result_text(&resp);
    assert!(
        text.contains("Greeter"),
        "outline must list the class symbol; got: {text:?}"
    );
    assert!(
        text.contains("greet"),
        "outline must list the method symbol; got: {text:?}"
    );
    assert!(
        text.contains("src/a.py:1-20"),
        "outline lines carry the symbol's file:start-end range; got: {text:?}"
    );
    assert!(
        !text.contains("src/sub/b.py"),
        "a FILE outline must not include symbols from another file; got: {text:?}"
    );

    // (b) A DIRECTORY path lists symbols across every file under it.
    let (server_dir, _tmp_dir) = test_server_seeded(&chunks);
    let resp_dir = tools_call(
        server_dir,
        15,
        "codecache_outline",
        serde_json::json!({ "path": "src" }),
    );
    let text_dir = call_result_text(&resp_dir);
    assert!(
        text_dir.contains("Greeter") && text_dir.contains("helper"),
        "a DIRECTORY outline must span every file under it; got: {text_dir:?}"
    );
    assert!(
        text_dir.contains("src/a.py:1-20") && text_dir.contains("src/sub/b.py:1-4"),
        "a DIRECTORY outline carries each symbol's file:start-end range; got: {text_dir:?}"
    );
}

// ── 15. bad arguments / unknown tool → invalid params (-32602) ───────────────────────────────

/// A `tools/call` with a missing required argument, or naming an unknown tool, maps to `-32602`
/// (invalid params), echoing the request id. Pins decision #5: an unknown tool NAME is an invalid
/// *param* of `tools/call` (-32602), NOT an unknown JSON-RPC method (-32601).
#[test]
fn call_with_bad_arguments_returns_invalid_params() {
    // (a) codecache_search with no `query` (required) → -32602.
    let (server, _tmp) = test_server_seeded(&[]);
    let resp = tools_call(server, 20, "codecache_search", serde_json::json!({}));
    assert_error_code(&resp, -32602, 20);

    // (b) codecache_outline with no `path` (required) → -32602.
    let (server, _tmp) = test_server_seeded(&[]);
    let resp = tools_call(server, 21, "codecache_outline", serde_json::json!({}));
    assert_error_code(&resp, -32602, 21);

    // (c) codecache_update with no `files` (required) → -32602.
    let (server, _tmp) = test_server_seeded(&[]);
    let resp = tools_call(server, 22, "codecache_update", serde_json::json!({}));
    assert_error_code(&resp, -32602, 22);

    // (d) an UNKNOWN tool name → -32602 (invalid param `name`), NOT -32601.
    let (server, _tmp) = test_server_seeded(&[]);
    let resp = tools_call(
        server,
        23,
        "codecache_not_a_real_tool",
        serde_json::json!({ "query": "x" }),
    );
    assert_error_code(&resp, -32602, 23);
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// M8.4 — D14 self-healing search (RED).
//
// Before answering, `codecache_search` hash-checks the files implicated by the top results
// (project_plan §8.2 "Self-healing search (D14)" + ROADMAP D14), transparently RE-INDEXES the ones
// whose on-disk content changed since indexing, DROPS results whose file was DELETED on disk (and
// evicts its now-stale chunks so a second search never returns them), then RE-RUNS the query ONCE
// and formats THAT fresh result. A clean (unchanged) result set ⇒ NO re-index writes. The self-heal
// is BOUNDED to the files surfaced by the first query (cost ∝ result count, overview §5.2), not the
// whole index.
//
// These tests drive the REAL self-healing path: the index must reflect a file's ORIGINAL bytes, the
// test then mutates/deletes that file behind the index's back, and the assertion proves the server
// healed before answering. So they seed through `codecache::init` + `codecache::index` against a
// REAL on-disk temp project (giving `files_metadata` true content+mtime hashes via §4.4) and build
// the server over THAT db — the stored `file_path`s are absolute-under-root, so the handler can
// re-hash + re-index them straight from disk.
//
// ── PINNED M8.4 DECISIONS (the eng-lead must honor — these tests are the contract) ───────────────
//
//   1. ALGORITHM (`handle_search`): (a) run the query once → collect the DISTINCT `file_path`s of the
//      results; (b) for each implicated file, `hasher::is_changed(path, Storage::get_file_hash(path))`
//      — CHANGED & still on disk ⇒ `Indexer::update_files(&[path])` (transparent re-index); DELETED on
//      disk (`compute_file_hash` errors / path missing) ⇒ `delete_chunks_for_file` + `delete_file_meta`
//      (EVICT, never panic); UNCHANGED ⇒ leave untouched (NO write); (c) re-run the query ONCE and
//      format that fresh `QueryResult` with the §6.4.3 text formatter. The healing is bounded to the
//      first query's result files only.
//
//   2. STALENESS-WINDOW METRIC HOOK (overview §5.2 Layer 3). The slice exposes a small, observable
//      counter set for the LAST search, reachable AFTER the server is moved into `serve`. Pinned shape:
//
//          // src/mcp_server/mod.rs (or a submodule, re-exported)
//          #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
//          pub struct StalenessStats {
//              pub files_checked: usize,    // distinct result files hash-checked this search
//              pub files_reindexed: usize,  // changed-on-disk files transparently re-indexed
//              pub files_dropped: usize,    // deleted-on-disk files evicted + dropped from results
//          }
//
//      `CodeCacheServer` exposes a CHEAP, shared handle grabbed BEFORE the `serve` move (so the
//      `serve(reader, writer, server)` signature is UNCHANGED — do not touch it):
//
//          impl CodeCacheServer { pub fn staleness_handle(&self) -> StalenessHandle; }
//          impl StalenessHandle { pub fn last(&self) -> StalenessStats; }
//
//      i.e. `StalenessHandle` wraps a `Clone` (e.g. `Arc<Mutex<StalenessStats>>`) the server writes
//      at the end of each `handle_search`; tests clone the handle, run a search through `serve`, then
//      read `handle.last()`. (Any equivalent SMALL surface that lets a test observe checked/reindexed/
//      dropped counts after the search is acceptable, but match these field names — the tests pin them.)
//      Non-search tool calls leave the stats at their previous value; a fresh server reads `Default`
//      (all zero).
//
//   3. NO-WRITE OBSERVABLE (test 2). A clean search must not re-index any result file. Pinned two ways
//      (both asserted): (a) the metric hook reports `files_reindexed == 0`; (b) the stored
//      `Storage::get_file_hash(path)` for each result file is BYTE-IDENTICAL across the search (an
//      unnecessary re-index would re-stamp it — §4.4 hashes content+mtime). `files_checked` MAY be > 0
//      (the files were hash-checked — that is the cheap read), but NO write occurred.
//
// These fail now because `handle_search` (src/mcp_server/handlers.rs) does a single `Retriever::query`
// with no hash-check / re-index / eviction, and neither `StalenessStats`/`StalenessHandle` nor
// `staleness_handle()` exist — so tests 1–4 fail to compile (handle hook) and, once stubbed, assert the
// stale/un-healed values. That is the RED state.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

use codecache::mcp_server::StalenessStats;

/// Build a server over a REAL on-disk project: `init`, write `files` (relative path → LF content),
/// then `index`, then open the indexed DB and wrap it in a `CodeCacheServer`. Returns the server,
/// the live temp dir (KEEP it alive, dropping it deletes the fixture), and the project root so a
/// test can mutate the very files the index was built from. The stored `file_path`s are
/// absolute-under-root, so the self-healing handler re-hashes/re-indexes them straight from disk.
fn test_server_on_disk(files: &[(&str, &str)]) -> (CodeCacheServer, tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("create temp project dir");
    let root = tmp.path().to_path_buf();
    codecache::init(&root).expect("init project");
    for (rel, content) in files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create source subdir");
        }
        std::fs::write(&path, content).expect("write source file");
    }
    codecache::index(&root).expect("initial index");

    let db_path = root.join(".codecache").join("index.db");
    let storage = Storage::new(&db_path).expect("open indexed db");
    (CodeCacheServer::new(storage), tmp, root)
}

/// Drive `server` (consumed) with one `codecache_search` request and return BOTH the parsed
/// response and the staleness stats the server recorded for that search. Grabs the metric handle
/// BEFORE moving the server into `serve` (the `serve` signature is unchanged), then reads it after.
fn search_with_stats(
    server: CodeCacheServer,
    id: i64,
    query: &str,
) -> (serde_json::Value, StalenessStats) {
    let handle = server.staleness_handle();
    let resp = tools_call(
        server,
        id,
        "codecache_search",
        serde_json::json!({ "query": query }),
    );
    (resp, handle.last())
}

// ── 16. THE headline D14 test: a file edited behind the index returns FRESH content ──────────────

/// `codecache_search` heals before answering: index a file whose matching chunk says X, EDIT the
/// file on disk so that chunk now says X' (a renamed body symbol) WITHOUT re-indexing, then search a
/// term that matches the file → the response reflects the EDITED content (X'), proving a transparent
/// re-index-before-answer. The stale token (X) must NOT survive; the metric reports one re-index.
#[test]
fn search_after_file_edit_returns_fresh_content() {
    // ORIGINAL: the matching chunk body references `legacy_password_check`.
    let (server, _tmp, root) = test_server_on_disk(&[(
        "auth.py",
        "def authenticate_user(username, password):\n    return legacy_password_check(username, password)\n",
    )]);

    // EDIT behind the index's back: same function name (so the query still surfaces it) but the body
    // now references `argon2_verify`. mtime/content change ⇒ the stored §4.4 hash no longer matches.
    std::fs::write(
        root.join("auth.py"),
        "def authenticate_user(username, password):\n    return argon2_verify(username, password)\n",
    )
    .expect("edit source file on disk");

    let (resp, stats) = search_with_stats(server, 30, "authenticate user");
    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(30),
        "search response must echo the request id; got: {resp}"
    );

    let text = call_result_text(&resp);
    assert!(
        text.contains("argon2_verify"),
        "self-heal must re-index the edited file and return its FRESH body; got: {text:?}"
    );
    assert!(
        !text.contains("legacy_password_check"),
        "the STALE pre-edit body must NOT survive a self-healing search; got: {text:?}"
    );
    assert_eq!(
        stats.files_reindexed, 1,
        "exactly the one edited result file must be transparently re-indexed; got: {stats:?}"
    );
}

// ── 17. clean (unchanged) result files ⇒ NO re-index writes ───────────────────────────────────────

/// A search over an UNCHANGED on-disk index re-indexes NOTHING. Pinned two ways: the metric reports
/// `files_reindexed == 0`, AND every result file's stored §4.4 content hash is byte-identical across
/// the search (an unnecessary re-index would re-stamp it). The files MAY be hash-checked
/// (`files_checked` is the cheap read) but never re-written.
#[test]
fn search_on_unchanged_files_does_no_reindex_writes() {
    let (server, _tmp, root) = test_server_on_disk(&[
        (
            "auth.py",
            "def authenticate_user(username, password):\n    return verify(username, password)\n",
        ),
        (
            "math.py",
            "def compute_factorial(n):\n    return product(range(1, n + 1))\n",
        ),
    ]);

    // Snapshot the stored hashes of the files the query will surface BEFORE the search. We read them
    // off a sibling Storage handle over the same on-disk DB (D8: cheap clone over one connection).
    let db_path = root.join(".codecache").join("index.db");
    let probe = Storage::new(&db_path).expect("open indexed db for probing");
    let auth_abs = root.join("auth.py");
    let hash_before = probe
        .get_file_hash(&auth_abs)
        .expect("read stored hash")
        .expect("auth.py must have a stored hash after indexing");

    let (resp, stats) = search_with_stats(server, 31, "authenticate user");
    // The search still succeeds and surfaces the (unchanged) file.
    let text = call_result_text(&resp);
    assert!(
        text.contains("authenticate_user"),
        "an unchanged search must still return the matching symbol; got: {text:?}"
    );

    // (a) metric observable: nothing re-indexed.
    assert_eq!(
        stats.files_reindexed, 0,
        "a clean (unchanged) result set must trigger NO re-index; got: {stats:?}"
    );

    // (b) storage observable: the result file's stored hash is byte-identical across the search — a
    // spurious re-index would re-stamp it (§4.4 hashes content+mtime).
    let hash_after = probe
        .get_file_hash(&auth_abs)
        .expect("read stored hash")
        .expect("auth.py must still have a stored hash after a clean search");
    assert_eq!(
        hash_before, hash_after,
        "a clean search must not re-stamp the stored hash (no spurious re-index)"
    );
}

// ── 18. a result file deleted on disk is dropped + its stale chunks evicted ───────────────────────

/// `codecache_search` for a term whose only match lives in a file DELETED from disk drops that file
/// from the results WITHOUT panicking/erroring (a clean, possibly-empty result), and EVICTS the
/// file's now-stale chunks so a SECOND search never returns them either. The metric reports one drop.
#[test]
fn search_result_file_deleted_on_disk_is_dropped_from_results() {
    let (server, _tmp, root) = test_server_on_disk(&[(
        "ghost.py",
        "def haunted_function(spectre):\n    return spectre.materialize()\n",
    )]);

    // DELETE the only file matching the query, behind the index's back.
    std::fs::remove_file(root.join("ghost.py")).expect("delete source file on disk");

    // First search: must NOT panic/error, and must NOT surface the deleted file's symbol.
    let (resp, stats) = search_with_stats(server, 32, "haunted function");
    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(32),
        "search response must echo the request id even when a result file vanished; got: {resp}"
    );
    assert!(
        resp.get("error").is_none(),
        "a deleted result file must be handled gracefully, NOT as a JSON-RPC error; got: {resp}"
    );
    let text = call_result_text(&resp);
    assert!(
        !text.contains("haunted_function"),
        "a result whose file was deleted on disk must be dropped from the response; got: {text:?}"
    );
    assert_eq!(
        stats.files_dropped, 1,
        "the deleted result file must be counted as dropped; got: {stats:?}"
    );

    // The stale chunk must be EVICTED: a second, fresh server over the same DB must not return it,
    // proving the eviction was persisted (not just filtered in-memory for the first response).
    let db_path = root.join(".codecache").join("index.db");
    let storage2 = Storage::new(&db_path).expect("reopen indexed db");
    let server2 = CodeCacheServer::new(storage2);
    let resp2 = tools_call(
        server2,
        33,
        "codecache_search",
        serde_json::json!({ "query": "haunted function" }),
    );
    let text2 = call_result_text(&resp2);
    assert!(
        !text2.contains("haunted_function"),
        "the deleted file's stale chunk must be EVICTED so a later search never returns it; got: {text2:?}"
    );
}

// ── 19. self-heal is BOUNDED to the result files (cost ∝ result count) ────────────────────────────

/// The self-heal only touches files SURFACED by the first query, not the whole index. Index two
/// unrelated files; edit BOTH on disk; search a term that matches only the FIRST → the surfaced file
/// is re-indexed (its fresh body returns) but the UNRELATED edited file is left stale (not checked,
/// not re-indexed). Pins the D14 bound (overview §5.2): healing cost is proportional to result count.
#[test]
fn search_self_heal_is_bounded_to_result_files() {
    let (server, _tmp, root) = test_server_on_disk(&[
        (
            "alpha.py",
            "def alpha_widget():\n    return old_alpha_value()\n",
        ),
        (
            "beta.py",
            "def beta_gadget():\n    return old_beta_value()\n",
        ),
    ]);

    // Edit BOTH files behind the index's back.
    std::fs::write(
        root.join("alpha.py"),
        "def alpha_widget():\n    return new_alpha_value()\n",
    )
    .expect("edit alpha.py");
    std::fs::write(
        root.join("beta.py"),
        "def beta_gadget():\n    return new_beta_value()\n",
    )
    .expect("edit beta.py");

    // Query matches only alpha — beta is never surfaced, so it must not be hash-checked/re-indexed.
    let (resp, stats) = search_with_stats(server, 34, "alpha widget");
    let text = call_result_text(&resp);
    assert!(
        text.contains("new_alpha_value"),
        "the surfaced file must be healed to its fresh body; got: {text:?}"
    );
    assert!(
        !text.contains("alpha_widget") || !text.contains("old_alpha_value"),
        "the surfaced file's stale body must not survive; got: {text:?}"
    );

    // Exactly one file (alpha) was checked + re-indexed; the unrelated edited beta was NOT touched.
    assert_eq!(
        stats.files_checked, 1,
        "only the result file may be hash-checked (cost ∝ result count, D14); got: {stats:?}"
    );
    assert_eq!(
        stats.files_reindexed, 1,
        "only the surfaced file may be re-indexed; the unrelated edit stays stale; got: {stats:?}"
    );

    // Proof beta stayed stale: its stored chunk still references the OLD body (it was never healed).
    let db_path = root.join(".codecache").join("index.db");
    let probe = Storage::new(&db_path).expect("open indexed db for probing");
    let beta_abs = root.join("beta.py");
    let beta_changed = codecache::hasher::is_changed(
        &beta_abs,
        probe
            .get_file_hash(&beta_abs)
            .expect("read beta hash")
            .as_deref(),
    )
    .expect("hash-check beta");
    assert!(
        beta_changed,
        "beta.py was edited but never surfaced, so the index must still hold its STALE hash \
         (a whole-index re-heal would have updated it — D14 bounds the heal to result files)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// D33 — MCP `codecache_search` `file_filter` is a GLOB match (parity with the CLI), and a malformed
// glob is a JSON-RPC `-32602` (invalid params), NOT an internal `-32603` and NOT a panic (RED,
// test-lead, 2026-06-22).
//
// Pre-D33 bug: `handle_search` wrapped the raw `file_filter` string as a literal `PathBuf` and the
// retriever compared it for exact equality against absolute stored paths ⇒ any `file_filter` value
// dropped every result. D33 makes `file_filter` a glob compiled in the retriever (one code path for
// CLI + MCP). For MCP the malformed-glob error must map to `-32602` because the bad pattern is an
// invalid ARGUMENT (a param of `tools/call`), not an internal retrieval failure — so the eng-lead
// must remap the new `RetrieverError::InvalidFilter` to `-32602` (the current handler funnels all
// retriever errors to `-32603`, which is why the malformed test is RED on the code, not just compile).
//
// These tests use the existing in-memory `serve` harness + `test_server_seeded` seeding (the seeded
// `file_path`s are relative, e.g. `src/auth.py`, so a basename/extension glob restricts them just
// like discovery-stored absolute paths would under suffix-anchoring).
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// `tools/call codecache_search { query, file_filter }` with a GLOB `file_filter` restricts the
/// results to the matching files — proving the MCP path uses the same glob semantics as the CLI.
/// The seed has one `.py` and one `.go` symbol both matching the query term; `*.py` keeps only the
/// `.py` hit. Under the pre-D33 exact-equality bug this returned NOTHING.
#[test]
fn call_codecache_search_file_filter_glob_restricts_results() {
    let (server, _tmp) = test_server_seeded(&[
        seed_chunk(
            "src/auth.py",
            "lookup_account",
            SymbolType::Function,
            None,
            "def lookup_account(user):\n    return account_lookup(user)",
            10,
            20,
        ),
        seed_chunk(
            "src/store.go",
            "LookupRecord",
            SymbolType::Function,
            None,
            "func LookupRecord(id string) Record {\n    return recordLookup(id)\n}",
            5,
            15,
        ),
    ]);

    // `*.py` (suffix-anchored to `**/*.py`) keeps only the Python hit; the Go hit is filtered out.
    let resp = tools_call(
        server,
        40,
        "codecache_search",
        serde_json::json!({ "query": "lookup", "file_filter": "*.py" }),
    );
    assert_eq!(
        resp.get("id").and_then(|v| v.as_i64()),
        Some(40),
        "search response must echo the request id; got: {resp}"
    );

    let text = call_result_text(&resp);
    assert!(
        text.contains("lookup_account"),
        "the .py hit must survive the `*.py` file_filter glob; got: {text:?}"
    );
    assert!(
        !text.contains("LookupRecord"),
        "the .go hit must be excluded by the `*.py` file_filter glob; got: {text:?}"
    );
    assert!(
        !text.contains("src/store.go"),
        "no .go locator may appear under a `*.py` file_filter; got: {text:?}"
    );
}

/// A MALFORMED `file_filter` glob (unclosed character class `a/[`) must map to JSON-RPC `-32602`
/// (invalid params) — a bad argument — NOT the `-32603` internal-error code and NOT a panic. Pins
/// the D33 error-code remap for the MCP surface.
#[test]
fn call_codecache_search_malformed_file_filter_is_invalid_params() {
    let (server, _tmp) = test_server_seeded(&[seed_chunk(
        "src/auth.py",
        "lookup_account",
        SymbolType::Function,
        None,
        "def lookup_account(user):\n    return account_lookup(user)",
        10,
        20,
    )]);

    let resp = tools_call(
        server,
        41,
        "codecache_search",
        serde_json::json!({ "query": "lookup", "file_filter": "a/[" }),
    );
    // -32602 (invalid params), echoing id 41 — a malformed glob is a bad ARGUMENT, not -32603.
    assert_error_code(&resp, -32602, 41);
}
