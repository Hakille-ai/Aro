param(
  [int]$Port = 1420,
  [string]$HostAddress = "127.0.0.1",
  [string]$RepoRoot = (Resolve-Path "$PSScriptRoot\..").Path
)

$ErrorActionPreference = "Stop"

function Get-ProcessInfo {
  param([int]$ProcessId)
  Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId" -ErrorAction SilentlyContinue
}

$resolvedRepoRoot = (Resolve-Path $RepoRoot).Path.TrimEnd('\').ToLowerInvariant()
$listenConnections = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
  Where-Object {
    $_.LocalAddress -eq $HostAddress -or
    $_.LocalAddress -eq "0.0.0.0" -or
    $_.LocalAddress -eq "::" -or
    $_.LocalAddress -eq "::1"
  }

if (-not $listenConnections) {
  exit 0
}

$owningProcesses = $listenConnections |
  Select-Object -ExpandProperty OwningProcess -Unique |
  Where-Object { $_ -gt 0 }

foreach ($processId in $owningProcesses) {
  $processInfo = Get-ProcessInfo -ProcessId $processId
  if ($null -eq $processInfo) {
    continue
  }

  $commandLine = [string]$processInfo.CommandLine
  $normalizedCommandLine = $commandLine.ToLowerInvariant()
  $isRepoVite = $normalizedCommandLine.Contains($resolvedRepoRoot) -and
    ($normalizedCommandLine.Contains("vite") -or $normalizedCommandLine.Contains("vite.js"))

  if (-not $isRepoVite) {
    throw "Port $Port is already used by PID $processId and it is not an ARO Vite dev server: $commandLine"
  }

  Write-Host "Stopping stale ARO Vite dev server on port $Port (PID $processId)"
  Stop-Process -Id $processId -Force -ErrorAction Stop
}

Get-Process aro-desktop -ErrorAction SilentlyContinue | ForEach-Object {
  Write-Host "Stopping stale aro-desktop process (PID $($_.Id))"
  Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}

for ($attempt = 0; $attempt -lt 30; $attempt += 1) {
  Start-Sleep -Milliseconds 100
  $stillListening = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
  if (-not $stillListening) {
    exit 0
  }
}

throw "Port $Port is still busy after stopping stale ARO Vite processes."
