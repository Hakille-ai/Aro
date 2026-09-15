param([string]$BaseUrl = "http://127.0.0.1:8710")
# Verifie les chemins d'ecriture utilises par l'UI mobile + le stream IA (provider mock).
$ErrorActionPreference = "Stop"
$Api = $BaseUrl.TrimEnd('/')

function Call {
  param([string]$Method, [string]$Path, [object]$Body = $null, [string]$Token = "")
  $headers = @{}
  if ($Token) { $headers["Authorization"] = "Bearer $Token" }
  $params = @{ Method = $Method; Uri = "$Api$Path"; Headers = $headers; TimeoutSec = 60 }
  if ($null -ne $Body) { $params.ContentType = "application/json"; $params.Body = ($Body | ConvertTo-Json -Depth 20 -Compress) }
  try {
    $r = Invoke-WebRequest @params -UseBasicParsing
    return @{ Status = [int]$r.StatusCode; Body = "$($r.Content)" }
  } catch {
    $resp = $_.Exception.Response
    if ($null -eq $resp) { return @{ Status = -1; Body = "NETWORK: $($_.Exception.Message)" } }
    $reader = New-Object System.IO.StreamReader($resp.GetResponseStream())
    return @{ Status = [int]$resp.StatusCode; Body = $reader.ReadToEnd() }
  }
}

$login = Call -Method "POST" -Path "/v1/auth/login" -Body @{ email = "contract.test@aro.dev"; password = "contract-test-password-123" }
$Token = ($login.Body | ConvertFrom-Json).accessToken
Write-Host "token ok"

# Nouveaux endpoints backend
$r = Call -Method "GET" -Path "/v1/auth/session" -Token $Token
Write-Host "GET /v1/auth/session => $($r.Status) $($r.Body.Substring(0, [Math]::Min(120, $r.Body.Length))))"
$r = Call -Method "GET" -Path "/v1/capabilities" -Token $Token
Write-Host "GET /v1/capabilities => $($r.Status) $($r.Body)"

# Conversations : cycle complet
$c = Call -Method "POST" -Path "/v1/conversations" -Token $Token -Body @{ title = "probe2"; mode = "chat" }
$cid = ($c.Body | ConvertFrom-Json).id
Write-Host "POST /v1/conversations => $($c.Status) id=$cid"
$r = Call -Method "PATCH" -Path "/v1/conversations/$cid" -Token $Token -Body @{ title = "renamed2" }
Write-Host "PATCH title => $($r.Status)"
$r = Call -Method "PATCH" -Path "/v1/conversations/$cid/move" -Token $Token -Body @{ projectId = $null; folderId = $null }
Write-Host "PATCH move => $($r.Status) $($r.Body.Substring(0, [Math]::Min(80, $r.Body.Length)))"
$r = Call -Method "GET" -Path "/v1/conversations/$cid/messages" -Token $Token
Write-Host "GET messages => $($r.Status) $($r.Body)"

# Stream IA (mock) : on lit juste le debut du flux
try {
  $headers = @{ Authorization = "Bearer $Token" }
  $body = (@{ content = "dis bonjour"; mode = "chat" } | ConvertTo-Json -Compress)
  $resp = Invoke-WebRequest -Method "POST" -Uri "$Api/v1/assistant/stream" -Headers $headers -ContentType "application/json" -Body $body -TimeoutSec 60 -UseBasicParsing
  $txt = "$($resp.Content)"
  Write-Host "POST /v1/assistant/stream => $($resp.StatusCode) (longueur flux: $($txt.Length), debut: $($txt.Substring(0, [Math]::Min(120, $txt.Length))))"
} catch {
  Write-Host "STREAM ECHEC: $($_.Exception.Message)"
}

# Upload fichier : create + PUT + get
$bytes = [System.Text.Encoding]::UTF8.GetBytes("contract probe file")
$sha = -join (([System.Security.Cryptography.SHA256]::Create()).ComputeHash($bytes) | ForEach-Object { $_.ToString("x2") })
$up = Call -Method "POST" -Path "/v1/files/uploads" -Token $Token -Body @{ originalName = "probe.txt"; mimeType = "text/plain"; sizeBytes = $bytes.Length; sha256 = $sha }
Write-Host "POST uploads => $($up.Status) $($up.Body.Substring(0, [Math]::Min(150, $up.Body.Length)))"
$upObj = $up.Body | ConvertFrom-Json
if ($upObj.upload.id) {
  $uri = "$Api/v1/files/uploads/$($upObj.upload.id)/content"
  try {
    $put = Invoke-WebRequest -Method "PUT" -Uri $uri -Headers @{ Authorization = "Bearer $Token" } -ContentType "application/octet-stream" -Body $bytes -TimeoutSec 60 -UseBasicParsing
    Write-Host "PUT content => $($put.StatusCode)"
  } catch { Write-Host "PUT content ECHEC: $($_.Exception.Message)" }
  $g = Call -Method "GET" -Path "/v1/files/$($upObj.file.id)" -Token $Token
  Write-Host "GET file => $($g.Status) $($g.Body.Substring(0, [Math]::Min(150, $g.Body.Length)))"
  $d = Call -Method "DELETE" -Path "/v1/files/$($upObj.file.id)" -Token $Token
  Write-Host "DELETE file => $($d.Status)"
}

# Agents : start + get + pause + resume + cancel
$run = Call -Method "POST" -Path "/v1/agent/runs" -Token $Token -Body @{ goal = "contract probe run"; mode = "chat"; priority = "normal" }
Write-Host "POST agent/runs => $($run.Status) $($run.Body.Substring(0, [Math]::Min(150, $run.Body.Length)))"
try {
  $runId = ($run.Body | ConvertFrom-Json).run.id
  if (-not $runId) { $runId = ($run.Body | ConvertFrom-Json).id }
  if ($runId) {
    $g = Call -Method "GET" -Path "/v1/agent/runs/$runId" -Token $Token
    Write-Host "GET run => $($g.Status)"
    foreach ($act in @("pause", "resume", "cancel")) {
      $a = Call -Method "POST" -Path "/v1/agent/runs/$runId/$act" -Token $Token -Body @{}
      Write-Host "POST run/$act => $($a.Status)"
    }
  }
} catch { Write-Host "AGENT DETAIL ECHEC: $($_.Exception.Message)" }

# Settings PUT (toggle speak)
$s = Call -Method "GET" -Path "/v1/settings" -Token $Token
$sObj = $s.Body | ConvertFrom-Json
$sObj.speakResponses = -not [bool]$sObj.speakResponses
$put = Call -Method "PUT" -Path "/v1/settings" -Token $Token -Body $sObj
Write-Host "PUT /v1/settings => $($put.Status)"

# Nettoyage conversation
$d = Call -Method "DELETE" -Path "/v1/conversations/$cid" -Token $Token
Write-Host "DELETE conv => $($d.Status)"
Write-Host "Termine."
