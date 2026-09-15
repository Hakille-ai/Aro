$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$envFile = Join-Path $projectRoot ".env"

if (Test-Path -LiteralPath $envFile) {
    Get-Content -LiteralPath $envFile | ForEach-Object {
        $line = $_.Trim()
        if ($line -and -not $line.StartsWith("#") -and $line.Contains("=")) {
            $parts = $line.Split('=', 2)
            $varName = $parts[0].Trim()
            $varVal = $parts[1].Trim()
            [System.Environment]::SetEnvironmentVariable($varName, $varVal, 'Process')
        }
    }
}

$binPath = Join-Path $projectRoot "target\debug\aro-api.exe"
if (-not (Test-Path -LiteralPath $binPath)) {
    Write-Host "aro-api.exe not found in target\debug, compiling..." -ForegroundColor Yellow
    Set-Location $projectRoot
    cargo build -p aro-api
}

Write-Host "Starting ARO API on $($env:ARO_API_BIND)..." -ForegroundColor Cyan
Set-Location $projectRoot
& $binPath serve
