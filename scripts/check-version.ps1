$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw | Select-String -Pattern '(?m)^version\s*=\s*"([^"]+)"' | ForEach-Object { $_.Matches[0].Groups[1].Value }
$version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
$site = Get-Content (Join-Path $root 'site/index.html') -Raw | Select-String -Pattern 'id="release-value">v([^<]+)' | ForEach-Object { $_.Matches[0].Groups[1].Value }
$metrics = Get-Content (Join-Path $root 'site/metrics.json') -Raw | ConvertFrom-Json
$values = @{
    'Cargo.toml' = $cargo
    'VERSION' = $version
    'site/index.html' = $site
    'site/metrics.json' = $metrics.latestRelease.TrimStart('v')
}

$mismatched = $values.GetEnumerator() | Where-Object { $_.Value -ne $cargo }
if ($mismatched) {
    $details = ($values.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join ', '
    throw "Version mismatch: $details"
}

Write-Host "Version consistency check passed: v$cargo"
