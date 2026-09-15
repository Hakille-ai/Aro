param([string]$BaseUrl='http://127.0.0.1:8710')
$ErrorActionPreference='Stop'
# Explicit local, isolated account. Never runs against a production URL.
$testUri=[Uri]$BaseUrl
if ($testUri.Host -notin @('127.0.0.1','localhost')) { throw 'Ce test doit viser une API locale.' }
$testId=[guid]::NewGuid().ToString('N')
$testPassword='Mobile!'+[guid]::NewGuid().ToString('N')
$script:headers=@{}
$results=[System.Collections.Generic.List[object]]::new()
$created=[System.Collections.Generic.List[object]]::new()
function Request([string]$Method,[string]$Path,$Body=$null) {
  $args=@{Method=$Method;Uri="$BaseUrl/v1$Path";Headers=$script:headers;TimeoutSec=20}
  if($null -ne $Body){$args.ContentType='application/json';$args.Body=ConvertTo-Json -InputObject $Body -Depth 20 -Compress}
  Invoke-RestMethod @args
}
function Check([string]$Label,[scriptblock]$Run) {
  try { & $Run; $results.Add(@{name=$Label;status='passed'}) }
  catch { $results.Add(@{name=$Label;status='failed';error=$_.Exception.Message}) }
}
$auth=Request 'POST' '/auth/register' @{email="mobile-$testId@example.test";password=$testPassword;name='Mobile Verification';organizationName="Mobile QA $testId"}
$script:headers=@{Authorization="Bearer $($auth.accessToken)"}
if(-not $auth.accessToken){throw 'Le serveur doit retourner AuthSession.accessToken.'}
try {
  Check 'bootstrap' { $b=Request 'GET' '/bootstrap'; if(-not $b.currentUser.id){throw 'currentUser absent'} }
  $project=Request 'POST' '/projects' @{name='Projet de vérification';color='#0071e3'}
  $created.Add(@{path="/projects/$($project.id)"})
  $folder=Request 'POST' '/folders' @{name='Dossier de vérification';projectId=$project.id}
  $created.Add(@{path="/folders/$($folder.id)"})
  $conversation=Request 'POST' '/conversations' @{title='Conversation de vérification';mode='chat'}
  $created.Add(@{path="/conversations/$($conversation.id)"})
  Check 'conversation move roundtrip' {
    Request 'PATCH' "/conversations/$($conversation.id)/move" @{projectId=$project.id;folderId=$folder.id}|Out-Null
    $c=Request 'GET' "/conversations/$($conversation.id)"
    if($c.folderId -ne $folder.id){throw 'folderId non persisté'}
    Request 'PATCH' "/conversations/$($conversation.id)" @{title='Titre mobile'}|Out-Null
  }
  Check 'settings roundtrip' { $s=Request 'GET' '/settings'; $s.retainHistory=$false; $saved=Request 'PUT' '/settings' $s; if($saved.retainHistory -ne $false){throw 'Préférence non persistée'} }
  $probes=@(
    @{collection='memories';body=@{content='Mémoire de vérification';category='personal';pinned=$false}},
    @{collection='personalities';body=@{name='Rédacteur';prompt='Écris clairement';temperature=0.7}},
    @{collection='system-prompts';body=@{mode='chat';identity='ARO';rules='Être précis';formatting='Clair';enabled=$true}},
    @{collection='skills';body=@{name='Synthèse';kind='prompt';content='Résumer';enabled=$true;triggers=@()}},
    @{collection='mcp-servers';body=@{name='MCP QA';transport='sse';url='https://example.test/mcp';enabled=$false}},
    @{collection='hooks';body=@{name='Webhook QA';url='https://example.test/hook';events=@();enabled=$false}},
    @{collection='scheduled-tasks';body=@{name='Tâche QA';prompt='Test';scheduleType='cron';cronExpression='0 9 * * *';enabled=$false}},
    @{collection='agent-definitions';body=@{name='Agent QA';systemPrompt='Être précis';enabled=$false}},
    @{collection='plans';body=@{title='Plan QA';tasks=@();status='active';conversationId=$conversation.id}}
  )
  foreach($probe in $probes){
    Check "$($probe.collection) CRUD" {
      $path="/collections/$($probe.collection)"
      $item=Request 'POST' $path $probe.body
      if(-not $item.id){throw 'Identifiant absent'}
      $created.Add(@{path="$path/$($item.id)"})
      Request 'GET' $path|Out-Null
      Request 'PATCH' "$path/$($item.id)" $probe.body|Out-Null
    }
  }
  Check 'refresh rotation' {
    $refreshed=Request 'POST' '/auth/refresh' @{refreshToken=$auth.refreshToken}
    if(-not $refreshed.accessToken -or $refreshed.refreshToken -eq $auth.refreshToken){throw 'Rotation absente'}
    $script:headers=@{Authorization="Bearer $($refreshed.accessToken)"}
  }
}finally{
  for($i=$created.Count-1;$i -ge 0;$i--){try{Request 'DELETE' $created[$i].path|Out-Null}catch{$results.Add(@{name="cleanup $($created[$i].path)";status='failed'})}}
}
$results|ConvertTo-Json -Depth 5
if(@($results|Where-Object status -eq 'failed').Count -gt 0){exit 1}
