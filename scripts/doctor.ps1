param(
  [string]$ApiBaseUrl = "http://127.0.0.1:8710",
  [string]$PostgresContainer = "aro-postgres",
  [string]$MinioContainer = "aro-minio",
  [string]$RedisContainer = "aro-redis",
  [string]$SettingsPath = "",
  [string]$VoiceRoot = "",
  [switch]$VoiceSmoke,
  [switch]$ShowPaths
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($VoiceRoot)) {
  $VoiceRoot = Join-Path $root "vendor\voice"
}

Write-Host "ARO developer doctor" -ForegroundColor Cyan

$script:Failures = 0
$script:Warnings = 0
$script:VoiceSettings = $null

function Pass($Message) {
  Write-Host "[ok] $Message" -ForegroundColor Green
}

function Warn($Message) {
  $script:Warnings += 1
  Write-Host "[warn] $Message" -ForegroundColor Yellow
}

function Info($Message) {
  Write-Host "[info] $Message" -ForegroundColor DarkGray
}

function Fail($Message) {
  $script:Failures += 1
  Write-Host "[fail] $Message" -ForegroundColor Red
}

function Check-Command {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [switch]$Required
  )

  $cmd = Get-Command $Name -ErrorAction SilentlyContinue
  if ($null -eq $cmd) {
    if ($Required) {
      Fail "missing required command: $Name"
    } else {
      Warn "optional command not found: $Name"
    }
    return $false
  }

  Pass "found ${Name}: $($cmd.Source)"
  return $true
}

function Check-Version {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][scriptblock]$Command,
    [string]$MinimumMajor
  )

  try {
    $version = (& $Command) -join " "
    if ([string]::IsNullOrWhiteSpace($version)) {
      Warn "$Name version command returned no output"
      return
    }
    Pass "$Name $version"
    if ($MinimumMajor -and ($version -notmatch [regex]::Escape($MinimumMajor))) {
      Warn "$Name should match project requirement around $MinimumMajor"
    }
  } catch {
    Warn "could not read $Name version: $($_.Exception.Message)"
  }
}

function Check-Secret {
  param(
    [Parameter(Mandatory = $true)][string]$Name
  )

  $value = [Environment]::GetEnvironmentVariable($Name)
  if ([string]::IsNullOrWhiteSpace($value)) {
    Warn "$Name is not set in this shell"
    return
  }

  $lower = $value.Trim().ToLowerInvariant()
  if ($value.Trim().Length -lt 32 -or $lower.Contains("replace-with") -or $lower.Contains("change-me") -or $lower.Contains("placeholder")) {
    Fail "$Name is too short or still looks like a placeholder"
    return
  }

  Pass "$Name is present and passes local validation"
}

function Resolve-AroPath {
  param([string]$Path)

  if ([string]::IsNullOrWhiteSpace($Path)) {
    return $null
  }
  if ([System.IO.Path]::IsPathRooted($Path)) {
    return $Path
  }
  return (Join-Path $root $Path)
}

function ConvertTo-OutputPath {
  param([AllowNull()][string]$Path)

  if ([string]::IsNullOrWhiteSpace($Path)) {
    return $null
  }

  $full = [System.IO.Path]::GetFullPath($Path)
  if ($ShowPaths) {
    return $full
  }

  if ($full.StartsWith($root, [System.StringComparison]::OrdinalIgnoreCase)) {
    return $full.Substring($root.Length).TrimStart([char[]]@("\", "/"))
  }

  return "<device-local path; rerun with -ShowPaths>"
}

function Check-FilePath {
  param(
    [string]$Path,
    [Parameter(Mandatory = $true)][string]$Label,
    [int64]$MinBytes = 1,
    [switch]$Required
  )

  $resolved = Resolve-AroPath $Path
  if ([string]::IsNullOrWhiteSpace($resolved)) {
    if ($Required) {
      Fail "$Label path is not configured"
    } else {
      Warn "$Label path is not configured"
    }
    return $null
  }

  if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
    if ($Required) {
      Fail "$Label not found: $(ConvertTo-OutputPath $resolved)"
    } else {
      Warn "$Label not found: $(ConvertTo-OutputPath $resolved)"
    }
    return $null
  }

  $item = Get-Item -LiteralPath $resolved
  if ($item.Length -lt $MinBytes) {
    if ($Required) {
      Fail "$Label is unexpectedly small ($($item.Length) bytes): $(ConvertTo-OutputPath $resolved)"
    } else {
      Warn "$Label is unexpectedly small ($($item.Length) bytes): $(ConvertTo-OutputPath $resolved)"
    }
    return $item.FullName
  }

  Pass "$Label exists ($($item.Length) bytes): $(ConvertTo-OutputPath $item.FullName)"
  return $item.FullName
}

function Check-DirectoryPath {
  param(
    [string]$Path,
    [Parameter(Mandatory = $true)][string]$Label,
    [switch]$Required
  )

  $resolved = Resolve-AroPath $Path
  if ([string]::IsNullOrWhiteSpace($resolved) -or -not (Test-Path -LiteralPath $resolved -PathType Container)) {
    if ($Required) {
      Fail "$Label not found: $(ConvertTo-OutputPath $resolved)"
    } else {
      Warn "$Label not found: $(ConvertTo-OutputPath $resolved)"
    }
    return $false
  }

  Pass "$Label exists: $(ConvertTo-OutputPath $resolved)"
  return $true
}

function Get-DefaultSettingsPath {
  if ([string]::IsNullOrWhiteSpace($env:APPDATA)) {
    return $null
  }
  return (Join-Path $env:APPDATA "ARO\ARO\data\settings.json")
}

function Get-VoiceDefaults {
  [pscustomobject]@{
    wakeWordManifestPath = Join-Path $VoiceRoot "models\wake-word\aro-whisper-gate\slot.json"
    voiceSlotsManifestPath = Join-Path $VoiceRoot "models\voice-slots.json"
    whisperBinary = Join-Path $VoiceRoot "runtimes\whisper.cpp\Release\whisper-cli.exe"
    whisperModelPath = Join-Path $VoiceRoot "models\whisper\ggml-base.bin"
    piperBinary = Join-Path $VoiceRoot "runtimes\piper\piper\piper.exe"
    piperVoicePath = Join-Path $VoiceRoot "models\piper\fr_FR-upmc-medium\fr_FR-upmc-medium.onnx"
    piperVoiceConfigPath = Join-Path $VoiceRoot "models\piper\fr_FR-upmc-medium\fr_FR-upmc-medium.onnx.json"
  }
}

function Check-VoiceSlotManifest {
  param([Parameter(Mandatory = $true)][object]$Defaults)

  $wakeManifest = Check-FilePath $Defaults.wakeWordManifestPath "default wake-word slot manifest" 100
  if ($wakeManifest) {
    try {
      $wakeSlot = Get-Content -Raw -LiteralPath $wakeManifest | ConvertFrom-Json
      if ($wakeSlot.slot -eq "wake-word") {
        Pass "wake-word slot is declared ($($wakeSlot.status), $($wakeSlot.engine))"
      } else {
        Warn "wake-word slot manifest has unexpected slot value: $($wakeSlot.slot)"
      }

      if ($wakeSlot.status -eq "reserved") {
        Warn "wake-word slot is reserved; current desktop wake-word mode uses local Whisper phrase matching"
      }
    } catch {
      Fail "wake-word slot manifest is not valid JSON: $(ConvertTo-OutputPath $wakeManifest) ($($_.Exception.Message))"
    }
  }

  $voiceSlotsManifest = Check-FilePath $Defaults.voiceSlotsManifestPath "voice slots manifest" 100
  if (-not $voiceSlotsManifest) {
    return
  }

  try {
    $manifest = Get-Content -Raw -LiteralPath $voiceSlotsManifest | ConvertFrom-Json
    $slotNames = @($manifest.slots | ForEach-Object { $_.slot })
    foreach ($requiredSlot in @("wake-word", "stt-whisper", "tts-piper")) {
      if ($slotNames -contains $requiredSlot) {
        Pass "voice slot listed: $requiredSlot"
      } else {
        Warn "voice slots manifest does not list $requiredSlot"
      }
    }
  } catch {
    Fail "voice slots manifest is not valid JSON: $(ConvertTo-OutputPath $voiceSlotsManifest) ($($_.Exception.Message))"
  }
}

function Check-VoiceRuntime {
  if ([string]::IsNullOrWhiteSpace($SettingsPath)) {
    $SettingsPath = Get-DefaultSettingsPath
  }

  $defaults = Get-VoiceDefaults
  Check-VoiceSlotManifest $defaults
  Check-FilePath $defaults.whisperBinary "default Whisper binary" 1024 | Out-Null
  Check-FilePath $defaults.whisperModelPath "default Whisper model" 10000000 | Out-Null
  Check-FilePath $defaults.piperBinary "default Piper binary" 1024 | Out-Null
  Check-FilePath $defaults.piperVoicePath "default Piper voice model" 10000000 | Out-Null
  Check-FilePath $defaults.piperVoiceConfigPath "default Piper voice config" 100 | Out-Null

  if ([string]::IsNullOrWhiteSpace($SettingsPath)) {
    Warn "could not resolve ARO settings path because APPDATA is not set"
    return
  }

  if (-not (Test-Path -LiteralPath $SettingsPath -PathType Leaf)) {
    Warn "ARO settings file not found: $(ConvertTo-OutputPath $SettingsPath). Run scripts\setup-voice.ps1 or launch the desktop app once."
    return
  }

  try {
    $settings = Get-Content -Raw -LiteralPath $SettingsPath | ConvertFrom-Json
  } catch {
    Fail "ARO settings file is not valid JSON: $(ConvertTo-OutputPath $SettingsPath) ($($_.Exception.Message))"
    return
  }

  Pass "ARO settings file is readable: $(ConvertTo-OutputPath $SettingsPath)"
  if ($null -eq $settings.voice) {
    Warn "settings.json has no voice block"
    return
  }

  $script:VoiceSettings = $settings.voice
  $voiceEnabled = $settings.voice.enabled -eq $true
  $speechToText = [string]$settings.voice.speechToText
  $textToSpeech = [string]$settings.voice.textToSpeech

  if ($voiceEnabled) {
    Pass "voice is enabled in settings"
  } else {
    Warn "voice is disabled in settings"
  }

  if ($speechToText -eq "whisper-cpp") {
    $whisperBinary = Check-FilePath $settings.voice.whisperBinary "configured Whisper binary" 1024 -Required:$voiceEnabled
    Check-FilePath $settings.voice.whisperModelPath "configured Whisper model" 10000000 -Required:$voiceEnabled | Out-Null
    if ($whisperBinary) {
      $whisperDir = Split-Path -Parent $whisperBinary
      Check-FilePath (Join-Path $whisperDir "whisper.dll") "configured Whisper DLL" 1024 -Required:$voiceEnabled | Out-Null
      Check-FilePath (Join-Path $whisperDir "ggml.dll") "configured Whisper ggml DLL" 1024 -Required:$voiceEnabled | Out-Null
    }
  } elseif ($speechToText -eq "disabled" -or [string]::IsNullOrWhiteSpace($speechToText)) {
    Warn "speech-to-text is disabled"
  } else {
    Warn "unknown speech-to-text runtime in settings: $speechToText"
  }

  if ($textToSpeech -eq "piper") {
    $piperBinary = Check-FilePath $settings.voice.piperBinary "configured Piper binary" 1024 -Required:$voiceEnabled
    $piperVoice = Check-FilePath $settings.voice.piperVoicePath "configured Piper voice model" 10000000 -Required:$voiceEnabled
    if ($piperVoice) {
      Check-FilePath "$piperVoice.json" "configured Piper voice config" 100 -Required:$voiceEnabled | Out-Null
    }
    if ($piperBinary) {
      $piperDir = Split-Path -Parent $piperBinary
      Check-FilePath (Join-Path $piperDir "onnxruntime.dll") "configured Piper onnxruntime DLL" 1024 -Required:$voiceEnabled | Out-Null
      Check-FilePath (Join-Path $piperDir "piper_phonemize.dll") "configured Piper phonemizer DLL" 1024 -Required:$voiceEnabled | Out-Null
      Check-DirectoryPath (Join-Path $piperDir "espeak-ng-data") "configured Piper espeak-ng-data" -Required:$voiceEnabled | Out-Null
    }
  } elseif ($textToSpeech -eq "disabled" -or [string]::IsNullOrWhiteSpace($textToSpeech)) {
    if ($settings.speakResponses -eq $true) {
      Warn "speakResponses is true but text-to-speech is disabled"
    } else {
      Warn "text-to-speech is disabled"
    }
  } else {
    Warn "unknown text-to-speech runtime in settings: $textToSpeech"
  }

  if ($null -eq $settings.voice.wakeWord) {
    Warn "wake-word settings block is missing"
  } else {
    $wakeWordEnabled = $settings.voice.wakeWord.enabled -eq $true
    $wakeWordRuntime = [string]$settings.voice.wakeWord.runtime
    if ($wakeWordEnabled -and $wakeWordRuntime -eq "local-model") {
      Check-FilePath $settings.voice.wakeWord.modelPath "configured wake-word model" 32 -Required:$voiceEnabled | Out-Null
      $threshold = [double]$settings.voice.wakeWord.threshold
      if ($threshold -ge 0 -and $threshold -le 1) {
        Pass "wake-word threshold is valid ($threshold)"
      } else {
        Fail "wake-word threshold must be between 0 and 1"
      }
    } elseif ($wakeWordEnabled) {
      Warn "unknown wake-word runtime in settings: $wakeWordRuntime"
    } else {
      Warn "wake-word is disabled in settings"
    }
  }
}

function Invoke-VoiceSmoke {
  $smokeScript = Join-Path $PSScriptRoot "smoke-voice.ps1"
  if (-not (Test-Path -LiteralPath $smokeScript -PathType Leaf)) {
    Fail "voice smoke script not found: $smokeScript"
    return
  }

  $paths = Get-VoiceDefaults
  if ($null -ne $script:VoiceSettings) {
    if (-not [string]::IsNullOrWhiteSpace($script:VoiceSettings.whisperBinary)) {
      $paths.whisperBinary = Resolve-AroPath $script:VoiceSettings.whisperBinary
    }
    if (-not [string]::IsNullOrWhiteSpace($script:VoiceSettings.whisperModelPath)) {
      $paths.whisperModelPath = Resolve-AroPath $script:VoiceSettings.whisperModelPath
    }
    if (-not [string]::IsNullOrWhiteSpace($script:VoiceSettings.piperBinary)) {
      $paths.piperBinary = Resolve-AroPath $script:VoiceSettings.piperBinary
    }
    if (-not [string]::IsNullOrWhiteSpace($script:VoiceSettings.piperVoicePath)) {
      $paths.piperVoicePath = Resolve-AroPath $script:VoiceSettings.piperVoicePath
    }
  }

  try {
    & $smokeScript `
      -WhisperBinary $paths.whisperBinary `
      -WhisperModelPath $paths.whisperModelPath `
      -PiperBinary $paths.piperBinary `
      -PiperVoicePath $paths.piperVoicePath `
      -ShowPaths:$ShowPaths | Write-Host
    Pass "Piper -> Whisper voice smoke completed"
  } catch {
    Fail "Piper -> Whisper voice smoke failed: $($_.Exception.Message)"
  }
}

$hasRustc = Check-Command "rustc" -Required
$hasCargo = Check-Command "cargo" -Required
$hasNode = Check-Command "node" -Required
$hasNpm = Check-Command "npm" -Required
$hasDocker = Check-Command "docker"
Check-Command "ollama" | Out-Null

if ($hasRustc) {
  Check-Version "rustc" { rustc --version } "1.95"
}
if ($hasNode) {
  Check-Version "node" { node --version } "v24"
}
if ($hasNpm) {
  Check-Version "npm" { npm --version } "11"
}

if ([Environment]::GetEnvironmentVariable("DATABASE_URL")) {
  Pass "DATABASE_URL is set"
} else {
  Warn "DATABASE_URL is not set; use postgres://aro:aro@127.0.0.1:5432/aro for local Docker"
}
if ([Environment]::GetEnvironmentVariable("ARO_REDIS_URL")) {
  Pass "ARO_REDIS_URL is set"
} else {
  Warn "ARO_REDIS_URL is not set; Redis-backed rate limiting and outbox streaming are disabled outside production"
}
Check-Secret "ARO_JWT_SECRET"
Check-Secret "ARO_SECRETS_KEY"
if ([Environment]::GetEnvironmentVariable("ARO_FILES_API")) {
  Pass "ARO_FILES_API is set"
} else {
  Warn "ARO_FILES_API is not set; set it to true for the scalable files API"
}

if ($hasDocker) {
  try {
    docker info *> $null
    Pass "Docker daemon is reachable"
  } catch {
    Warn "Docker command exists but daemon is not reachable: $($_.Exception.Message)"
  }

  try {
    $container = docker ps --filter "name=$PostgresContainer" --format "{{.Names}}"
    if ($container -contains $PostgresContainer) {
      Pass "PostgreSQL container '$PostgresContainer' is running"
      try {
        docker exec $PostgresContainer pg_isready -U aro -d aro *> $null
        Pass "PostgreSQL accepts connections"
      } catch {
        Fail "PostgreSQL container is running but pg_isready failed"
      }
    } else {
      Warn "PostgreSQL container '$PostgresContainer' is not running; start it with docker compose up -d postgres"
    }
  } catch {
    Warn "could not inspect Docker containers: $($_.Exception.Message)"
  }

  try {
    $redis = docker ps --filter "name=$RedisContainer" --format "{{.Names}}"
    if ($redis -contains $RedisContainer) {
      Pass "Redis container '$RedisContainer' is running"
      try {
        $redisPing = docker exec $RedisContainer redis-cli ping
        if ($redisPing -eq "PONG") {
          Pass "Redis accepts connections"
        } else {
          Fail "Redis ping returned unexpected response: $redisPing"
        }
      } catch {
        Fail "Redis container is running but redis-cli ping failed"
      }
    } else {
      Warn "Redis container '$RedisContainer' is not running; start it with docker compose up -d redis"
    }
  } catch {
    Warn "could not inspect Redis container: $($_.Exception.Message)"
  }

  try {
    $minio = docker ps --filter "name=$MinioContainer" --format "{{.Names}}"
    if ($minio -contains $MinioContainer) {
      Pass "MinIO container '$MinioContainer' is running"
      $bucket = [Environment]::GetEnvironmentVariable("ARO_S3_BUCKET")
      if ([string]::IsNullOrWhiteSpace($bucket)) {
        $bucket = "aro-files-dev"
      }
      try {
        docker run --rm --network aro_default minio/mc:latest sh -c "mc alias set aro-local http://aro-minio:9000 minioadmin minioadmin >/dev/null && mc ls aro-local/$bucket >/dev/null"
        Pass "MinIO bucket '$bucket' is reachable"
      } catch {
        Warn "MinIO is running but bucket '$bucket' was not reachable; run docker compose up -d minio minio-init"
      }
    } else {
      Warn "MinIO container '$MinioContainer' is not running; start it with docker compose up -d minio minio-init"
    }
  } catch {
    Warn "could not inspect MinIO container: $($_.Exception.Message)"
  }
}

Check-VoiceRuntime
if ($VoiceSmoke) {
  Invoke-VoiceSmoke
} else {
  Info "voice round-trip smoke was not run; use scripts\doctor.ps1 -VoiceSmoke or scripts\smoke-voice.ps1"
}

try {
  $health = Invoke-RestMethod -Uri "$ApiBaseUrl/health" -TimeoutSec 5
  if ($health.status -eq "ok") {
    Pass "ARO API health is ok at $ApiBaseUrl"
  } else {
    Warn "ARO API responded but health status was not ok"
  }
} catch {
  Warn "ARO API is not reachable at $ApiBaseUrl; start it with npm run api:dev"
}

if ($script:Failures -gt 0) {
  throw "doctor found $script:Failures failure(s) and $script:Warnings warning(s)"
}

Write-Host "doctor completed with $script:Warnings warning(s)" -ForegroundColor Cyan
