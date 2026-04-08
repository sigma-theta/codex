[CmdletBinding()]
param(
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

function Get-WorkspaceVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ManifestPath
    )

    $manifestContent = Get-Content $ManifestPath -Raw
    $match = [regex]::Match($manifestContent, '(?ms)\[workspace\.package\].*?^version = "(?<version>[^"]+)"')
    if (-not $match.Success) {
        throw "Failed to determine workspace.package.version from $ManifestPath"
    }

    $match.Groups['version'].Value
}

function Get-LatestReleaseVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$UpstreamRemoteName
    )

    Invoke-Step "git fetch $UpstreamRemoteName --tags" -AllowDryRun

    $tags = Get-GitOutput "git tag --list 'rust-v*' --sort=-version:refname"
    $latestTag = $tags | Select-Object -First 1
    if (-not $latestTag) {
        throw "Failed to determine the latest upstream rust release tag."
    }

    if ($latestTag -notmatch '^rust-v(?<version>.+)$') {
        throw "Unexpected rust release tag format: $latestTag"
    }

    $Matches.version
}

function Set-WorkspaceVersionForBuild {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ManifestPath,
        [Parameter(Mandatory = $true)]
        [string]$BuildVersion
    )

    $manifestContent = Get-Content $ManifestPath -Raw
    $versionPattern = '(?ms)(\[workspace\.package\].*?^version = ")([^"]+)(")'
    $updatedContent = [regex]::Replace(
        $manifestContent,
        $versionPattern,
        ('$1{0}$3' -f $BuildVersion),
        1
    )

    if ($updatedContent -eq $manifestContent) {
        throw "Failed to update workspace.package.version in $ManifestPath"
    }

    Set-Content -Path $ManifestPath -Value $updatedContent -NoNewline
    $manifestContent
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
    Invoke-Step "git fetch $UpstreamRemote $BaseBranch" -AllowDryRun
    Invoke-Step "git fetch $PushRemote" -AllowDryRun
    Invoke-Step "git switch $BaseBranch" -AllowDryRun
    Invoke-Step "git rebase $UpstreamRemote/$BaseBranch" -AllowDryRun

    if (-not $SkipPush) {
        Invoke-Step "git push $PushRemote $BaseBranch" -AllowDryRun
    }

    if (-not $SkipBuild) {
        $codexRsRoot = Join-Path $repoRoot "codex-rs"
        $cargoManifestPath = Join-Path $codexRsRoot "Cargo.toml"
        Set-Location $codexRsRoot
        $originalManifestContent = $null

        try {
            if (-not $DryRun) {
                $workspaceVersion = Get-WorkspaceVersion -ManifestPath $cargoManifestPath
                if ($workspaceVersion -eq "0.0.0") {
                    $latestReleaseVersion = Get-LatestReleaseVersion -UpstreamRemoteName $UpstreamRemote
                    $shortHead = Get-GitOutput "git rev-parse --short HEAD" | Select-Object -First 1
                    $buildVersion = "$latestReleaseVersion-local+$shortHead"
                    Write-Host "==> Using temporary build version $buildVersion"
                    $originalManifestContent = Set-WorkspaceVersionForBuild -ManifestPath $cargoManifestPath -BuildVersion $buildVersion
                }
            }

            Invoke-Step "cargo build --release -p codex-tui --bins" -AllowDryRun
        } finally {
            if ($null -ne $originalManifestContent) {
                Set-Content -Path $cargoManifestPath -Value $originalManifestContent -NoNewline
            }
        }
    }
} finally {
    Set-Location $repoRoot
    if (-not $DryRun -and $originalBranch -and $originalBranch -ne (Get-GitOutput "git branch --show-current" | Select-Object -First 1)) {
        Invoke-Step "git switch $originalBranch"
    }
}
