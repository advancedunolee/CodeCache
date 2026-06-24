//! MCP tool definitions (Decision Log D13) for `tools/list`.
//!
//! Holds the three CodeCache tool schemas (`codecache_search`, `codecache_update`,
//! `codecache_outline`) as hand-written `serde_json` values mirroring `project_plan.md` §8.2
//! verbatim. [`tool_definitions`] returns them in a FIXED, deterministic order
//! `[search, update, outline]` (a `Vec`, never `HashMap` iteration) so an MCP client sees a
//! stable list across calls. `tools/call` execution is M8.3 — this module only describes the
//! tools; it does not invoke them.

use serde_json::{json, Value};

/// The three MCP tool definitions in the pinned, stable order
/// `[codecache_search, codecache_update, codecache_outline]` (§8.2 / D13). Each is a
/// `{ name, description, inputSchema }` object placed under `result.tools` by `tools/list`.
pub(crate) fn tool_definitions() -> Vec<Value> {
    vec![search_tool(), update_tool(), outline_tool()]
}

/// Tool 1 — `codecache_search` (§8.2): `query` (string), `max_tokens` (integer, default `4000`),
/// `file_filter` (string, default `null`); required `["query"]`. `file_filter` is a **glob** (D33)
/// matched against stored paths — non-absolute patterns are suffix-anchored, absolute used as-is.
fn search_tool() -> Value {
    json!({
        "name": "codecache_search",
        "description": "Search the codebase for relevant functions, classes, or code snippets using semantic queries. Returns concentrated code context optimized for token budgets.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Free-form search query (e.g., 'authentication logic', 'error handling')"
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "Maximum tokens to return (for context budget)",
                    "default": 4000
                },
                "file_filter": {
                    "type": "string",
                    "description": "Optional: restrict results to files matching a glob (D33). A pattern without a leading '/' is suffix-anchored (e.g. '*.py' matches any .py file, 'src/auth/**' matches that subtree anywhere); an absolute glob is used as-is. A malformed glob is a clean error, not a silent empty result.",
                    "default": null
                }
            },
            "required": ["query"]
        }
    })
}

/// Tool 2 — `codecache_update` (§8.2): `files` (array of string); required `["files"]`.
fn update_tool() -> Value {
    json!({
        "name": "codecache_update",
        "description": "Incrementally update the CodeCache index for specific files. Call this after modifying code to ensure fresh search results.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of file paths to re-index"
                }
            },
            "required": ["files"]
        }
    })
}

/// Tool 3 — `codecache_outline` (§8.2 / D13): `path` (string), `max_tokens` (integer, default
/// `2000`); required `["path"]`.
fn outline_tool() -> Value {
    json!({
        "name": "codecache_outline",
        "description": "Return the symbol skeleton (functions, classes, methods with signatures and line ranges) of a file or directory. The cheapest way to orient in unfamiliar code before reading bodies.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File or directory path to outline (relative to the indexed root)"
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "Maximum tokens to return",
                    "default": 2000
                }
            },
            "required": ["path"]
        }
    })
}
