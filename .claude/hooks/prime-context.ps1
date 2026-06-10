# SessionStart hook: prime the session with CodeCache's current state.
# Emits the in-progress + next-up items from docs/TODO.md so every session and
# subagent starts aligned without a manual lookup. stdout is injected as context.
$ErrorActionPreference = 'SilentlyContinue'

$root = $env:CLAUDE_PROJECT_DIR
if (-not $root) { $root = (Get-Location).Path }
$todo = Join-Path $root 'docs\TODO.md'
if (-not (Test-Path $todo)) { exit 0 }

$lines = Get-Content $todo
$inProgress = $lines | Where-Object { $_ -match '^\s*-\s*\[~\]' }
$open        = $lines | Where-Object { $_ -match '^\s*-\s*\[ \]' } | Select-Object -First 6

Write-Output "## CodeCache — current state (auto-primed from docs/TODO.md)"
Write-Output ""
if ($inProgress) {
    Write-Output "In progress:"
    $inProgress | ForEach-Object { Write-Output ("  " + $_.Trim()) }
    Write-Output ""
}
if ($open) {
    Write-Output "Next up:"
    $open | ForEach-Object { Write-Output ("  " + $_.Trim()) }
    Write-Output ""
}
Write-Output "Reminders: TDD (failing test first). Kick off non-trivial work via the"
Write-Output "principal-engineering-manager agent (it writes a brief in .claude/briefs/)."
Write-Output "See CLAUDE.md, docs/ENGINEERING_PLAN.md, docs/ROADMAP.md."
exit 0
