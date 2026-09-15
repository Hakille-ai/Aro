param(
  [string]$BaseUrl = "http://127.0.0.1:8710",
  [string]$Email = "contract.test@aro.dev",
  [string]$Password = "contract-test-password-123"
)
# Verifie chaque endpoint utilise par le client mobile (@aro/api-client).
# Distingue "route absente" (404 vide Axum) de "ressource absente" (404 JSON {"error"}).
$ErrorActionPreference = "Stop"
$Api = $BaseUrl.TrimEnd('/')

function Call {
  param([string]$Method, [string]$Path, [object]$Body = $null, [string]$Token = "")
  $headers = @{}
  if ($Token) { $headers["Authorization"] = "Bearer $Token" }
  $params = @{ Method = $Method; Uri = "$Api$Path"; Headers = $headers; TimeoutSec = 20 }
  if ($null -ne $Body) {
    $params.ContentType = "application/json"
    $params.Body = ($Body | ConvertTo-Json -Depth 20 -Compress)
  }
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

function Verdict {
  param([int]$Status, [string]$Body, [int[]]$OkStatuses = @(200, 201, 204))
  if ($Status -in $OkStatuses) { return "OK" }
  if ($Status -eq 404) {
    if ($Body -match '"error"') { return "ROUTE-OK/RESOURCE-MISSING" }
    return "!!! ROUTE MANQUANTE"
  }
  if ($Status -eq 405) { return "!!! MAUVAISE METHODE" }
  return "HTTP $Status"
}

function Check {
  param([string]$Method, [string]$Path, [object]$Body = $null, [string]$Token = "", [int[]]$Ok = @(200, 201, 204))
  $r = Call -Method $Method -Path $Path -Body $Body -Token $Token
  $v = Verdict -Status $r.Status -Body $r.Body -OkStatuses $Ok
  $short = if ($r.Body.Length -gt 110) { $r.Body.Substring(0, 110) + "..." } else { $r.Body }
  Write-Host ("{0,-6} {1,-42} => {2,3}  [{3}]  {4}" -f $Method, $Path, $r.Status, $v, $short)
  return $r
}

# --- Auth : register ou login si deja existant ---
$reg = Call -Method "POST" -Path "/v1/auth/register" -Body @{
  email = $Email; password = $Password; name = "Contract Test"; organizationName = "Contract Org"
}
if ($reg.Status -eq 409) {
  Write-Output "register => 409 (compte existant, on se logue)"
  $login = Call -Method "POST" -Path "/v1/auth/login" -Body @{ email = $Email; password = $Password }
  $session = $login.Body | ConvertFrom-Json
} elseif ($reg.Status -eq 200 -or $reg.Status -eq 201) {
  $session = $reg.Body | ConvertFrom-Json
} else {
  Write-Output "AUTH ECHEC: $($reg.Status) $($reg.Body)"; exit 1
}
$Token = $session.accessToken
$OrgId = $session.activeOrganization.id
Write-Output "auth OK, org=$OrgId"
Write-Output ""

# --- Lecture / bootstrap ---
Check "GET" "/v1/bootstrap" -Token $Token | Out-Null
Check "GET" "/v1/auth/session" -Token $Token | Out-Null
Check "GET" "/v1/capabilities" -Token $Token | Out-Null
Check "GET" "/health" | Out-Null

# --- Conversations / messages ---
$c = Check "POST" "/v1/conversations" -Token $Token -Body @{ title = "contract probe" }
$convId = ($c.Body | ConvertFrom-Json).id
Check "GET" "/v1/conversations" -Token $Token | Out-Null
Check "PATCH" "/v1/conversations/$convId" -Token $Token -Body @{ title = "renamed" } | Out-Null
Check "GET" "/v1/conversations/$convId/messages" -Token $Token | Out-Null
Check "PATCH" "/v1/conversations/$convId/move" -Token $Token -Body @{ projectId = $null; folderId = $null } | Out-Null
Check "POST" "/v1/conversations/$convId/regenerate" -Token $Token -Body @{} | Out-Null
Check "GET" "/v1/conversations/search?q=test&limit=5" -Token $Token | Out-Null
Check "POST" "/v1/assistant/message" -Token $Token -Body @{ content = "hi"; mode = "chat" } | Out-Null
Check "PATCH" "/v1/messages/00000000-0000-0000-0000-000000000000" -Token $Token -Body @{ content = "x" } | Out-Null

# --- Projets / dossiers ---
$p = Check "POST" "/v1/projects" -Token $Token -Body @{ name = "contract proj" }
$projId = ($p.Body | ConvertFrom-Json).id
Check "GET" "/v1/projects" -Token $Token | Out-Null
$f = Check "POST" "/v1/folders" -Token $Token -Body @{ name = "contract folder" }
$folderId = ($f.Body | ConvertFrom-Json).id
Check "GET" "/v1/folders" -Token $Token | Out-Null

# --- Fichiers ---
Check "GET" "/v1/files" -Token $Token | Out-Null
Check "GET" "/v1/files/quota" -Token $Token | Out-Null

# --- Agents ---
Check "GET" "/v1/agent/runs" -Token $Token | Out-Null
Check "GET" "/v1/agent/lanes" -Token $Token | Out-Null
Check "GET" "/v1/agent/orchestrator" -Token $Token | Out-Null
Check "GET" "/v1/agent/permission-profiles" -Token $Token | Out-Null
Check "GET" "/v1/permissions" -Token $Token | Out-Null

# --- Memoires ---
Check "GET" "/v1/memories" -Token $Token | Out-Null
Check "GET" "/v1/memory-index/status" -Token $Token | Out-Null

# --- Orgs / membres / cles ---
Check "GET" "/v1/organizations" -Token $Token | Out-Null
Check "GET" "/v1/memberships" -Token $Token | Out-Null
Check "GET" "/v1/invitations?limit=5" -Token $Token | Out-Null
Check "GET" "/v1/api-keys" -Token $Token | Out-Null
Check "PATCH" "/v1/users/me" -Token $Token -Body @{} | Out-Null
Check "PUT" "/v1/preferences" -Token $Token -Body @{} | Out-Null

# --- Settings / modeles / runtime / voix ---
Check "GET" "/v1/settings" -Token $Token | Out-Null
Check "GET" "/v1/models" -Token $Token | Out-Null
Check "GET" "/v1/model-providers" -Token $Token | Out-Null
Check "POST" "/v1/model-providers/select" -Token $Token -Body @{ providerId = "x"; modelId = "y" } | Out-Null
Check "GET" "/v1/runtime/status" -Token $Token | Out-Null
Check "GET" "/v1/voice/status" -Token $Token | Out-Null
Check "GET" "/v1/voice/models" -Token $Token | Out-Null

# --- Devices / push / client-state ---
Check "POST" "/v1/devices" -Token $Token -Body @{ name = "contract phone"; platform = "android" } | Out-Null
Check "PUT" "/v1/client-state/aro-push-token" -Token $Token -Body @{ value = @{ token = "ExponentPushToken[test]"; platform = "android" } } | Out-Null
Check "GET" "/v1/client-state/aro-push-token" -Token $Token | Out-Null

# --- Nettoyage ---
if ($folderId) { Check "DELETE" "/v1/folders/$folderId" -Token $Token -Ok @(200, 204) | Out-Null }
if ($projId) { Check "DELETE" "/v1/projects/$projId" -Token $Token -Ok @(200, 204) | Out-Null }
if ($convId) { Check "DELETE" "/v1/conversations/$convId" -Token $Token -Ok @(200, 204) | Out-Null }
Write-Output ""
Write-Output "Termine."
