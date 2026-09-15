param(
  [ValidateSet('check','test','preview','android','web')][string]$Action = 'check',
  [string]$ApiUrl = '',
  [string]$FlutterPath = ''
)
$ErrorActionPreference = 'Stop'
$mobileRoot = Join-Path $PSScriptRoot '../apps/mobile'
if (-not $FlutterPath) {
  $flutterCmd = Get-Command flutter -ErrorAction SilentlyContinue
  if ($flutterCmd) { $FlutterPath = $flutterCmd.Source }
  else { $FlutterPath = Join-Path $env:USERPROFILE '.local/share/aro-flutter-sdk/bin/flutter.bat' }
}
if (-not (Test-Path -LiteralPath $FlutterPath)) { throw 'Flutter est requis. Installez le SDK ou passez -FlutterPath.' }
Push-Location $mobileRoot
try {
  switch ($Action) {
    'check' { & $FlutterPath analyze }
    'test' { & $FlutterPath test }
    'android' {
      if (-not (Test-Path 'android/key.properties')) { throw 'Configurez android/key.properties pour signer la version Android.' }
      if (-not $ApiUrl.StartsWith('https://')) { throw 'Passez -ApiUrl avec une adresse HTTPS de serveur ARO.' }
      & $FlutterPath build apk --release "--dart-define=ARO_API_URL=$ApiUrl"
    }
    'web' {
      if (-not $ApiUrl.StartsWith('https://')) { throw 'Passez -ApiUrl avec une adresse HTTPS de serveur ARO.' }
      & $FlutterPath build web "--dart-define=ARO_API_URL=$ApiUrl"
    }
    'preview' {
      & $FlutterPath build web --debug --dart-define=ARO_API_URL=http://127.0.0.1:1440
      if ($LASTEXITCODE -ne 0) { throw 'La compilation a échoué.' }
      node tool/preview-server.cjs
    }
  }
  if ($LASTEXITCODE -ne 0) { throw "Flutter a échoué avec le code $LASTEXITCODE." }
} finally { Pop-Location }
