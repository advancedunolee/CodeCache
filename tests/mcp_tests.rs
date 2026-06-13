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
