param(
  [string]$WhisperBinary = "",
  [string]$WhisperModelPath = "",
  [string]$PiperBinary = "",
  [string]$PiperVoicePath = "",
  [string]$OutputDirectory = "",
  [string]$MetricsPath = "",
  [int]$PiperTimeoutSeconds = 60,
  [int]$WhisperTimeoutSeconds = 120,
  [double]$MaxWordErrorRate = 0.35,
  [int]$MaxPiperP95Ms = 1200,
  [int]$MaxWhisperP95Ms = 2500,
  [switch]$IncludeTranscript,
  [switch]$KeepArtifacts,
  [switch]$VerifyOnly
)

$ErrorActionPreference = "Stop"

# Privacy contract: shared metrics MUST NOT log raw audio, generated audio,
# raw transcripts, prompts, assistant text, secrets, or absolute local paths.
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
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
  $OutputDirectory = Join-Path ([System.IO.Path]::GetTempPath()) "aro-voice-measure"
}

function Get-VoiceFixtures {
  @(
    [pscustomobject]@{
      id = "wake_positive_short"
      kind = "wake_word"
      language = "fr"
      text = "Salut ARO, lance l'ecoute."
      expectedTokens = @("salut", "lance", "ecoute")
      wakeExpected = $true
      maxWordErrorRate = 0.45
      minAudioBytes = 12000
    },
    [pscustomobject]@{
      id = "wake_negative_embedded"
      kind = "wake_word"
      language = "fr"
      text = "Caroline lance l'ecoute."
      expectedTokens = @("caroline", "lance", "ecoute")
      wakeExpected = $false
      maxWordErrorRate = 0.45
      minAudioBytes = 12000
    },
    [pscustomobject]@{
      id = "stt_short_command"
      kind = "stt"
      language = "fr"
      text = "Bonjour, je suis ARO."
      expectedTokens = @("bonjour", "suis")
      wakeExpected = $null
      maxWordErrorRate = 0.35
      minAudioBytes = 12000
    },
    [pscustomobject]@{
      id = "tts_short_response"
      kind = "tts"
      language = "fr"
      text = "Resume la reunion en trois points."
      expectedTokens = @("resume", "reunion", "trois", "points")
      wakeExpected = $null
      maxWordErrorRate = 0.35
      minAudioBytes = 12000
    }
  )
}

function Assert-FixtureContract {
  param([Parameter(Mandatory = $true)][object[]]$Fixtures)

  $ids = @{}
  foreach ($fixture in $Fixtures) {
    if ([string]::IsNullOrWhiteSpace($fixture.id)) {
      throw "Voice fixture is missing an id."
    }
    if ($ids.ContainsKey($fixture.id)) {
      throw "Duplicate voice fixture id: $($fixture.id)"
    }
    $ids[$fixture.id] = $true

    if ($fixture.kind -notin @("wake_word", "stt", "tts")) {
      throw "Voice fixture '$($fixture.id)' has unsupported kind '$($fixture.kind)'."
    }
    if ([string]::IsNullOrWhiteSpace($fixture.text)) {
      throw "Voice fixture '$($fixture.id)' is missing scripted text."
    }
    if ($null -eq $fixture.expectedTokens -or $fixture.expectedTokens.Count -eq 0) {
      throw "Voice fixture '$($fixture.id)' must define expected tokens."
    }
  }
}

function Resolve-RequiredFile {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label,
    [int64]$MinBytes = 1
  )

  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Label not found. Run scripts\setup-voice.ps1 first or pass the path explicitly."
  }

  $item = Get-Item -LiteralPath $Path
  if ($item.Length -lt $MinBytes) {
    throw "$Label is unexpectedly small ($($item.Length) bytes)."
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
    throw "process_timeout"
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

function Normalize-Text {
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

function Get-TextWords {
  param([string]$Value)

  $normalized = Normalize-Text $Value
  if ([string]::IsNullOrWhiteSpace($normalized)) {
    return @()
  }

  @($normalized -split " " | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Get-WordDistance {
  param(
    [Parameter(Mandatory = $true)][string[]]$Reference,
    [Parameter(Mandatory = $true)][string[]]$Hypothesis
  )

  $matrix = [int[,]]::new($Reference.Count + 1, $Hypothesis.Count + 1)
  for ($i = 0; $i -le $Reference.Count; $i++) {
    $matrix[$i, 0] = $i
  }
  for ($j = 0; $j -le $Hypothesis.Count; $j++) {
    $matrix[0, $j] = $j
  }

  for ($i = 1; $i -le $Reference.Count; $i++) {
    for ($j = 1; $j -le $Hypothesis.Count; $j++) {
      $cost = 1
      if ($Reference[$i - 1] -eq $Hypothesis[$j - 1]) {
        $cost = 0
      }

      $deleteCost = $matrix[$i - 1, $j] + 1
      $insertCost = $matrix[$i, $j - 1] + 1
      $substituteCost = $matrix[$i - 1, $j - 1] + $cost
      $matrix[$i, $j] = [Math]::Min([Math]::Min($deleteCost, $insertCost), $substituteCost)
    }
  }

  return $matrix[$Reference.Count, $Hypothesis.Count]
}

function Get-WordErrorRate {
  param(
    [Parameter(Mandatory = $true)][string]$ReferenceText,
    [Parameter(Mandatory = $true)][string]$HypothesisText
  )

  $reference = Get-TextWords $ReferenceText
  $hypothesis = Get-TextWords $HypothesisText
  if ($reference.Count -eq 0) {
    if ($hypothesis.Count -eq 0) {
      return 0.0
    }
    return 1.0
  }

  $distance = Get-WordDistance -Reference $reference -Hypothesis $hypothesis
  return [double]$distance / [double]$reference.Count
}

function Test-WakeWordTranscript {
  param([string]$Transcript)

  $normalized = Normalize-Text $Transcript
  return $normalized -match "(^| )(aro|haro|aero|arrow|arro|aroo)( |$)"
}

function Get-SizeBucket {
  param([int64]$Bytes)

  if ($Bytes -lt 16000) {
    return "<16KB"
  }
  if ($Bytes -lt 64000) {
    return "16-64KB"
  }
  if ($Bytes -lt 256000) {
    return "64-256KB"
  }
  return ">=256KB"
}

function Get-Percentile {
  param(
    [Parameter(Mandatory = $true)][int[]]$Values,
    [int]$Percentile = 95
  )

  if ($Values.Count -eq 0) {
    return 0
  }

  $ordered = @($Values | Sort-Object)
  $index = [int][Math]::Ceiling(($Percentile / 100.0) * $ordered.Count) - 1
  if ($index -lt 0) {
    $index = 0
  }
  if ($index -ge $ordered.Count) {
    $index = $ordered.Count - 1
  }

  return [int]$ordered[$index]
}

function Measure-Fixture {
  param(
    [Parameter(Mandatory = $true)][psobject]$Fixture,
    [Parameter(Mandatory = $true)][string]$ResolvedWhisperBinary,
    [Parameter(Mandatory = $true)][string]$ResolvedWhisperModelPath,
    [Parameter(Mandatory = $true)][string]$ResolvedPiperBinary,
    [Parameter(Mandatory = $true)][string]$ResolvedPiperVoicePath
  )

  $audioPath = Join-Path $OutputDirectory "$($Fixture.id).wav"
  if (Test-Path -LiteralPath $audioPath -PathType Leaf) {
    Remove-Item -LiteralPath $audioPath -Force
  }

  try {
    $piperResult = Invoke-LocalProcess `
      -FilePath $ResolvedPiperBinary `
      -Arguments @("--model", $ResolvedPiperVoicePath, "--output_file", $audioPath) `
      -WorkingDirectory (Split-Path -Parent $ResolvedPiperBinary) `
      -StandardInput $Fixture.text `
      -TimeoutSeconds $PiperTimeoutSeconds

    if ($piperResult.exitCode -ne 0) {
      throw "piper_failed"
    }

    $audio = Resolve-RequiredFile $audioPath "Piper output WAV" $Fixture.minAudioBytes
    $audioBytes = (Get-Item -LiteralPath $audio).Length

    $whisperResult = Invoke-LocalProcess `
      -FilePath $ResolvedWhisperBinary `
      -Arguments @("-m", $ResolvedWhisperModelPath, "-f", $audio, "-l", $Fixture.language, "-nt", "-np") `
      -WorkingDirectory (Split-Path -Parent $ResolvedWhisperBinary) `
      -TimeoutSeconds $WhisperTimeoutSeconds

    if ($whisperResult.exitCode -ne 0) {
      throw "whisper_failed"
    }

    $transcript = $whisperResult.stdout.Trim()
    $normalizedTranscript = Normalize-Text $transcript
    $missingTokens = @()
    foreach ($token in $Fixture.expectedTokens) {
      $normalizedToken = Normalize-Text $token
      if (-not [string]::IsNullOrWhiteSpace($normalizedToken) -and $normalizedTranscript -notmatch "(^| )$([regex]::Escape($normalizedToken))( |$)") {
        $missingTokens += $token
      }
    }

    $wakeDetected = $null
    $wakeOk = $true
    if ($Fixture.kind -eq "wake_word") {
      $wakeDetected = Test-WakeWordTranscript $transcript
      $wakeOk = $wakeDetected -eq $Fixture.wakeExpected
    }

    $wordErrorRate = Get-WordErrorRate $Fixture.text $transcript
    $fixtureMaxWer = $MaxWordErrorRate
    if ($null -ne $Fixture.maxWordErrorRate) {
      $fixtureMaxWer = [double]$Fixture.maxWordErrorRate
    }

    $result = [ordered]@{
      id = $Fixture.id
      kind = $Fixture.kind
      ok = ($missingTokens.Count -eq 0 -and $wakeOk -and $wordErrorRate -le $fixtureMaxWer)
      transcriptRedacted = -not $IncludeTranscript
      textRedacted = -not $IncludeTranscript
      audioRetained = [bool]$KeepArtifacts
      audioBytesBucket = Get-SizeBucket $audioBytes
      piperMs = $piperResult.elapsedMs
      whisperMs = $whisperResult.elapsedMs
      wordErrorRate = [Math]::Round($wordErrorRate, 4)
      wordErrorRateThreshold = $fixtureMaxWer
      expectedTokenCount = $Fixture.expectedTokens.Count
      matchedTokenCount = $Fixture.expectedTokens.Count - $missingTokens.Count
      missingTokenCount = $missingTokens.Count
      wakeExpected = $Fixture.wakeExpected
      wakeDetected = $wakeDetected
    }

    if ($IncludeTranscript) {
      $result.referenceText = $Fixture.text
      $result.transcript = $transcript
    }

    return [pscustomobject]$result
  } finally {
    if (-not $KeepArtifacts -and (Test-Path -LiteralPath $audioPath -PathType Leaf)) {
      Remove-Item -LiteralPath $audioPath -Force
    }
  }
}

$fixtures = @(Get-VoiceFixtures)
Assert-FixtureContract $fixtures

if ($VerifyOnly) {
  [pscustomobject]@{
    ok = $true
    mode = "verify-only"
    fixtureCount = $fixtures.Count
    fixtureKinds = @($fixtures | ForEach-Object { $_.kind } | Sort-Object -Unique)
    privacy = [ordered]@{
      rawAudioLogged = $false
      generatedAudioLogged = $false
      transcriptRedactedByDefault = $true
      metricsContainAbsolutePaths = $false
    }
  } | ConvertTo-Json -Depth 6
  return
}

$WhisperBinary = Resolve-RequiredFile $WhisperBinary "Whisper binary" 1024
$WhisperModelPath = Resolve-RequiredFile $WhisperModelPath "Whisper model" 10000000
$PiperBinary = Resolve-RequiredFile $PiperBinary "Piper binary" 1024
$PiperVoicePath = Resolve-RequiredFile $PiperVoicePath "Piper voice model" 10000000
Resolve-RequiredFile "$PiperVoicePath.json" "Piper voice config" 100 | Out-Null

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

$results = @()
foreach ($fixture in $fixtures) {
  try {
    $results += Measure-Fixture `
      -Fixture $fixture `
      -ResolvedWhisperBinary $WhisperBinary `
      -ResolvedWhisperModelPath $WhisperModelPath `
      -ResolvedPiperBinary $PiperBinary `
      -ResolvedPiperVoicePath $PiperVoicePath
  } catch {
    Write-Warning "Voice fixture '$($fixture.id)' failed with redacted category runtime_failed."
    $results += [pscustomobject]@{
      id = $fixture.id
      kind = $fixture.kind
      ok = $false
      transcriptRedacted = $true
      textRedacted = $true
      errorCategory = "runtime_failed"
      errorMessageRedacted = $true
      wakeExpected = $fixture.wakeExpected
      wakeDetected = $null
    }
  }
}

$piperValues = @($results | Where-Object { $null -ne $_.piperMs } | ForEach-Object { [int]$_.piperMs })
$whisperValues = @($results | Where-Object { $null -ne $_.whisperMs } | ForEach-Object { [int]$_.whisperMs })
$werValues = @($results | Where-Object { $null -ne $_.wordErrorRate } | ForEach-Object { [double]$_.wordErrorRate })
$nonWakeWerValues = @($results | Where-Object { $_.kind -ne "wake_word" -and $null -ne $_.wordErrorRate } | ForEach-Object { [double]$_.wordErrorRate })
$maxWer = 0.0
if ($werValues.Count -gt 0) {
  $maxWer = [double](@($werValues | Sort-Object -Descending)[0])
}
$maxNonWakeWer = 0.0
if ($nonWakeWerValues.Count -gt 0) {
  $maxNonWakeWer = [double](@($nonWakeWerValues | Sort-Object -Descending)[0])
}

$failedFixtures = @($results | Where-Object { -not $_.ok })
$wakeFalseRejects = @($results | Where-Object { $_.kind -eq "wake_word" -and $_.wakeExpected -eq $true -and $_.wakeDetected -ne $true })
$wakeFalseAccepts = @($results | Where-Object { $_.kind -eq "wake_word" -and $_.wakeExpected -eq $false -and $_.wakeDetected -eq $true })
$piperP95 = Get-Percentile -Values $piperValues -Percentile 95
$whisperP95 = Get-Percentile -Values $whisperValues -Percentile 95

$gates = @(
  [ordered]@{
    name = "all_fixtures_pass"
    ok = $failedFixtures.Count -eq 0
    actual = $failedFixtures.Count
    threshold = 0
  },
  [ordered]@{
    name = "wake_word_false_reject_fixtures"
    ok = $wakeFalseRejects.Count -eq 0
    actual = $wakeFalseRejects.Count
    threshold = 0
  },
  [ordered]@{
    name = "wake_word_false_accept_fixtures"
    ok = $wakeFalseAccepts.Count -eq 0
    actual = $wakeFalseAccepts.Count
    threshold = 0
  },
  [ordered]@{
    name = "stt_tts_word_error_rate_max"
    ok = $maxNonWakeWer -le $MaxWordErrorRate
    actual = [Math]::Round($maxNonWakeWer, 4)
    threshold = $MaxWordErrorRate
  },
  [ordered]@{
    name = "piper_synthesis_p95_ms"
    ok = $piperP95 -le $MaxPiperP95Ms
    actual = $piperP95
    threshold = $MaxPiperP95Ms
  },
  [ordered]@{
    name = "whisper_transcription_p95_ms"
    ok = $whisperP95 -le $MaxWhisperP95Ms
    actual = $whisperP95
    threshold = $MaxWhisperP95Ms
  }
)

$output = [ordered]@{
  schemaVersion = 1
  generatedAtUtcMinute = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:00Z")
  privacy = [ordered]@{
    rawAudioLogged = $false
    generatedAudioLogged = $false
    generatedAudioRetained = [bool]$KeepArtifacts
    transcriptsRedacted = -not $IncludeTranscript
    fixtureTextRedacted = -not $IncludeTranscript
    metricsContainAbsolutePaths = $false
  }
  runtime = [ordered]@{
    whisperBinary = Split-Path -Leaf $WhisperBinary
    whisperModel = Split-Path -Leaf $WhisperModelPath
    piperBinary = Split-Path -Leaf $PiperBinary
    piperVoice = Split-Path -Leaf $PiperVoicePath
  }
  thresholds = [ordered]@{
    maxWordErrorRate = $MaxWordErrorRate
    maxPiperP95Ms = $MaxPiperP95Ms
    maxWhisperP95Ms = $MaxWhisperP95Ms
  }
  summary = [ordered]@{
    totalFixtures = $results.Count
    failedFixtures = $failedFixtures.Count
    wakeFalseRejectFixtures = $wakeFalseRejects.Count
    wakeFalseAcceptFixtures = $wakeFalseAccepts.Count
    piperP95Ms = $piperP95
    whisperP95Ms = $whisperP95
    maxWordErrorRate = [Math]::Round($maxWer, 4)
    maxSttTtsWordErrorRate = [Math]::Round($maxNonWakeWer, 4)
  }
  gates = $gates
  results = $results
}

$json = $output | ConvertTo-Json -Depth 10
if (-not [string]::IsNullOrWhiteSpace($MetricsPath)) {
  $metricsParent = Split-Path -Parent $MetricsPath
  if (-not [string]::IsNullOrWhiteSpace($metricsParent)) {
    New-Item -ItemType Directory -Force -Path $metricsParent | Out-Null
  }
  [System.IO.File]::WriteAllText($MetricsPath, $json, [System.Text.UTF8Encoding]::new($false))
}

$json

$failedGates = @($gates | Where-Object { -not $_.ok })
if ($failedGates.Count -gt 0) {
  throw "Voice measurement gates failed: $(@($failedGates | ForEach-Object { $_.name }) -join ', ')"
}
