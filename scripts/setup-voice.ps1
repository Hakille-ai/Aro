param(
  [string]$WhisperRelease = "v1.9.1",
  [string]$PiperRelease = "2023.11.14-2",
  [string]$WhisperModel = "ggml-base.bin",
  [string]$PiperVoice = "fr_FR-upmc-medium",
  [string]$PiperVoiceBaseUrl = "https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/upmc/medium",
  [string]$WakeWordName = "aro-whisper-gate",
  [string]$WakeWordModelUrl = "",
  [string]$WakeWordModelFile = "",
  [string]$SettingsPath = "",
  [switch]$RunSmoke,
  [switch]$ShowPaths
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$voiceRoot = Join-Path $root "vendor\voice"
$downloads = Join-Path $voiceRoot "downloads"
$whisperRuntime = Join-Path $voiceRoot "runtimes\whisper.cpp"
$piperRuntime = Join-Path $voiceRoot "runtimes\piper"
$whisperModelDir = Join-Path $voiceRoot "models\whisper"
$piperModelDir = Join-Path $voiceRoot "models\piper\$PiperVoice"
$wakeWordModelDir = Join-Path $voiceRoot "models\wake-word\$WakeWordName"

New-Item -ItemType Directory -Force -Path $downloads,$whisperRuntime,$piperRuntime,$whisperModelDir,$piperModelDir,$wakeWordModelDir | Out-Null

function Write-Step {
  param([Parameter(Mandatory = $true)][string]$Message)
  Write-Host "[voice] $Message" -ForegroundColor Cyan
}

function Download-IfMissing {
  param(
    [Parameter(Mandatory = $true)][string]$Url,
    [Parameter(Mandatory = $true)][string]$Path,
    [int64]$MinBytes = 1
  )

  if (Test-Path -LiteralPath $Path -PathType Leaf) {
    $existing = Get-Item -LiteralPath $Path
    if ($existing.Length -lt $MinBytes) {
      throw "Existing download is too small ($($existing.Length) bytes): $Path. Delete it and rerun setup if the download was interrupted."
    }
    Write-Step "exists $Path"
    return
  }

  Write-Step "downloading $Url"
  Invoke-WebRequest -Uri $Url -OutFile $Path -UseBasicParsing
  $downloaded = Get-Item -LiteralPath $Path
  if ($downloaded.Length -lt $MinBytes) {
    throw "Downloaded file is too small ($($downloaded.Length) bytes): $Path"
  }
}

function Assert-File {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label,
    [int64]$MinBytes = 1
  )

  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Label missing: $Path"
  }

  $item = Get-Item -LiteralPath $Path
  if ($item.Length -lt $MinBytes) {
    throw "$Label is unexpectedly small ($($item.Length) bytes): $Path"
  }

  Write-Step "$Label ok ($($item.Length) bytes)"
  return $item.FullName
}

function Assert-Directory {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label
  )

  if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
    throw "$Label missing: $Path"
  }

  Write-Step "$Label ok"
}

function Set-JsonProperty {
  param(
    [Parameter(Mandatory = $true)][psobject]$Object,
    [Parameter(Mandatory = $true)][string]$Name,
    [AllowNull()][object]$Value
  )

  if ($Object.PSObject.Properties.Name -contains $Name) {
    $Object.$Name = $Value
  } else {
    $Object | Add-Member -NotePropertyName $Name -NotePropertyValue $Value
  }
}

function Get-FileNameFromUrl {
  param(
    [Parameter(Mandatory = $true)][string]$Url,
    [Parameter(Mandatory = $true)][string]$Fallback
  )

  try {
    $uri = [System.Uri]$Url
    $name = Split-Path -Leaf $uri.AbsolutePath
    if (-not [string]::IsNullOrWhiteSpace($name)) {
      return $name
    }
  } catch {
    # Fall through to fallback.
  }

  return $Fallback
}

function Write-JsonFile {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][object]$Value
  )

  $json = $Value | ConvertTo-Json -Depth 20
  [System.IO.File]::WriteAllText($Path, $json, [System.Text.UTF8Encoding]::new($false))
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

function Get-DefaultSettingsPath {
  if ([string]::IsNullOrWhiteSpace($env:APPDATA)) {
    throw "APPDATA is not set; pass -SettingsPath explicitly."
  }
  return (Join-Path $env:APPDATA "ARO\ARO\data\settings.json")
}

function Install-WakeWordSlot {
  $modelPath = $null
  $status = "installed"
  $engine = "deterministic-local-gate"
  $notes = @(
    "Default local wake-word gate filters silence and quiet noise before local Whisper verifies the ARO phrase.",
    "Pass -WakeWordModelUrl to replace this lightweight gate with a dedicated on-device wake-word model."
  )

  if (-not [string]::IsNullOrWhiteSpace($WakeWordModelUrl)) {
    if ([string]::IsNullOrWhiteSpace($WakeWordModelFile)) {
      $WakeWordModelFile = Get-FileNameFromUrl $WakeWordModelUrl "wake-word.onnx"
    }

    $modelPath = Join-Path $wakeWordModelDir $WakeWordModelFile
    Download-IfMissing $WakeWordModelUrl $modelPath 1024
    $modelPath = Assert-File $modelPath "wake-word model" 1024
    $status = "installed"
    $engine = "external-local-wake-word"
    $notes = @(
      "Dedicated wake-word model file is present locally.",
      "The current desktop app still needs a source-code adapter before it can use this model directly."
    )
  } else {
    if ([string]::IsNullOrWhiteSpace($WakeWordModelFile)) {
      $WakeWordModelFile = "aro-energy-gate.model"
    }
    $modelPath = Join-Path $wakeWordModelDir $WakeWordModelFile
    if (-not (Test-Path -LiteralPath $modelPath -PathType Leaf)) {
      @(
        "# ARO local wake-word gate model"
        "# Compatible with aro-voice deterministic local wake-word scorer."
        "gain=1.0"
        "bias=0.0"
      ) | Set-Content -LiteralPath $modelPath -Encoding UTF8
    }
    $modelPath = Assert-File $modelPath "wake-word gate model" 32
  }

  $manifestPath = Join-Path $wakeWordModelDir "slot.json"
  $manifest = [ordered]@{
    slot = "wake-word"
    name = $WakeWordName
    status = $status
    engine = $engine
    localOnly = $true
    modelPath = $modelPath
    wakePhrases = @("ARO", "hey ARO", "ok ARO", "bonjour ARO", "salut ARO")
    privacy = [ordered]@{
      rawAudioStored = $false
      transcriptStored = $false
      cloudRequired = $false
    }
    notes = $notes
  }
  Write-JsonFile $manifestPath $manifest
  Write-Step "wake-word slot manifest written: $(ConvertTo-OutputPath $manifestPath)"

  return [pscustomobject]@{
    slot = "wake-word"
    manifestPath = (Resolve-Path -LiteralPath $manifestPath).Path
    modelPath = $modelPath
    status = $status
    engine = $engine
  }
}

function Write-VoiceSlotsManifest {
  param([Parameter(Mandatory = $true)][object[]]$Slots)

  $manifestPath = Join-Path $voiceRoot "models\voice-slots.json"
  $manifest = [ordered]@{
    version = 1
    localOnly = $true
    root = (Resolve-Path -LiteralPath $voiceRoot).Path
    slots = $Slots
    privacy = [ordered]@{
      deviceLocalPaths = $true
      rawAudioStored = $false
      userTranscriptStored = $false
      cloudRequired = $false
    }
  }
  Write-JsonFile $manifestPath $manifest
  Write-Step "voice slots manifest written: $(ConvertTo-OutputPath $manifestPath)"
}

$whisperZip = Join-Path $downloads "whisper-bin-x64-$WhisperRelease.zip"
$piperZip = Join-Path $downloads "piper_windows_amd64-$PiperRelease.zip"
$whisperModelPath = Join-Path $whisperModelDir $WhisperModel
$piperVoicePath = Join-Path $piperModelDir "$PiperVoice.onnx"
$piperVoiceConfigPath = Join-Path $piperModelDir "$PiperVoice.onnx.json"

$wakeWordSlot = Install-WakeWordSlot
Download-IfMissing "https://github.com/ggml-org/whisper.cpp/releases/download/$WhisperRelease/whisper-bin-x64.zip" $whisperZip 1000000
Download-IfMissing "https://github.com/rhasspy/piper/releases/download/$PiperRelease/piper_windows_amd64.zip" $piperZip 1000000
Download-IfMissing "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$WhisperModel" $whisperModelPath 10000000
Download-IfMissing "$PiperVoiceBaseUrl/$PiperVoice.onnx" $piperVoicePath 10000000
Download-IfMissing "$PiperVoiceBaseUrl/$PiperVoice.onnx.json" $piperVoiceConfigPath 100

Write-Step "extracting Whisper.cpp runtime"
Expand-Archive -Path $whisperZip -DestinationPath $whisperRuntime -Force
Write-Step "extracting Piper runtime"
Expand-Archive -Path $piperZip -DestinationPath $piperRuntime -Force

$whisperBinary = Join-Path $whisperRuntime "Release\whisper-cli.exe"
$piperBinary = Join-Path $piperRuntime "piper\piper.exe"
$piperRuntimeDir = Split-Path -Parent $piperBinary
$whisperRuntimeDir = Split-Path -Parent $whisperBinary

$whisperBinary = Assert-File $whisperBinary "Whisper binary" 1024
$whisperModelPath = Assert-File $whisperModelPath "Whisper model" 10000000
Assert-File (Join-Path $whisperRuntimeDir "whisper.dll") "Whisper DLL" 1024 | Out-Null
Assert-File (Join-Path $whisperRuntimeDir "ggml.dll") "Whisper ggml DLL" 1024 | Out-Null

$piperBinary = Assert-File $piperBinary "Piper binary" 1024
$piperVoicePath = Assert-File $piperVoicePath "Piper voice model" 10000000
Assert-File $piperVoiceConfigPath "Piper voice config" 100 | Out-Null
Assert-File (Join-Path $piperRuntimeDir "onnxruntime.dll") "Piper onnxruntime DLL" 1024 | Out-Null
Assert-File (Join-Path $piperRuntimeDir "piper_phonemize.dll") "Piper phonemizer DLL" 1024 | Out-Null
Assert-Directory (Join-Path $piperRuntimeDir "espeak-ng-data") "Piper espeak-ng-data"

Write-VoiceSlotsManifest @(
  $wakeWordSlot,
  [pscustomobject]@{
    slot = "stt-whisper"
    runtimePath = $whisperBinary
    modelPath = $whisperModelPath
    status = "installed"
    engine = "whisper.cpp"
  },
  [pscustomobject]@{
    slot = "tts-piper"
    runtimePath = $piperBinary
    modelPath = $piperVoicePath
    configPath = (Resolve-Path -LiteralPath $piperVoiceConfigPath).Path
    status = "installed"
    engine = "piper"
  }
)

if ([string]::IsNullOrWhiteSpace($SettingsPath)) {
  $SettingsPath = Get-DefaultSettingsPath
}

if (Test-Path -LiteralPath $SettingsPath) {
  $settings = Get-Content -Raw -LiteralPath $SettingsPath | ConvertFrom-Json
} else {
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $SettingsPath) | Out-Null
  $settings = [pscustomobject]@{
    model = [pscustomobject]@{
      provider = "mock"
      modelId = "gemma3:1b"
      ollamaEndpoint = "http://127.0.0.1:11434"
      llamaCppEndpoint = "http://127.0.0.1:8080"
      temperature = 0.7
      maxTokens = 768
    }
    voice = [pscustomobject]@{}
    retainHistory = $true
    speakResponses = $false
  }
}

Set-JsonProperty $settings "voice" ([pscustomobject]@{
  enabled = $true
  speechToText = "whisper-cpp"
  textToSpeech = "piper"
  whisperBinary = (Resolve-Path -LiteralPath $whisperBinary).Path
  whisperModelPath = (Resolve-Path -LiteralPath $whisperModelPath).Path
  piperBinary = (Resolve-Path -LiteralPath $piperBinary).Path
  piperVoicePath = (Resolve-Path -LiteralPath $piperVoicePath).Path
  wakeWord = [pscustomobject]@{
    enabled = $true
    runtime = "local-model"
    modelPath = (Resolve-Path -LiteralPath $wakeWordSlot.modelPath).Path
    threshold = 0.25
  }
})
Set-JsonProperty $settings "speakResponses" $true

$settingsJson = $settings | ConvertTo-Json -Depth 20
[System.IO.File]::WriteAllText($SettingsPath, $settingsJson, [System.Text.UTF8Encoding]::new($false))

Write-Host "ARO voice is configured." -ForegroundColor Green
Write-Host "Settings: $(ConvertTo-OutputPath $SettingsPath)"
Write-Host "Wake:     $(ConvertTo-OutputPath $wakeWordSlot.manifestPath)"
Write-Host "Whisper:  $(ConvertTo-OutputPath $settings.voice.whisperBinary)"
Write-Host "Model:    $(ConvertTo-OutputPath $settings.voice.whisperModelPath)"
Write-Host "Piper:    $(ConvertTo-OutputPath $settings.voice.piperBinary)"
Write-Host "Voice:    $(ConvertTo-OutputPath $settings.voice.piperVoicePath)"

if ($RunSmoke) {
  Write-Step "running Piper -> Whisper smoke test"
  & (Join-Path $PSScriptRoot "smoke-voice.ps1") `
    -WhisperBinary $settings.voice.whisperBinary `
    -WhisperModelPath $settings.voice.whisperModelPath `
    -PiperBinary $settings.voice.piperBinary `
    -PiperVoicePath $settings.voice.piperVoicePath `
    -ShowPaths:$ShowPaths
} else {
  Write-Host "Optional smoke test: powershell -ExecutionPolicy Bypass -File scripts\smoke-voice.ps1" -ForegroundColor Yellow
}
