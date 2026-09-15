param(
  [string]$BaseUrl = "http://127.0.0.1:8710",
  [string]$RedisContainer = "aro-redis",
  [string]$RedisStreamKey = "aro:development:outbox"
)

$ErrorActionPreference = "Stop"

function Assert-True {
  param(
    [bool]$Condition,
    [string]$Message
  )
  if (-not $Condition) {
    throw "Redis smoke assertion failed: $Message"
  }
}

$docker = Get-Command docker -ErrorAction SilentlyContinue
Assert-True ($null -ne $docker) "docker command should be available"

$container = docker ps --filter "name=$RedisContainer" --format "{{.Names}}"
Assert-True ($container -contains $RedisContainer) "Redis container '$RedisContainer' should be running"

$ping = docker exec $RedisContainer redis-cli ping
Assert-True ($ping -eq "PONG") "Redis should respond to PING"

$BaseUrl = $BaseUrl.TrimEnd('/')
if ($BaseUrl.EndsWith('/v1')) {
  $RootUrl = $BaseUrl.Substring(0, $BaseUrl.Length - 3)
  $ApiUrl = $BaseUrl
} else {
  $RootUrl = $BaseUrl
  $ApiUrl = "$BaseUrl/v1"
}
$ready = Invoke-RestMethod -Uri "$RootUrl/ready" -TimeoutSec 10
Assert-True ($ready.redis -eq "ok") "/ready should report Redis ok"

$metrics = Invoke-WebRequest -UseBasicParsing -Uri "$RootUrl/metrics" -TimeoutSec 10
Assert-True ($metrics.Content.Contains("aro_redis_ready 1")) "/metrics should report Redis ready"
Assert-True ($metrics.Content.Contains("aro_rate_limit_enabled 1")) "/metrics should report rate limiting enabled"

$rateIp = "198.51.100.$(Get-Random -Minimum 1 -Maximum 250)"
$rateLimited = $false
for ($i = 0; $i -lt 12; $i++) {
  try {
    Invoke-RestMethod `
      -Method "POST" `
      -Uri "$ApiUrl/auth/login" `
      -Headers @{ "X-Forwarded-For" = $rateIp } `
      -ContentType "application/json" `
      -Body (@{ email = "missing-$rateIp@aro.local"; password = "definitely-wrong" } | ConvertTo-Json) `
      -TimeoutSec 10 | Out-Null
  } catch {
    $response = $_.Exception.Response
    if ($null -ne $response -and [int]$response.StatusCode -eq 429) {
      $rateLimited = $true
      break
    }
    if ($null -eq $response -or [int]$response.StatusCode -ne 401) {
      throw
    }
  }
}
Assert-True $rateLimited "auth login burst should eventually return HTTP 429"

$streamInfo = docker exec $RedisContainer redis-cli XLEN $RedisStreamKey

[PSCustomObject]@{
  ok = $true
  baseUrl = $RootUrl
  apiUrl = $ApiUrl
  redisContainer = $RedisContainer
  streamKey = $RedisStreamKey
  streamLength = [int]$streamInfo
} | ConvertTo-Json -Depth 10
