//! MCP server: hand-rolled JSON-RPC 2.0 over stdio (Decision Log D15 — serde/serde_json only,
//! no `rmcp`, no tokio).
//!
//! API anchor: `project_plan.md` §8.2 / §8.3. Transport-agnostic core (Decision Log D4): the
//! [`serve`] loop is generic over any [`BufRead`]/[`Write`] so the real CLI hands it
//! `stdin.lock()`/`stdout.lock()` while tests inject in-memory pipes. Lends a shared `Storage`
//! (`Arc<Mutex<Connection>>`, Decision Log D8) to `Retriever`/`Indexer` (wired in M8.3).
//!
//! M8.1 scope: line-delimited JSON framing (one `\n`-terminated object per line), the
//! `initialize` handshake, and strict JSON-RPC error mapping (-32700 / -32601 / -32602). The loop
//! NEVER panics on malformed input — every bad line yields a structured error and the loop
//! continues; clean EOF returns `Ok(())`. `tools/list` / `tools/call` / self-healing are M8.2–M8.4.

mod handlers;
mod tools;

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::storage::Storage;

/// MCP protocol revision advertised in the `initialize` result. Pinned by M8.1 (the project plan
/// pins none); change in lock-step with `tests/mcp_tests.rs::PROTOCOL_VERSION`.
const PROTOCOL_VERSION: &str = "2024-11-05";
/// Server name reported in `serverInfo`.
const SERVER_NAME: &str = "codecache";

// ── JSON-RPC 2.0 error codes (subset used in M8.1) ───────────────────────────────────────────
const PARSE_ERROR: i64 = -32700;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// The MCP server context. Holds the shared [`Storage`] (D8) so M8.2–M8.4 can lend it to
/// `Retriever`/`Indexer` without changing this seam. The M8.1 handshake path does not read it.
pub struct CodeCacheServer {
    /// The shared connection (D8), lent onward to the `Retriever`/`Indexer` built per `tools/call`.
    storage: Storage,
}

impl CodeCacheServer {
    /// Build a server over a shared `Storage` (D8: one `Arc<Mutex<Connection>>` lent onward).
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// Dispatch one parsed JSON-RPC request object to its handler, returning the JSON value to
    /// place under `result`. `Err(code, message)` maps to a JSON-RPC error object. Takes `&mut
    /// self` because `tools/call codecache_update` mutates the index (M8.3).
    fn dispatch(&mut self, method: &str, params: Option<&Value>) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => self.handle_initialize(params),
            "tools/list" => Ok(self.handle_tools_list()),
            "tools/call" => self.handle_tools_call(params),
            _ => Err((METHOD_NOT_FOUND, format!("method not found: {method}"))),
        }
    }

    /// `initialize` → advertise protocol version, capabilities, and server info. Requires a
    /// `params` object carrying `protocolVersion`; its absence is an invalid-params error.
    fn handle_initialize(&self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let params = params.ok_or_else(|| {
            (
                INVALID_PARAMS,
                "initialize requires a `params` object with `protocolVersion`".to_string(),
            )
        })?;
        if params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err((
                INVALID_PARAMS,
                "initialize `params.protocolVersion` is required".to_string(),
            ));
        }

        Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "serverInfo": {
                "name": SERVER_NAME,
                "version": crate::VERSION,
            }
        }))
    }

    /// `tools/list` → enumerate the three D13 tools (`codecache_search`, `codecache_update`,
    /// `codecache_outline`) with their §8.2 inputSchemas, in a fixed deterministic order.
    /// `params` is optional per MCP and is ignored.
    fn handle_tools_list(&self) -> Value {
        json!({ "tools": tools::tool_definitions() })
    }

    /// `tools/call` → execute one of the three D13 tools (M8.3). `params` must carry `name` (the
    /// tool) and `arguments` (its input object). On success the `result` is the MCP content
    /// envelope `{ content: [ { type:"text", text } ] }`. A missing/wrong-typed required argument
    /// OR an unknown tool name maps to `-32602` (invalid params): per MCP the tool `name` is a
    /// *param* of `tools/call`, so a bad name is an invalid param, not an unknown method (-32601).
    fn handle_tools_call(&mut self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let params = params.ok_or_else(|| {
            (
                INVALID_PARAMS,
                "tools/call requires a `params` object with `name` and `arguments`".to_string(),
            )
        })?;
        let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
            (
                INVALID_PARAMS,
                "tools/call requires a string `name`".to_string(),
            )
        })?;
        // `arguments` is optional in the envelope; an absent object behaves like `{}` so the
        // per-tool required-argument checks produce the -32602 (not a structural error here).
        let empty = json!({});
        let arguments = params.get("arguments").unwrap_or(&empty);

        let text = match name {
            "codecache_search" => handlers::handle_search(&self.storage, arguments),
            "codecache_update" => handlers::handle_update(&self.storage, arguments),
            "codecache_outline" => handlers::handle_outline(&self.storage, arguments),
            other => Err((INVALID_PARAMS, format!("unknown tool: {other}"))),
        }?;

        Ok(json!({ "content": [ { "type": "text", "text": text } ] }))
    }
}

/// Transport-agnostic (D4) read → dispatch → write loop. Reads line-delimited JSON-RPC requests
/// from `reader`, writes one `\n`-terminated JSON-RPC response per answered line to `writer`.
/// Returns `Ok(())` at clean EOF and NEVER panics on malformed input — a bad line yields a
/// structured JSON-RPC error and the loop continues. `R`/`W` are generic so tests inject in-memory
/// pipes while the CLI passes `stdin.lock()`/`stdout.lock()`.
pub fn serve<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    mut server: CodeCacheServer,
) -> anyhow::Result<()> {
    for line in reader.lines() {
        let line = line?;
        // Blank/whitespace lines carry no request; skip without emitting a frame.
        if line.trim().is_empty() {
            continue;
        }

        let response = handle_line(&mut server, &line);
        write_frame(&mut writer, &response)?;
    }
    writer.flush()?;
    Ok(())
}

/// Build the JSON-RPC response value for a single (non-blank) input line. Parse failures and
/// structurally invalid envelopes map to error objects; valid envelopes dispatch by `method`.
fn handle_line(server: &mut CodeCacheServer, line: &str) -> Value {
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => return error_response(Value::Null, PARSE_ERROR, "Parse error"),
    };

    // A JSON-RPC request must be an object; a bare array/scalar is malformed.
    let Some(obj) = request.as_object() else {
        return error_response(
            Value::Null,
            PARSE_ERROR,
            "Parse error: request must be a JSON object",
        );
    };

    // Echo the request id verbatim (or null when absent/unparseable per JSON-RPC).
    let id = obj.get("id").cloned().unwrap_or(Value::Null);

    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return error_response(id, INVALID_PARAMS, "Invalid request: missing `method`");
    };

    match server.dispatch(method, obj.get("params")) {
        Ok(result) => success_response(id, result),
        Err((code, message)) => error_response(id, code, &message),
    }
}

/// A JSON-RPC 2.0 success envelope: `{ jsonrpc, id, result }`.
fn success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// A JSON-RPC 2.0 error envelope: `{ jsonrpc, id, error: { code, message } }`.
fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

/// Serialize `value` to a single line and write it `\n`-terminated (line-delimited framing).
fn write_frame<W: Write>(writer: &mut W, value: &Value) -> anyhow::Result<()> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    writer.write_all(line.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_response_has_null_id_and_code() {
        let resp = error_response(Value::Null, PARSE_ERROR, "Parse error");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["error"]["code"], PARSE_ERROR);
        assert!(resp["id"].is_null());
    }
}
