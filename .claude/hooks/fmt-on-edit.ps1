# PostToolUse hook: format Rust on every Edit/Write of a .rs file.
# Non-blocking: always exits 0. No-ops cleanly before the Rust project is scaffolded.
# Receives the tool event as JSON on stdin (tool_input.file_path is the edited file).
$ErrorActionPreference = 'SilentlyContinue'

$raw = [Console]::In.ReadToEnd()
try { $data = $raw | ConvertFrom-Json } catch { exit 0 }

$fp = $data.tool_input.file_path
if (-not $fp) { exit 0 }
if ($fp -notlike '*.rs') { exit 0 }

# Project root is provided by Claude Code; fall back to current dir.
$root = $env:CLAUDE_PROJECT_DIR
if (-not $root) { $root = (Get-Location).Path }

if (-not (Test-Path (Join-Path $root 'Cargo.toml'))) { exit 0 }  # not scaffolded yet

Push-Location $root
try { & cargo fmt 2>$null } catch { } finally { Pop-Location }
exit 0
