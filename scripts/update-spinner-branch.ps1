[CmdletBinding()]
param(
    [string]$PatchBranch = "local/spinner-words",
    [string]$BaseBranch = "main",
    [string]$UpstreamRemote = "origin",
    [string]$PushRemote = "fork",
    [switch]$SkipBuild,
    [switch]$SkipPush,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Invoke-Step {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [switch]$AllowDryRun
    )

    Write-Host "==> $Command"
    if ($DryRun -and $AllowDryRun) {
        return
    }

    & powershell -NoProfile -Command $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed: $Command"
    }
}

function Get-GitOutput {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    $output = & powershell -NoProfile -Command $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed: $Command"
    }
    $output
}

$repoRoot = Get-GitOutput "git rev-parse --show-toplevel"
$repoRoot = $repoRoot | Select-Object -First 1
if (-not $repoRoot) {
    throw "Failed to determine repository root."
}

Set-Location $repoRoot

$status = Get-GitOutput "git status --porcelain"
if (-not $DryRun -and $status) {
    throw "Working tree is not clean. Commit or stash changes before running this script."
}

$originalBranch = Get-GitOutput "git branch --show-current"
$originalBranch = $originalBranch | Select-Object -First 1
if (-not $originalBranch) {
    throw "Failed to determine current branch."
}

try {
    Invoke-Step "git fetch $UpstreamRemote $BaseBranch $PushRemote" -AllowDryRun
    Invoke-Step "git switch $BaseBranch" -AllowDryRun
    Invoke-Step "git rebase $UpstreamRemote/$BaseBranch" -AllowDryRun
    Invoke-Step "git switch $PatchBranch" -AllowDryRun
    Invoke-Step "git rebase $BaseBranch" -AllowDryRun

    if (-not $SkipPush) {
        Invoke-Step "git push --force-with-lease $PushRemote $PatchBranch" -AllowDryRun
    }

    if (-not $SkipBuild) {
        $codexRsRoot = Join-Path $repoRoot "codex-rs"
        Set-Location $codexRsRoot
        Invoke-Step "cargo build --release -p codex-tui" -AllowDryRun
    }
} finally {
    Set-Location $repoRoot
    if (-not $DryRun -and $originalBranch -and $originalBranch -ne (Get-GitOutput "git branch --show-current" | Select-Object -First 1)) {
        Invoke-Step "git switch $originalBranch"
    }
}
