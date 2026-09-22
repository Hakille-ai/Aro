param([string]$BaseUrl='http://127.0.0.1:8710')
$ErrorActionPreference='Stop'
# Sovereign-AI verification: local-first generation, honest errors,
# default-deny cloud. Explicit local, isolated account. Never production.
$testUri=[Uri]$BaseUrl
if ($testUri.Host -notin @('127.0.0.1','localhost')) { throw 'Ce test doit viser une API locale.' }
$testId=[guid]::NewGuid().ToString('N')
$testPassword='Sov!'+[guid]::NewGuid().ToString('N')
$script:headers=@{}
$results=[System.Collections.Generic.List[object]]::new()
function Request([string]$Method,[string]$Path,$Body=$null) {
  $args=@{Method=$Method;Uri="$BaseUrl/v1$Path";Headers=$script:headers;TimeoutSec=25}
  if($null -ne $Body){$args.ContentType='application/json';$args.Body=ConvertTo-Json -InputObject $Body -Depth 20 -Compress}
  Invoke-RestMethod @args
}
function Check([string]$Label,[scriptblock]$Run) {
  try { & $Run; $results.Add(@{name=$Label;status='passed'}) }
  catch { $results.Add(@{name=$Label;status='failed';error=$_.Exception.Message}) }
}
$auth=Request 'POST' '/auth/register' @{email="sov-$testId@example.test";password=$testPassword;name='Sovereign QA';organizationName="Sovereign QA $testId"}
$script:headers=@{Authorization="Bearer $($auth.accessToken)"}
if(-not $auth.accessToken){throw 'Le serveur doit retourner AuthSession.accessToken.'}

Check 'assistant/status reports local generation' {
  $status=Request 'GET' '/assistant/status'
  if($status.canGenerate -ne $true){throw 'canGenerate devrait etre vrai (Ollama local)'}
  if($status.source -ne 'local'){throw "source=${status.source}, attendu local"}
  if($status.ollamaReachable -ne $true){throw 'Ollama devrait etre joignable'}
  if(-not $status.runnableModelIds -or $status.runnableModelIds.Count -eq 0){throw 'aucun modele executable'}
  if($status.guidance -ne 'ok'){throw "guidance=${status.guidance}, attendu ok"}
}

Check 'assistant/stream generates for real (no unavailable)' {
  $conversation=Request 'POST' '/conversations' @{title='Sovereign QA';mode='chat'}
  $sseFile=Join-Path $env:TEMP "aro-sov-$testId.sse"
  $body=@{conversationId=$conversation.id;content='Dis bonjour en cinq mots exactement.';mode='chat';webAccess='off'} | ConvertTo-Json -Compress
  # Premier chunks puis coupure : prouve la generation sans attendre la fin.
  $curlArgs=@('-s','-N','--max-time','25','-X','POST',"$BaseUrl/v1/assistant/stream",'-H',"Authorization: Bearer $($auth.accessToken)",'-H','Content-Type: application/json','--data-binary','@-')
  $body | & curl.exe @curlArgs > $sseFile
  $sse=Get-Content $sseFile -Raw -ErrorAction SilentlyContinue
  Remove-Item $sseFile -ErrorAction SilentlyContinue
  if(-not $sse -or $sse -notmatch 'event: chunk'){throw 'aucun chunk SSE recu'}
  if($sse -match "Je n'ai pas pu g"){throw 'le serveur a renvoye une indisponibilite'}
  if($sse -match '"unavailable":\s*true'){throw 'done.unavailable=true : erreur persistee'}
}

Check 'explicit model override is honored exactly (no substitution)' {
  $conversation=Request 'POST' '/conversations' @{title='Sovereign Override QA';mode='chat'}
  $sseFile=Join-Path $env:TEMP "aro-sov-override-$testId.sse"
  # Pre-chauffe best-effort (le premier chargement Ollama a froid est lent).
  try { & ollama run phi3:mini 'ok' 2>$null | Out-Null } catch {}
  # phi3:mini est petit et rapide : la surcharge doit servir LUI, pas un autre.
  # 120 s : l'inference CPU peut mettre une minute sur un premier appel.
  $body=@{conversationId=$conversation.id;content='Dis ok.';mode='chat';webAccess='off';modelId='phi3:mini';provider='ollama-local'} | ConvertTo-Json -Compress
  $curlArgs=@('-s','-N','--max-time','120','-X','POST',"$BaseUrl/v1/assistant/stream",'-H',"Authorization: Bearer $($auth.accessToken)",'-H','Content-Type: application/json','--data-binary','@-')
  $body | & curl.exe @curlArgs > $sseFile
  $sse=Get-Content $sseFile -Raw -ErrorAction SilentlyContinue
  Remove-Item $sseFile -ErrorAction SilentlyContinue
  if(-not $sse -or $sse -notmatch 'event: chunk'){throw 'aucun chunk SSE recu (override)'}
  if($sse -match '"unavailable":\s*true'){throw 'override explicite local a echoue'}
  if($sse -notmatch '"modelId":"phi3:mini"'){throw 'le modele servi nest pas celui demande (substitution ?)'}
}

Check 'unknown explicit model fails honestly without substitution' {
  $conversation=Request 'POST' '/conversations' @{title='Sovereign Strict QA';mode='chat'}
  $sseFile=Join-Path $env:TEMP "aro-sov-strict-$testId.sse"
  $body=@{conversationId=$conversation.id;content='Dis ok.';mode='chat';webAccess='off';modelId='modele-qui-nexiste-pas:999'} | ConvertTo-Json -Compress
  $curlArgs=@('-s','-N','--max-time','25','-X','POST',"$BaseUrl/v1/assistant/stream",'-H',"Authorization: Bearer $($auth.accessToken)",'-H','Content-Type: application/json','--data-binary','@-')
  $body | & curl.exe @curlArgs > $sseFile
  $sse=Get-Content $sseFile -Raw -ErrorAction SilentlyContinue
  Remove-Item $sseFile -ErrorAction SilentlyContinue
  if($sse -notmatch '"unavailable":\s*true'){throw 'un modele inconnu devrait echouer honnetement'}
  if($sse -match 'modele-qui-nexiste-pas' -and $sse -match '"assistantMessage":\{'){throw 'erreur persistee comme message (interdit)'}
}

Check 'ai-cloud default-deny then consent roundtrip (no key material)' {
  $status=Request 'GET' '/settings/ai-cloud/status'
  if($status.consent.enabled -ne $false){throw 'opt-in devrait etre coupe par defaut'}
  $roundtrip=Request 'PUT' '/settings/ai-cloud/consent' @{enabled=$true;providerIds=@('openai');dataResidency='EU'}
  if($roundtrip.consent.enabled -ne $true){throw 'consentement non active'}
  if($roundtrip.consent.acceptedBy -eq $null){throw 'acceptedBy manquant (audit)'}
  $withKey=Request 'PUT' '/settings/ai-cloud/keys/openai' @{apiKey='sk-test-sov-fake-key-001'}
  if(($withKey.keys | Where-Object {$_.providerId -eq 'openai'}).configured -ne $true){throw 'cle non marquee deposee'}
  $leak=($withKey | ConvertTo-Json -Depth 10 -Compress)
  if($leak.Contains('sk-test-sov-fake-key-001')){throw 'FUITE : cle visible dans le statut !'}
  $revoked=Request 'DELETE' '/settings/ai-cloud/keys/openai'
  if(($revoked.keys | Where-Object {$_.providerId -eq 'openai'} | Measure-Object).Count -ne 0){throw 'cle non revoquee'}
  $off=Request 'PUT' '/settings/ai-cloud/consent' @{enabled=$false;providerIds=@();dataResidency=$null}
  if($off.consent.enabled -ne $false){throw 'opt-in non coupe'}
}

$failed=@($results | Where-Object {$_.status -eq 'failed'})
Write-Output ($results | Format-Table -AutoSize | Out-String)
if($failed.Count -gt 0){ throw "$($failed.Count) verification(s) Sovereign-AI en echec." }
Write-Output 'Sovereign-AI: toutes les verifications sont vertes.'
