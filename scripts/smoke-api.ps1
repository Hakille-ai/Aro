param(
  [string]$BaseUrl = "http://127.0.0.1:8710",
  [string]$DatabaseUrl = "postgres://aro:aro@127.0.0.1:5432/aro",
  [switch]$SkipDockerDatabaseChecks
)

$ErrorActionPreference = "Stop"

function Invoke-AroJson {
  param(
    [Parameter(Mandatory = $true)][string]$Method,
    [Parameter(Mandatory = $true)][string]$Uri,
    [hashtable]$Headers = @{},
    [object]$Body = $null
  )

  $params = @{
    Method = $Method
    Uri = $Uri
    Headers = $Headers
    ContentType = "application/json"
    TimeoutSec = 20
  }
  if ($null -ne $Body) {
    $params.Body = ($Body | ConvertTo-Json -Depth 20)
  }
  Invoke-RestMethod @params
}

function Assert-True {
  param(
    [bool]$Condition,
    [string]$Message
  )
  if (-not $Condition) {
    throw "Smoke assertion failed: $Message"
  }
}

function Assert-AroHttpError {
  param(
    [Parameter(Mandatory = $true)][string]$Method,
    [Parameter(Mandatory = $true)][string]$Uri,
    [hashtable]$Headers = @{},
    [object]$Body = $null,
    [Parameter(Mandatory = $true)][int]$ExpectedStatus,
    [Parameter(Mandatory = $true)][string]$ExpectedErrorFragment
  )

  try {
    Invoke-AroJson -Method $Method -Uri $Uri -Headers $Headers -Body $Body | Out-Null
    throw "Expected $ExpectedStatus from $Method $Uri"
  } catch {
    $response = $_.Exception.Response
    if ($null -eq $response) {
      throw
    }
    Assert-True ([int]$response.StatusCode -eq $ExpectedStatus) "$Method $Uri should return HTTP $ExpectedStatus"
    $body = $_.ErrorDetails.Message
    Assert-True ($body.Contains($ExpectedErrorFragment)) "$Method $Uri should include '$ExpectedErrorFragment'"
  }
}

function Get-Sha256Hex {
  param([Parameter(Mandatory = $true)][byte[]]$Bytes)
  $sha = [System.Security.Cryptography.SHA256]::Create()
  try {
    -join ($sha.ComputeHash($Bytes) | ForEach-Object { $_.ToString("x2") })
  } finally {
    $sha.Dispose()
  }
}

$BaseUrl = $BaseUrl.TrimEnd('/')
if ($BaseUrl.EndsWith('/v1')) {
  $RootUrl = $BaseUrl.Substring(0, $BaseUrl.Length - 3)
  $ApiUrl = $BaseUrl
} else {
  $RootUrl = $BaseUrl
  $ApiUrl = "$BaseUrl/v1"
}

$health = Invoke-AroJson -Method "GET" -Uri "$RootUrl/health"
Assert-True ($health.status -eq "ok") "health endpoint should return ok"

# Health endpoints remain unversioned for orchestrators. All product contracts use /v1.
$BaseUrl = $ApiUrl

$suffix = [Guid]::NewGuid().ToString("N")
$email = "smoke-$suffix@aro.local"
$password = "SuperSecret123!"

$session = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/auth/register" -Body @{
  email = $email
  password = $password
  name = "Smoke User"
  organizationName = "Smoke Org $suffix"
  organizationDomain = "smoke.local"
}
$headers = @{ Authorization = "Bearer $($session.accessToken)" }

$bootstrap = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/bootstrap" -Headers $headers
Assert-True ($bootstrap.currentUser.email -eq $email) "bootstrap should return registered user"
Assert-True ($bootstrap.activeOrganization.name -like "Smoke Org*") "bootstrap should return active organization"

$secondOrg = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/organizations" -Headers $headers -Body @{
  name = "Second Smoke Org $suffix"
  domain = "second.local"
}
$switched = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/auth/switch-organization" -Headers $headers -Body @{
  organizationId = $secondOrg.organization.id
  refreshToken = $session.refreshToken
}
$headers = @{ Authorization = "Bearer $($switched.accessToken)" }
Assert-True ($switched.activeOrganization.id -eq $secondOrg.organization.id) "switch organization should return a session scoped to the new org"

$preferences = Invoke-AroJson -Method "PUT" -Uri "$BaseUrl/preferences" -Headers $headers -Body @{
  theme = "dark"
  language = "en"
  wakeWordEnabled = $true
  inferenceMode = "local"
}
Assert-True ($preferences.theme -eq "dark") "preferences should persist"

$members = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/memberships" -Headers $headers
Assert-True (@($members).Count -ge 1) "memberships should include the current user"

$apiKeyResponse = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/api-keys" -Headers $headers -Body @{
  name = "Smoke Key"
}
Assert-True ([string]::IsNullOrWhiteSpace($apiKeyResponse.secret) -eq $false) "api key create should return one-time secret"
$apiKeys = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/api-keys" -Headers $headers
Assert-True (@($apiKeys).Count -ge 1) "api key list should include created key"
Assert-True (-not ($apiKeys | ConvertTo-Json -Depth 10).Contains("secret")) "api key list should not expose secrets"

$memory = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/memories" -Headers $headers -Body @{
  content = "remember smoke $suffix"
  category = "technical"
  pinned = $true
}
Assert-True ([string]::IsNullOrWhiteSpace($memory.id) -eq $false) "memory create should return an id"
$memorySearch = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/memories/search?q=remember%20smoke%20$suffix&limit=5" -Headers $headers
$memorySearchJson = @($memorySearch) | ConvertTo-Json -Depth 10
Assert-True ($memorySearchJson.Contains($memory.id)) "memory search should find created memory"

$memoryIndexStatus = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/memory-index/status" -Headers $headers
Assert-True (@("active", "degraded", "disabled") -contains $memoryIndexStatus.state) "memory index status should be explicit"
Assert-True ([string]::IsNullOrWhiteSpace($memoryIndexStatus.embeddingProvider) -eq $false) "memory index status should include embedding provider"
Assert-True ([string]::IsNullOrWhiteSpace($memoryIndexStatus.embeddingModel) -eq $false) "memory index status should include embedding model"
$memoryIndexReindex = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/memory-index/reindex" -Headers $headers
Assert-True ($null -ne $memoryIndexReindex.status) "memory reindex should return status"
Assert-True (@("active", "degraded", "disabled") -contains $memoryIndexReindex.status.state) "memory reindex status should be explicit"
if ($memoryIndexStatus.state -eq "active" -or $memoryIndexReindex.status.state -eq "active") {
  $semanticMemorySearch = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/memories/search?q=durable%20technical%20note&limit=5" -Headers $headers
  $semanticMemorySearchJson = @($semanticMemorySearch) | ConvertTo-Json -Depth 10
  Assert-True ($semanticMemorySearchJson.Contains($memory.id)) "hybrid memory search should include pinned indexed memory"
}

$team = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/teams" -Headers $headers -Body @{
  name = "Smoke Team"
  description = "Created by smoke test"
  memberIds = @()
}
Assert-True ($team.name -eq "Smoke Team") "team create should round trip"

$dockerForRbac = Get-Command docker -ErrorAction SilentlyContinue
if ($dockerForRbac) {
  $memberEmail = "member-$suffix@aro.local"
  $memberPassword = "MemberSecret123!"
  $memberSession = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/auth/register" -Body @{
    email = $memberEmail
    password = $memberPassword
    name = "Smoke Member"
    organizationName = "Member Own Org $suffix"
  }
  Invoke-AroJson -Method "POST" -Uri "$BaseUrl/memberships" -Headers $headers -Body @{
    name = "Smoke Member"
    email = $memberEmail
    role = "member"
  } | Out-Null
  $orgId = $switched.activeOrganization.id
  $memberUserId = $memberSession.user.id
  docker exec aro-postgres psql -U aro -d aro -v ON_ERROR_STOP=1 -c "UPDATE memberships SET status = 'active', updated_at = now() WHERE user_id = '$memberUserId' AND organization_id = '$orgId';" | Out-Null
  $memberHeaders = @{ Authorization = "Bearer $($memberSession.accessToken)" }
  $memberScoped = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/auth/switch-organization" -Headers $memberHeaders -Body @{
    organizationId = $orgId
    refreshToken = $memberSession.refreshToken
  }
  $memberHeaders = @{ Authorization = "Bearer $($memberScoped.accessToken)" }

  $memberMemories = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/memories" -Headers $memberHeaders
  $memberMemoriesJson = @($memberMemories) | ConvertTo-Json -Depth 10
  if ([string]::IsNullOrWhiteSpace($memberMemoriesJson)) {
    $memberMemoriesJson = "[]"
  }
  Assert-True (-not $memberMemoriesJson.Contains($memory.id)) "member should not see another user's memory"
  Assert-AroHttpError -Method "PATCH" -Uri "$BaseUrl/memories/$($memory.id)" -Headers $memberHeaders -ExpectedStatus 404 -ExpectedErrorFragment "not found" -Body @{
    content = "forbidden update"
  }
  Assert-AroHttpError -Method "POST" -Uri "$BaseUrl/plugins" -Headers $memberHeaders -ExpectedStatus 403 -ExpectedErrorFragment "organization admin permission required" -Body @{
    name = "Forbidden Plugin"
  }
  Assert-AroHttpError -Method "POST" -Uri "$BaseUrl/skills" -Headers $memberHeaders -ExpectedStatus 403 -ExpectedErrorFragment "organization manager permission required" -Body @{
    name = "Forbidden Skill"
    kind = "prompt"
    content = "nope"
  }
}

$conversation = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/conversations" -Headers $headers -Body @{
  title = "Smoke Conversation"
  mode = "chat"
}
$fileText = "file smoke $suffix"
$fileBytes = [System.Text.Encoding]::UTF8.GetBytes($fileText)
$fileName = "smoke-file-$suffix.txt"
$fileSha = Get-Sha256Hex -Bytes $fileBytes
$upload = Invoke-AroJson -Method "POST" -Uri "$BaseUrl/files/uploads" -Headers $headers -Body @{
  originalName = $fileName
  mimeType = "text/plain"
  sizeBytes = $fileBytes.Length
  sha256 = $fileSha
}
Assert-True ($upload.file.status -eq "pending") "file upload create should return pending file"
$uploadResponse = Invoke-WebRequest -UseBasicParsing -Method "PUT" -Uri "$BaseUrl$($upload.uploadUrl)" -Headers $headers -ContentType "text/plain" -Body $fileBytes
$uploadedFile = $uploadResponse.Content | ConvertFrom-Json
Assert-True ($uploadedFile.status -eq "available") "file upload content should mark file available"
Assert-True ($uploadedFile.sha256 -eq $fileSha) "file upload should store expected sha256"
$downloadPath = Join-Path ([System.IO.Path]::GetTempPath()) "aro-smoke-$suffix.txt"
try {
  Invoke-WebRequest -UseBasicParsing -Method "GET" -Uri "$BaseUrl/files/$($uploadedFile.id)/content" -Headers $headers -OutFile $downloadPath
  $downloadedBytes = [System.IO.File]::ReadAllBytes($downloadPath)
  Assert-True ([System.Convert]::ToBase64String($downloadedBytes) -eq [System.Convert]::ToBase64String($fileBytes)) "file download should round trip byte-for-byte"
} finally {
  if (Test-Path -LiteralPath $downloadPath) {
    Remove-Item -LiteralPath $downloadPath -Force
  }
}
$sse = Invoke-WebRequest -UseBasicParsing -Method "POST" -Uri "$BaseUrl/assistant/stream" -Headers $headers -ContentType "application/json" -Body (@{
  conversationId = $conversation.id
  content = "ping"
  mode = "chat"
  systemPrompt = $null
  attachments = @(@{
    fileId = $uploadedFile.id
    mode = "cloud-object"
    displayName = $fileName
    sizeBytes = $fileBytes.Length
    mimeType = "text/plain"
  })
} | ConvertTo-Json -Depth 20)
Assert-True ($sse.Content.Contains("event: done")) "assistant stream should finish with done event"

$messages = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/conversations/$($conversation.id)/messages" -Headers $headers
Assert-True (@($messages).Count -eq 2) "assistant stream should persist user and assistant messages"
$messagesJson = @($messages) | ConvertTo-Json -Depth 20
Assert-True ($messagesJson.Contains($uploadedFile.id)) "messages should include cloud attachment metadata"
Assert-True (-not $messagesJson.Contains($fileText)) "message content should not inline raw file text"

$eventStats = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/admin/event-stats" -Headers $headers
Assert-True ([int]$eventStats.auditCount -ge 3) "mutations should write audit events"
Assert-True ([int]$eventStats.outboxCount -ge 3) "mutations should write outbox events"

$pendingOutbox = Invoke-AroJson -Method "GET" -Uri "$BaseUrl/admin/outbox?limit=10" -Headers $headers
Assert-True (@($pendingOutbox).Count -ge 1) "admin outbox endpoint should expose pending events"
$pendingOutboxJson = @($pendingOutbox) | ConvertTo-Json -Depth 20
Assert-True (-not $pendingOutboxJson.Contains("remember smoke $suffix")) "memory outbox payload should not expose raw content"
Assert-True (-not $pendingOutboxJson.Contains($fileText)) "file outbox payload should not expose raw file content"
Assert-True (-not $pendingOutboxJson.Contains($fileName)) "file outbox payload should not expose original file name"
Assert-True ($pendingOutboxJson.Contains("contentHash")) "memory outbox payload should include redacted content hash"

$auditCount = [int]$eventStats.auditCount
$outboxCount = [int]$eventStats.outboxCount
if (-not $SkipDockerDatabaseChecks) {
  $docker = Get-Command docker -ErrorAction SilentlyContinue
  if ($docker) {
    $orgId = $switched.activeOrganization.id
    $databaseAuditCount = docker exec aro-postgres psql -U aro -d aro -t -A -c "SELECT COUNT(*) FROM audit_events WHERE organization_id = '$orgId';"
    $databaseOutboxCount = docker exec aro-postgres psql -U aro -d aro -t -A -c "SELECT COUNT(*) FROM outbox_events WHERE organization_id = '$orgId';"
    $rawMemoryAuditCount = docker exec aro-postgres psql -U aro -d aro -t -A -c "SELECT COUNT(*) FROM audit_events WHERE organization_id = '$orgId' AND data::text LIKE '%remember smoke $suffix%';"
    $rawMemoryOutboxCount = docker exec aro-postgres psql -U aro -d aro -t -A -c "SELECT COUNT(*) FROM outbox_events WHERE organization_id = '$orgId' AND payload::text LIKE '%remember smoke $suffix%';"
    Assert-True ([int]$databaseAuditCount -eq $auditCount) "API audit count should match database"
    Assert-True ([int]$databaseOutboxCount -eq $outboxCount) "API outbox count should match database"
    Assert-True ([int]$rawMemoryAuditCount -eq 0) "audit events should not expose raw memory content"
    Assert-True ([int]$rawMemoryOutboxCount -eq 0) "outbox events should not expose raw memory content"
  }
}

[PSCustomObject]@{
  ok = $true
  baseUrl = $BaseUrl
  email = $email
  organizationId = $switched.activeOrganization.id
  memoryId = $memory.id
  memoryIndexState = $memoryIndexReindex.status.state
  teamId = $team.id
  conversationId = $conversation.id
  messageCount = @($messages).Count
  auditCount = $auditCount
  outboxCount = $outboxCount
} | ConvertTo-Json -Depth 10
