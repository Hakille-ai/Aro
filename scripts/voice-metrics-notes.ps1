param(
  [switch]$VerifyDocs,
  [switch]$VerifyScripts,
  [switch]$VerifyCi
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$voiceAcceptancePath = Join-Path $root "docs/VOICE_ACCEPTANCE.md"
$productSpecPath = Join-Path $root "docs/PRODUCT_SPEC.md"
$roadmapPath = Join-Path $root "docs/ROADMAP.md"
$ciPath = Join-Path $root ".github/workflows/ci.yml"
$packageJsonPath = Join-Path $root "package.json"
$desktopPackageJsonPath = Join-Path $root "apps/desktop/package.json"
$voiceUnitTestPath = Join-Path $root "apps/desktop/src/lib/voice/voice.test.ts"
$vitestConfigPath = Join-Path $root "apps/desktop/vitest.config.ts"

function Join-RepoPath {
  param([Parameter(Mandatory = $true)][string]$RelativePath)

  Join-Path $root $RelativePath
}

function Read-Doc {
  param(
    [Parameter(Mandatory = $true)][string]$Path
  )

  if (-not (Test-Path $Path)) {
    throw "Required document is missing: $Path"
  }

  Get-Content -Raw -Path $Path
}

function Assert-ContentContains {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Content,
    [Parameter(Mandatory = $true)][string[]]$Patterns,
    [string]$FailureNoun = "required wording"
  )

  foreach ($pattern in $Patterns) {
    if ($Content -notmatch $pattern) {
      throw "$Name is missing $FailureNoun`: $pattern"
    }
  }
}

function Assert-FileExists {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label,
    [int64]$MinBytes = 1
  )

  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Label is missing: $Path"
  }

  $item = Get-Item -LiteralPath $Path
  if ($item.Length -lt $MinBytes) {
    throw "$Label is unexpectedly small ($($item.Length) bytes): $Path"
  }

  return $item.FullName
}

function Read-JsonFile {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label
  )

  Assert-FileExists $Path $Label 2 | Out-Null
  try {
    Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
  } catch {
    throw "$Label is not valid JSON: $($_.Exception.Message)"
  }
}

function Assert-NpmScript {
  param(
    [Parameter(Mandatory = $true)][psobject]$PackageJson,
    [Parameter(Mandatory = $true)][string]$PackageName,
    [Parameter(Mandatory = $true)][string]$ScriptName
  )

  if ($null -eq $PackageJson.scripts -or $PackageJson.scripts.PSObject.Properties.Name -notcontains $ScriptName) {
    throw "$PackageName is missing npm script '$ScriptName'"
  }
}

Write-Host "ARO voice metrics notes"
Write-Host "- CI validates acceptance-gate docs, scripts, tests, and workflow wiring; it does not capture microphone audio."
Write-Host "- Runtime observability must stay aggregate-only: counts, durations, buckets, versions, and coarse error codes."
Write-Host "- Logs and metrics must not include raw audio, generated audio, raw transcripts, prompts, or assistant text."
Write-Host "- Model-backed wake-word/STT/TTS measurements are run locally with scripts/measure-voice.ps1 when models are installed."

if ($VerifyDocs) {
  $voiceAcceptance = Read-Doc $voiceAcceptancePath
  $productSpec = Read-Doc $productSpecPath
  $roadmap = Read-Doc $roadmapPath

  Assert-ContentContains "docs/VOICE_ACCEPTANCE.md" $voiceAcceptance @(
    "Wake-word",
    "false accept rate",
    "false reject rate",
    "STT",
    "word error rate",
    "TTS",
    "time to first audio",
    "Model-backed performance harness",
    "scripts/measure-voice.ps1",
    "fixture IDs",
    "CI contract",
    "Privacy-safe observability",
    "MUST NOT log raw audio",
    "MUST NOT log raw transcripts",
    "does not capture microphone"
  )

  Assert-ContentContains "docs/PRODUCT_SPEC.md" $productSpec @(
    "No raw audio or transcript logs",
    "aggregate-only observability",
    "model-backed voice harness",
    "wake-word",
    "STT",
    "TTS"
  )

  Assert-ContentContains "docs/ROADMAP.md" $roadmap @(
    "Voice acceptance gates",
    "scripts/measure-voice.ps1",
    "no raw audio or transcript logs"
  )

  Write-Host "Voice acceptance documentation checks passed."
}

if ($VerifyScripts) {
  $requiredFiles = @(
    @{ path = ".github/workflows/ci.yml"; label = "CI workflow" },
    @{ path = "scripts/doctor.ps1"; label = "doctor script" },
    @{ path = "scripts/setup-voice.ps1"; label = "voice setup script" },
    @{ path = "scripts/smoke-voice.ps1"; label = "voice smoke script" },
    @{ path = "scripts/voice-metrics-notes.ps1"; label = "voice metrics notes script" },
    @{ path = "scripts/measure-voice.ps1"; label = "voice measurement harness" },
    @{ path = "docs/VOICE_ACCEPTANCE.md"; label = "voice acceptance doc" },
    @{ path = "docs/PRODUCT_SPEC.md"; label = "product spec" },
    @{ path = "docs/ROADMAP.md"; label = "roadmap" },
    @{ path = "apps/desktop/vitest.config.ts"; label = "Vitest config" },
    @{ path = "apps/desktop/src/lib/voice/voice.test.ts"; label = "voice unit tests" }
  )

  foreach ($file in $requiredFiles) {
    Assert-FileExists (Join-RepoPath $file.path) $file.label | Out-Null
  }

  $rootPackage = Read-JsonFile $packageJsonPath "root package.json"
  $desktopPackage = Read-JsonFile $desktopPackageJsonPath "desktop package.json"
  foreach ($scriptName in @("check", "build", "test:unit", "test:rust")) {
    Assert-NpmScript $rootPackage "root package.json" $scriptName
  }
  Assert-NpmScript $desktopPackage "apps/desktop/package.json" "test:unit"

  $voiceTests = Get-Content -Raw -LiteralPath $voiceUnitTestPath
  Assert-ContentContains "apps/desktop/src/lib/voice/voice.test.ts" $voiceTests @(
    "matchesWakeWord",
    "isWhisperHallucination",
    "encodeWav",
    "appendBoundedAudioChunk"
  ) "required voice unit coverage"

  $harness = Get-Content -Raw -LiteralPath (Join-RepoPath "scripts/measure-voice.ps1")
  Assert-ContentContains "scripts/measure-voice.ps1" $harness @(
    "VerifyOnly",
    "IncludeTranscript",
    "transcriptRedacted",
    "wake_word",
    "stt",
    "tts",
    "MUST NOT log raw audio"
  ) "required harness contract"

  Write-Host "Voice script and unit-test presence checks passed."
}

if ($VerifyCi) {
  $ci = Read-Doc $ciPath
  Assert-ContentContains ".github/workflows/ci.yml" $ci @(
    "voice-acceptance",
    "voice-metrics-notes.ps1 -VerifyDocs -VerifyScripts -VerifyCi",
    "measure-voice.ps1 -VerifyOnly",
    "npm run test:unit",
    "npm run check",
    "npm run build",
    "cargo test --workspace"
  ) "required CI gate"

  Write-Host "Voice CI workflow checks passed."
}
