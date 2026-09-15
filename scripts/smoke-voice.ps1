param(
  [string]$WhisperBinary = "",
  [string]$WhisperModelPath = "",
  [string]$PiperBinary = "",
  [string]$PiperVoicePath = "",
  [string]$Text = "Bonjour, je suis ARO.",
  [string]$Language = "fr",
  [string[]]$ExpectedTokens = @("bonjour", "suis"),
  [string]$OutputFile = "",
  [int]$MinAudioBytes = 12000,
  [int]$PiperTimeoutSeconds = 60,
  [int]$WhisperTimeoutSeconds = 120,
  [switch]$IncludeTranscript,
  [switch]$KeepTranscriptArtifacts,
  [switch]$ShowPaths
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$voiceRoot = Join-Path $root "vendor\voice"

if ([string]::IsNullOrWhiteSpace($WhisperBinary)) {
  $WhisperBinary = Join-Path $voiceRoot "runtimes\whisper.cpp\Release\whisper-cli.exe"
}
if ([string]::IsNullOrWhiteSpace($WhisperModelPath)) {
  $WhisperModelPath = Join-Path $voiceRoot "models\whisper\ggml-base.bin"
}
if ([string]::IsNullOrWhiteSpace($PiperBinary)) {
  $PiperBinary = Join-Path $voiceRoot "runtimes\piper\piper\piper.exe"
}
if ([string]::IsNullOrWhiteSpace($PiperVoicePath)) {
  $PiperVoicePath = Join-Path $voiceRoot "models\piper\fr_FR-upmc-medium\fr_FR-upmc-medium.onnx"
}
if ([string]::IsNullOrWhiteSpace($OutputFile)) {
  $OutputFile = Join-Path $voiceRoot "voice-smoke.wav"
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

function ConvertTo-OutputText {
  param(
    [AllowNull()][string]$Value,
    [string[]]$KnownPaths = @()
  )

  if ([string]::IsNullOrWhiteSpace($Value)) {
    return $Value
  }

  $safe = $Value
  foreach ($knownPath in $KnownPaths) {
    if ([string]::IsNullOrWhiteSpace($knownPath)) {
      continue
    }

    $full = [System.IO.Path]::GetFullPath($knownPath)
    $safe = $safe.Replace($full, (ConvertTo-OutputPath $full))
  }

  if (-not $ShowPaths) {
    $safe = $safe.Replace($root, "<repo>")
  }

  return $safe
}

function Resolve-RequiredFile {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label,
    [int64]$MinBytes = 1
  )

  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Label not found: $(ConvertTo-OutputPath $Path)"
  }

  $item = Get-Item -LiteralPath $Path
  if ($item.Length -lt $MinBytes) {
    throw "$Label is unexpectedly small ($($item.Length) bytes): $(ConvertTo-OutputPath $Path)"
  }

  return $item.FullName
}

function ConvertTo-ProcessArgument {
  param([Parameter(Mandatory = $true)][string]$Value)

  if ($Value.Length -eq 0) {
    return '""'
  }
  if ($Value -notmatch '[\s"]') {
    return $Value
  }

  return '"' + ($Value -replace '\\+$', '$0$0' -replace '"', '\"') + '"'
}

function Invoke-LocalProcess {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [Parameter(Mandatory = $true)][string[]]$Arguments,
    [Parameter(Mandatory = $true)][string]$WorkingDirectory,
    [string]$StandardInput = $null,
    [int]$TimeoutSeconds = 60
  )

  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = $FilePath
  $startInfo.Arguments = ($Arguments | ForEach-Object { ConvertTo-ProcessArgument $_ }) -join " "
  $startInfo.WorkingDirectory = $WorkingDirectory
  $startInfo.UseShellExecute = $false
  $startInfo.RedirectStandardOutput = $true
  $startInfo.RedirectStandardError = $true
  $startInfo.RedirectStandardInput = $null -ne $StandardInput
  $startInfo.CreateNoWindow = $true

  $process = [System.Diagnostics.Process]::new()
  $process.StartInfo = $startInfo
  $timer = [System.Diagnostics.Stopwatch]::StartNew()

  [void]$process.Start()
  $stdoutTask = $process.StandardOutput.ReadToEndAsync()
  $stderrTask = $process.StandardError.ReadToEndAsync()

  if ($null -ne $StandardInput) {
    $process.StandardInput.Write($StandardInput)
    $process.StandardInput.Close()
  }

  $completed = $process.WaitForExit($TimeoutSeconds * 1000)
  if (-not $completed) {
    try {
      $process.Kill()
    } catch {
      # Best-effort cleanup after timeout.
    }
    throw "$(ConvertTo-OutputPath $FilePath) timed out after $TimeoutSeconds second(s)"
  }

  $process.WaitForExit()
  $timer.Stop()

  [pscustomobject]@{
    exitCode = $process.ExitCode
    stdout = $stdoutTask.Result
    stderr = $stderrTask.Result
    elapsedMs = [int][Math]::Round($timer.Elapsed.TotalMilliseconds)
  }
}

function Normalize-Transcript {
  param([string]$Value)

  $builder = [System.Text.StringBuilder]::new()
  foreach ($character in $Value.Normalize([System.Text.NormalizationForm]::FormD).ToCharArray()) {
    $category = [System.Globalization.CharUnicodeInfo]::GetUnicodeCategory($character)
    if ($category -ne [System.Globalization.UnicodeCategory]::NonSpacingMark) {
      [void]$builder.Append($character)
    }
  }

  return (($builder.ToString().ToLowerInvariant() -replace '[^a-z0-9]+', ' ').Trim())
}

$WhisperBinary = Resolve-RequiredFile $WhisperBinary "Whisper binary" 1024
$WhisperModelPath = Resolve-RequiredFile $WhisperModelPath "Whisper model" 10000000
$PiperBinary = Resolve-RequiredFile $PiperBinary "Piper binary" 1024
$PiperVoicePath = Resolve-RequiredFile $PiperVoicePath "Piper voice model" 10000000

$piperConfigPath = "$PiperVoicePath.json"
Resolve-RequiredFile $piperConfigPath "Piper voice config" 100 | Out-Null

$outputParent = Split-Path -Parent $OutputFile
if (-not [string]::IsNullOrWhiteSpace($outputParent)) {
  New-Item -ItemType Directory -Force -Path $outputParent | Out-Null
}

$OutputFile = [System.IO.Path]::GetFullPath($OutputFile)
$whisperStdoutFile = Join-Path $voiceRoot "voice-smoke-whisper-stdout.txt"
$whisperStderrFile = Join-Path $voiceRoot "voice-smoke-whisper-stderr.txt"

$piperResult = Invoke-LocalProcess `
  -FilePath $PiperBinary `
  -Arguments @("--model", $PiperVoicePath, "--output_file", $OutputFile) `
  -WorkingDirectory (Split-Path -Parent $PiperBinary) `
  -StandardInput $Text `
  -TimeoutSeconds $PiperTimeoutSeconds

if ($piperResult.exitCode -ne 0) {
  $safePiperError = ConvertTo-OutputText $piperResult.stderr @($PiperBinary, $PiperVoicePath, $OutputFile)
  throw "Piper failed with exit code $($piperResult.exitCode): $safePiperError"
}

$audio = Resolve-RequiredFile $OutputFile "Piper output WAV" $MinAudioBytes
$audioBytes = (Get-Item -LiteralPath $audio).Length

$whisperResult = Invoke-LocalProcess `
  -FilePath $WhisperBinary `
  -Arguments @("-m", $WhisperModelPath, "-f", $audio, "-l", $Language, "-nt", "-np") `
  -WorkingDirectory (Split-Path -Parent $WhisperBinary) `
  -TimeoutSeconds $WhisperTimeoutSeconds

$writeTranscriptArtifacts = $IncludeTranscript -or $KeepTranscriptArtifacts
if ($writeTranscriptArtifacts) {
  [System.IO.File]::WriteAllText($whisperStdoutFile, $whisperResult.stdout, [System.Text.UTF8Encoding]::new($false))
  [System.IO.File]::WriteAllText($whisperStderrFile, $whisperResult.stderr, [System.Text.UTF8Encoding]::new($false))
}

if ($whisperResult.exitCode -ne 0) {
  $safeWhisperError = ConvertTo-OutputText $whisperResult.stderr @($WhisperBinary, $WhisperModelPath, $audio)
  throw "Whisper failed with exit code $($whisperResult.exitCode): $safeWhisperError"
}

$transcript = $whisperResult.stdout.Trim()
$normalizedTranscript = Normalize-Transcript $transcript
$missingTokens = @()
foreach ($token in $ExpectedTokens) {
  $normalizedToken = Normalize-Transcript $token
  if (-not [string]::IsNullOrWhiteSpace($normalizedToken) -and $normalizedTranscript -notmatch "(^| )$([regex]::Escape($normalizedToken))( |$)") {
    $missingTokens += $token
  }
}

if ($missingTokens.Count -gt 0) {
  throw "Whisper transcript did not include expected token(s): $($missingTokens -join ', ')"
}

$whisperStdoutOutput = $null
$whisperStderrOutput = $null
if ($writeTranscriptArtifacts) {
  $whisperStdoutOutput = ConvertTo-OutputPath $whisperStdoutFile
  $whisperStderrOutput = ConvertTo-OutputPath $whisperStderrFile
}

$result = [ordered]@{
  ok = $true
  textRedacted = -not $IncludeTranscript
  language = $Language
  audioFile = ConvertTo-OutputPath $audio
  audioBytes = $audioBytes
  transcriptRedacted = -not $IncludeTranscript
  transcriptArtifactsWritten = $writeTranscriptArtifacts
  artifactPathsRedacted = -not $ShowPaths
  expectedTokens = $ExpectedTokens
  piperMs = $piperResult.elapsedMs
  whisperMs = $whisperResult.elapsedMs
  whisperStdoutFile = $whisperStdoutOutput
  whisperStderrFile = $whisperStderrOutput
}

if ($IncludeTranscript) {
  $result.text = $Text
  $result.transcript = $transcript
}

[pscustomobject]$result | ConvertTo-Json -Depth 5
