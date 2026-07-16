param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

function Update-TextFile([string] $Path, [string] $Pattern, [string] $Replacement) {
    $fullPath = Join-Path $root $Path
    $text = [System.IO.File]::ReadAllText($fullPath)
    $updated = ([regex]::new($Pattern)).Replace($text, $Replacement, 1)
    if ($updated -eq $text) {
        throw "Could not update $Path"
    }
    [System.IO.File]::WriteAllText($fullPath, $updated, [System.Text.UTF8Encoding]::new($false))
}

Update-TextFile 'Cargo.toml' '(?m)^version\s*=\s*"[^"]+"' "version = `"$Version`""
Update-TextFile 'VERSION' '^\s*[^\r\n]+' $Version
Update-TextFile 'site/index.html' '(<strong class="pulse-value pulse-version" id="release-value">)v[^<]+(</strong>)' "`$1v$Version`$2"
Update-TextFile 'site/metrics.json' '("latestRelease"\s*:\s*")v[^"]+(")' "`$1v$Version`$2"

$changelog = Join-Path $root 'CHANGELOG.md'
$content = [System.IO.File]::ReadAllText($changelog)
$date = Get-Date -Format 'yyyy-MM-dd'
$entry = "## [$Version] - $date`r`n`r`n### Added`r`n- **Release preparation** - version bumped to $Version.`r`n`r`n"
$content = [regex]::Replace($content, '(## \[Unreleased\]\s*(?:\r?\n)+)', "`$1`r`n$entry", 1)
[System.IO.File]::WriteAllText($changelog, $content, [System.Text.UTF8Encoding]::new($false))

Push-Location $root
try {
    cargo check
    if ($LASTEXITCODE -ne 0) {
        throw "cargo check failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

& (Join-Path $PSScriptRoot 'check-version.ps1')
Write-Host "Version bumped to $Version. Review CHANGELOG.md before committing."
