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

function Test-RebaseInProgress {
    $rebaseApplyPath = Get-GitOutput "git rev-parse --git-path rebase-apply"
    $rebaseApplyPath = $rebaseApplyPath | Select-Object -First 1
    $rebaseMergePath = Get-GitOutput "git rev-parse --git-path rebase-merge"
    $rebaseMergePath = $rebaseMergePath | Select-Object -First 1

    (Test-Path $rebaseApplyPath) -or (Test-Path $rebaseMergePath)
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

function Normalize-ReleaseVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$TagName
    )

    $normalizedVersion = $TagName.Trim()
    while ($normalizedVersion.StartsWith("rust-v")) {
        $normalizedVersion = $normalizedVersion.Substring(6)
    }
    while ($normalizedVersion.StartsWith("v")) {
        $normalizedVersion = $normalizedVersion.Substring(1)
    }

    if ($normalizedVersion -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$') {
        throw "Unexpected rust release tag format: $TagName"
    }

    $normalizedVersion
}

function Compare-PreReleaseIdentifiers {
    param(
        [string[]]$LeftIdentifiers,
        [string[]]$RightIdentifiers
    )

    $sharedLength = [Math]::Min($LeftIdentifiers.Length, $RightIdentifiers.Length)
    for ($index = 0; $index -lt $sharedLength; $index++) {
        $leftIdentifier = $LeftIdentifiers[$index]
        $rightIdentifier = $RightIdentifiers[$index]
        $leftIsNumeric = $leftIdentifier -match '^\d+$'
        $rightIsNumeric = $rightIdentifier -match '^\d+$'

        if ($leftIsNumeric -and $rightIsNumeric) {
            $leftNumber = [int64]$leftIdentifier
            $rightNumber = [int64]$rightIdentifier
            if ($leftNumber -ne $rightNumber) {
                return [Math]::Sign($leftNumber - $rightNumber)
            }
            continue
        }

        if ($leftIsNumeric -ne $rightIsNumeric) {
            if ($leftIsNumeric) {
                return -1
            }
            return 1
        }

        $identifierComparison = [string]::CompareOrdinal($leftIdentifier, $rightIdentifier)
        if ($identifierComparison -ne 0) {
            return [Math]::Sign($identifierComparison)
        }
    }

    if ($LeftIdentifiers.Length -eq $RightIdentifiers.Length) {
        return 0
    }

    if ($LeftIdentifiers.Length -lt $RightIdentifiers.Length) {
        return -1
    }

    1
}

function Compare-SemVer {
    param(
        [Parameter(Mandatory = $true)]
        [string]$LeftVersion,
        [Parameter(Mandatory = $true)]
        [string]$RightVersion
    )

    $versionPattern = '^(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)(?:-(?<prerelease>[0-9A-Za-z.-]+))?(?:\+(?<build>[0-9A-Za-z.-]+))?$'
    $leftMatch = [regex]::Match($LeftVersion, $versionPattern)
    $rightMatch = [regex]::Match($RightVersion, $versionPattern)
    if (-not $leftMatch.Success -or -not $rightMatch.Success) {
        throw "Failed to parse semantic version(s): $LeftVersion / $RightVersion"
    }

    foreach ($component in @('major', 'minor', 'patch')) {
        $leftValue = [int64]$leftMatch.Groups[$component].Value
        $rightValue = [int64]$rightMatch.Groups[$component].Value
        if ($leftValue -ne $rightValue) {
            return [Math]::Sign($leftValue - $rightValue)
        }
    }

    $leftPreRelease = $leftMatch.Groups['prerelease'].Value
    $rightPreRelease = $rightMatch.Groups['prerelease'].Value
    $leftHasPreRelease = -not [string]::IsNullOrEmpty($leftPreRelease)
    $rightHasPreRelease = -not [string]::IsNullOrEmpty($rightPreRelease)

    if ($leftHasPreRelease -and -not $rightHasPreRelease) {
        return -1
    }
    if (-not $leftHasPreRelease -and $rightHasPreRelease) {
        return 1
    }
    if (-not $leftHasPreRelease -and -not $rightHasPreRelease) {
        return 0
    }

    Compare-PreReleaseIdentifiers -LeftIdentifiers ($leftPreRelease -split '\.') -RightIdentifiers ($rightPreRelease -split '\.')
}

function Get-LatestReleaseVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$UpstreamRemoteName
    )

    Invoke-Step "git fetch $UpstreamRemoteName --tags" -AllowDryRun

    $tags = @(Get-GitOutput "git tag --list 'rust-v*'")
    if ($tags.Count -eq 0) {
        throw "Failed to determine the latest upstream rust release tag."
    }

    $latestVersion = $null
    $skippedTagCount = 0
    foreach ($tag in $tags) {
        try {
            $normalizedVersion = Normalize-ReleaseVersion -TagName $tag
        } catch {
            $skippedTagCount += 1
            continue
        }
        if ($normalizedVersion -match '^0\.0\.') {
            $skippedTagCount += 1
            continue
        }

        if ($null -eq $latestVersion -or (Compare-SemVer -LeftVersion $normalizedVersion -RightVersion $latestVersion) -gt 0) {
            $latestVersion = $normalizedVersion
        }
    }

    if ($null -eq $latestVersion) {
        throw "Failed to determine a valid upstream rust release tag."
    }
    if ($skippedTagCount -gt 0) {
        Write-Host "==> Ignored $skippedTagCount invalid rust release tag(s) while determining the latest upstream version"
    }

    $latestVersion
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

$backupBranch = $null

try {
    Invoke-Step "git fetch $UpstreamRemote $BaseBranch" -AllowDryRun
    Invoke-Step "git fetch $PushRemote" -AllowDryRun
    Invoke-Step "git switch $BaseBranch" -AllowDryRun

    if (-not $DryRun) {
        $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
        $backupBranch = "backup/$BaseBranch-before-upstream-sync-$timestamp"
        Invoke-Step "git branch $backupBranch"

        $customCommitCount = Get-GitOutput "git rev-list --count $UpstreamRemote/$BaseBranch..$BaseBranch"
        $customCommitCount = $customCommitCount | Select-Object -First 1
        Write-Host "==> Replaying $customCommitCount customization commit(s) onto $UpstreamRemote/$BaseBranch"
        Write-Host "==> If anything goes wrong, your pre-update branch tip is saved at $backupBranch"
    }

    Invoke-Step "git rebase --rebase-merges -X theirs $UpstreamRemote/$BaseBranch" -AllowDryRun

    if (-not $SkipPush) {
        Invoke-Step "git push --force-with-lease $PushRemote $BaseBranch" -AllowDryRun
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
} catch {
    if (-not $DryRun -and (Test-RebaseInProgress)) {
        Invoke-Step "git rebase --abort"
    }

    if ($backupBranch) {
        Write-Host "==> Update failed. Original branch tip preserved at $backupBranch"
    }

    throw
} finally {
    Set-Location $repoRoot
    if (-not $DryRun -and $originalBranch -and $originalBranch -ne (Get-GitOutput "git branch --show-current" | Select-Object -First 1)) {
        Invoke-Step "git switch $originalBranch"
    }
}
