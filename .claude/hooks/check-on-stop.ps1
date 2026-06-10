# Stop / SubagentStop hook: enforce quality gates before a turn ends.
# Runs `cargo clippy -D warnings` then `cargo test`; on failure exits 2 to surface the
# output back into the session so red lint/tests are never silently left behind.
# Honors `stop_hook_active` to avoid infinite stop loops, and no-ops before scaffolding.
$ErrorActionPreference = 'SilentlyContinue'

$raw = [Console]::In.ReadToEnd()
try { $data = $raw | ConvertFrom-Json } catch { $data = $null }

# Prevent loops: if we already blocked once this stop, let it proceed.
if ($data -and $data.stop_hook_active -eq $true) { exit 0 }

$root = $env:CLAUDE_PROJECT_DIR
if (-not $root) { $root = (Get-Location).Path }
if (-not (Test-Path (Join-Path $root 'Cargo.toml'))) { exit 0 }  # not scaffolded yet

Push-Location $root
try {
    $clippy = & cargo clippy --all-targets -- -D warnings 2>&1
    if ($LASTEXITCODE -ne 0) {
        [Console]::Error.WriteLine("[quality-gate] clippy failed - fix warnings before finishing:`n$clippy")
        exit 2
    }
    $test = & cargo test --quiet 2>&1
    if ($LASTEXITCODE -ne 0) {
        [Console]::Error.WriteLine("[quality-gate] tests failed - fix red tests before finishing:`n$test")
        exit 2
    }
}
finally { Pop-Location }
exit 0
