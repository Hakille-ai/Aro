param(
  [ValidateSet("ollama", "llama-cpp")]
  [string]$Runtime = "ollama",
  [string]$Model = "gemma3:1b",
  [switch]$SkipText,
  [switch]$Voice,
  [ValidateSet("all", "wake-word", "whisper", "piper")]
  [string[]]$VoiceSlots = @(),
  [string]$VoiceRoot = "",
  [string]$WhisperRelease = "v1.9.1",
  [string]$WhisperModel = "ggml-base.bin",
  [string]$WhisperModelUrl = "",
  [string]$PiperRelease = "2023.11.14-2",
  [string]$PiperVoice = "fr_FR-upmc-medium",
  [string]$PiperVoiceBaseUrl = "https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/upmc/medium",
  [string]$WakeWordName = "aro-whisper-gate",
  [string]$WakeWordModelUrl = "",
  [string]$WakeWordModelFile = "",
  [switch]$NoExtract
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($VoiceRoot)) {
  $VoiceRoot = Join-Path $root "vendor\voice"
}

function Write-Step {
  param([Parameter(Mandatory = $true)][string]$Message)
  Write-Host "[models] $Message" -ForegroundColor Cyan
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
      throw "Existing download is too small ($($existing.Length) bytes): $Path. Delete it and rerun the script if the download was interrupted."
    }
    Write-Step "exists $Path"
    return $existing.FullName
  }

  $parent = Split-Path -Parent $Path
  if (-not [string]::IsNullOrWhiteSpace($parent)) {
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
  }

  Write-Step "downloading $Url"
  Invoke-WebRequest -Uri $Url -OutFile $Path -UseBasicParsing
  $downloaded = Get-Item -LiteralPath $Path
  if ($downloaded.Length -lt $MinBytes) {
    throw "Downloaded file is too small ($($downloaded.Length) bytes): $Path"
  }
  return $downloaded.FullName
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
  return (Resolve-Path -LiteralPath $Path).Path
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
    # Fall through to the explicit fallback.
  }
  return $Fallback
}

function Get-VoiceSlotSelection {
  if ($Voice -and $VoiceSlots.Count -eq 0) {
    return @("wake-word", "whisper", "piper")
  }
  if ($VoiceSlots -contains "all") {
    return @("wake-word", "whisper", "piper")
  }
  return @($VoiceSlots)
}

function Write-JsonFile {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][object]$Value
  )

  $parent = Split-Path -Parent $Path
  if (-not [string]::IsNullOrWhiteSpace($parent)) {
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
  }
  $json = $Value | ConvertTo-Json -Depth 20
  [System.IO.File]::WriteAllText($Path, $json, [System.Text.UTF8Encoding]::new($false))
}

function Install-WakeWordSlot {
  $wakeDir = Join-Path $VoiceRoot "models\wake-word\$WakeWordName"
  New-Item -ItemType Directory -Force -Path $wakeDir | Out-Null

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
    $modelPath = Join-Path $wakeDir $WakeWordModelFile
    Download-IfMissing $WakeWordModelUrl $modelPath 1024 | Out-Null
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
    $modelPath = Join-Path $wakeDir $WakeWordModelFile
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

  $manifestPath = Join-Path $wakeDir "slot.json"
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
  Write-Step "wake-word slot manifest written: $manifestPath"

  return [pscustomobject]@{
    slot = "wake-word"
    manifestPath = (Resolve-Path -LiteralPath $manifestPath).Path
    modelPath = $modelPath
    status = $status
    engine = $engine
  }
}

function Install-WhisperSlot {
  $downloads = Join-Path $VoiceRoot "downloads"
  $runtimeDir = Join-Path $VoiceRoot "runtimes\whisper.cpp"
  $modelDir = Join-Path $VoiceRoot "models\whisper"
  New-Item -ItemType Directory -Force -Path $downloads,$runtimeDir,$modelDir | Out-Null

  $whisperZip = Join-Path $downloads "whisper-bin-x64-$WhisperRelease.zip"
  $modelPath = Join-Path $modelDir $WhisperModel

  if ([string]::IsNullOrWhiteSpace($WhisperModelUrl)) {
    $WhisperModelUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$WhisperModel"
  }

  Download-IfMissing "https://github.com/ggml-org/whisper.cpp/releases/download/$WhisperRelease/whisper-bin-x64.zip" $whisperZip 1000000 | Out-Null
  Download-IfMissing $WhisperModelUrl $modelPath 10000000 | Out-Null

  if (-not $NoExtract) {
    Write-Step "extracting Whisper.cpp runtime"
    Expand-Archive -Path $whisperZip -DestinationPath $runtimeDir -Force
  }

  $binary = Join-Path $runtimeDir "Release\whisper-cli.exe"
  if (-not $NoExtract) {
    $binary = Assert-File $binary "Whisper binary" 1024
    $whisperRuntime = Split-Path -Parent $binary
    Assert-File (Join-Path $whisperRuntime "whisper.dll") "Whisper DLL" 1024 | Out-Null
    Assert-File (Join-Path $whisperRuntime "ggml.dll") "Whisper ggml DLL" 1024 | Out-Null
  }
  $modelPath = Assert-File $modelPath "Whisper model" 10000000

  $runtimePath = $null
  if (-not $NoExtract) {
    $runtimePath = $binary
  }

  return [pscustomobject]@{
    slot = "stt-whisper"
    runtimePath = $runtimePath
    modelPath = $modelPath
    status = "installed"
    engine = "whisper.cpp"
  }
}

function Install-PiperSlot {
  $downloads = Join-Path $VoiceRoot "downloads"
  $runtimeDir = Join-Path $VoiceRoot "runtimes\piper"
  $modelDir = Join-Path $VoiceRoot "models\piper\$PiperVoice"
  New-Item -ItemType Directory -Force -Path $downloads,$runtimeDir,$modelDir | Out-Null

  $piperZip = Join-Path $downloads "piper_windows_amd64-$PiperRelease.zip"
  $voicePath = Join-Path $modelDir "$PiperVoice.onnx"
  $voiceConfigPath = Join-Path $modelDir "$PiperVoice.onnx.json"

  Download-IfMissing "https://github.com/rhasspy/piper/releases/download/$PiperRelease/piper_windows_amd64.zip" $piperZip 1000000 | Out-Null
  Download-IfMissing "$PiperVoiceBaseUrl/$PiperVoice.onnx" $voicePath 10000000 | Out-Null
  Download-IfMissing "$PiperVoiceBaseUrl/$PiperVoice.onnx.json" $voiceConfigPath 100 | Out-Null

  if (-not $NoExtract) {
    Write-Step "extracting Piper runtime"
    Expand-Archive -Path $piperZip -DestinationPath $runtimeDir -Force
  }

  $binary = Join-Path $runtimeDir "piper\piper.exe"
  if (-not $NoExtract) {
    $binary = Assert-File $binary "Piper binary" 1024
    $piperRuntime = Split-Path -Parent $binary
    Assert-File (Join-Path $piperRuntime "onnxruntime.dll") "Piper onnxruntime DLL" 1024 | Out-Null
    Assert-File (Join-Path $piperRuntime "piper_phonemize.dll") "Piper phonemizer DLL" 1024 | Out-Null
    Assert-Directory (Join-Path $piperRuntime "espeak-ng-data") "Piper espeak-ng-data" | Out-Null
  }

  $voicePath = Assert-File $voicePath "Piper voice model" 10000000
  $voiceConfigPath = Assert-File $voiceConfigPath "Piper voice config" 100

  $runtimePath = $null
  if (-not $NoExtract) {
    $runtimePath = $binary
  }

  return [pscustomobject]@{
    slot = "tts-piper"
    runtimePath = $runtimePath
    modelPath = $voicePath
    configPath = $voiceConfigPath
    status = "installed"
    engine = "piper"
  }
}

function Write-VoiceSlotsManifest {
  param([Parameter(Mandatory = $true)][object[]]$Slots)

  if ($Slots.Count -eq 0) {
    return
  }

  $manifestPath = Join-Path $VoiceRoot "models\voice-slots.json"
  $manifest = [ordered]@{
    version = 1
    localOnly = $true
    root = (Resolve-Path -LiteralPath $VoiceRoot).Path
    slots = $Slots
    privacy = [ordered]@{
      deviceLocalPaths = $true
      rawAudioStored = $false
      userTranscriptStored = $false
      cloudRequired = $false
    }
  }
  Write-JsonFile $manifestPath $manifest
  Write-Step "voice slots manifest written: $manifestPath"
}

if (-not $SkipText) {
  if ($Runtime -eq "ollama") {
    if (-not (Get-Command ollama -ErrorAction SilentlyContinue)) {
      throw "Ollama is not installed. Install it or run with -Runtime llama-cpp for manual guidance."
    }

    Write-Host "Pulling local model $Model through Ollama..."
    ollama pull $Model
    Write-Host "Done. Start Ollama and select provider 'Ollama' in ARO settings."
  } else {
    Write-Host "For llama.cpp, download a Gemma GGUF model manually from an official or trusted model repository."
    Write-Host "Then start llama-server with an OpenAI-compatible endpoint on 127.0.0.1."
    Write-Host "Example:"
    Write-Host "  llama-server -m C:\models\gemma.gguf --host 127.0.0.1 --port 8080"
  }
}

$selectedVoiceSlots = Get-VoiceSlotSelection
if ($selectedVoiceSlots.Count -gt 0) {
  New-Item -ItemType Directory -Force -Path $VoiceRoot | Out-Null
  $installedSlots = @()
  foreach ($slot in $selectedVoiceSlots) {
    switch ($slot) {
      "wake-word" { $installedSlots += Install-WakeWordSlot }
      "whisper" { $installedSlots += Install-WhisperSlot }
      "piper" { $installedSlots += Install-PiperSlot }
      default { throw "Unsupported voice slot: $slot" }
    }
  }
  Write-VoiceSlotsManifest $installedSlots
  Write-Host "Voice model slots prepared: $($installedSlots.slot -join ', ')" -ForegroundColor Green
}
