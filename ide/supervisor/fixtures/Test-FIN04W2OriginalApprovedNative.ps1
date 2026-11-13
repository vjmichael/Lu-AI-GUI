# FIN-04W2-C disposable, genuine Windows native kernel transaction.
# IMPORTANT: original record_human_approval(true) is CI-SIMULATED, NOT
# physical operator confirmation. No LLDB, no arbitrary service command.
# First-party native host owns a real original Supervisor in memory across
# preparation, approval, OS admission, Running, release and completion.
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
  throw 'DENIED: W2-C native fixture requires disposable hosted Windows CI'
}
$name='LuIdeDebuggerBroker'
$ruleName='LuIdeFin04W2OriginalGrantFixedDeny'
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$principal=[Security.Principal.WindowsPrincipal]::new($identity)
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  throw 'DENIED: independent Windows SCM/WFP acceptance requires elevated native runner'
}
$system='NT AUTHORITY\SYSTEM'
$serviceSid='NT SERVICE\LuIdeDebuggerBroker'
$userSid=$identity.User.Value
$ownerRoot=Join-Path $env:RUNNER_TEMP 'Fin04W2OriginalNativeAdmission'
$serviceRoot=Join-Path $env:ProgramData 'LuIDEDebuggerBroker'
$negative=Join-Path $env:RUNNER_TEMP 'Fin04W2OriginalOwnerOnly.txt'
$service=(Resolve-Path 'ide/supervisor/target/debug/windows_fin04w2_debugger_service.exe').Path
$worker=(Resolve-Path 'ide/supervisor/target/debug/windows_fin04w2_fixed_probe.exe').Path
$nativeHost=(Resolve-Path 'ide/supervisor/target/debug/windows_fin04w2_native_host_acceptance.exe').Path
if (Get-Service -Name $name -ErrorAction SilentlyContinue) {throw 'DENIED: service unexpectedly preexists'}
if (Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue) {throw 'DENIED: WFP rule already exists'}
if (Test-Path $ownerRoot -PathType Any) {throw 'DENIED: original owner evidence root unexpectedly exists'}
if (Test-Path $serviceRoot -PathType Any) {throw 'DENIED: protected debugger service root unexpectedly exists'}
if (Test-Path $negative -PathType Any) {throw 'DENIED: seed already exists'}
$serviceSha=(Get-FileHash -LiteralPath $service -Algorithm SHA256).Hash.ToLowerInvariant()
$workerSha=(Get-FileHash -LiteralPath $worker -Algorithm SHA256).Hash.ToLowerInvariant()
$hostSha=(Get-FileHash -LiteralPath $nativeHost -Algorithm SHA256).Hash.ToLowerInvariant()
$created=$false
$wfpCreated=$false
$child=$null
$nativeAccepted=$false
try {
  # The exact matching worker binary must have a positive host-network
  # baseline and an OS WFP exact-program outbound deny before any launch.
  $profiles=@(Get-NetConnectionProfile | Where-Object {
    $_.IPv4Connectivity -ne 'Disconnected'
  } | ForEach-Object {
    if ([string]$_.NetworkCategory -eq 'DomainAuthenticated') {'Domain'}
    else {[string]$_.NetworkCategory}
  } | Select-Object -Unique)
  if ($profiles.Count -eq 0) {throw 'DENIED: no active firewall profile'}
  foreach ($n in $profiles) {
    if (!(Get-NetFirewallProfile -Name $n -ErrorAction Stop).Enabled) {
      throw 'DENIED: active Windows firewall profile disabled'
    }
  }
  $baseline=[Net.Sockets.TcpClient]::new()
  try {
    $p=$baseline.BeginConnect('1.1.1.1',443,$null,$null)
    if (!$p.AsyncWaitHandle.WaitOne(4000)) {throw 'DENIED: no positive external TCP baseline'}
    $baseline.EndConnect($p)
  } finally {$baseline.Dispose()}
  Write-Host 'FIN04W2_C_HOST_POSITIVE_TCP_BASELINE=PASS'

  New-Item -ItemType Directory -Path $ownerRoot | Out-Null
  & icacls.exe $ownerRoot /inheritance:r /grant:r ('*'+$userSid+':(OI)(CI)F') ($system+':(OI)(CI)F') | Out-Null
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: owner manifest root ACL setup'}
  Set-Content -LiteralPath $negative -Value 'native-foreign-file-must-be-inaccessible' -NoNewline
  & icacls.exe $negative /inheritance:r /grant:r ('*'+$userSid+':F') ($system+':F') | Out-Null
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: original owner-private seed ACL'}
  if ((Get-Content -LiteralPath $negative -Raw) -ne 'native-foreign-file-must-be-inaccessible') {
    throw 'DENIED: host-owned file positive read baseline'
  }
  $fw=New-NetFirewallRule -Name $ruleName -DisplayName $ruleName -Program $worker -Direction Outbound -Action Block -Enabled True -Profile Any -RemoteAddress Any
  $wfpCreated=$true
  $actual=Get-NetFirewallRule -Name $ruleName -PolicyStore ActiveStore -ErrorAction Stop
  $application=$actual|Get-NetFirewallApplicationFilter
  $address=$actual|Get-NetFirewallAddressFilter
  if ($actual.Action -ne 'Block' -or $actual.Direction -ne 'Outbound' -or $actual.Enabled -ne 'True' -or ![string]::Equals([string]$application.Program,$worker,[StringComparison]::OrdinalIgnoreCase) -or [string]$address.RemoteAddress -ne 'Any') {
    throw 'DENIED: active kernel-backed WFP policy not exact for benign worker'
  }
  Write-Host 'FIN04W2_C_ACTIVE_WFP_EXACT_WORKER_DENY=PASS'

  # Host owns original Supervisor across the complete transaction. We
  # cannot reopen a prepared plan and mint a fresh grant: the original
  # in-memory one-use authority is intentionally never persisted.
  $head=(& git rev-parse HEAD).Trim().ToLowerInvariant()
  if ($LASTEXITCODE -ne 0 -or $head.Length -ne 40 -or $head -notmatch '^[0-9a-f]+$') {
    throw 'DENIED: original native checked source head unavailable'
  }
  $env:LU_FIN04W2_NATIVE_SOURCE_HEAD=$head
  $env:LU_FIN04W2_NATIVE_FIXTURE_ROOT=$ownerRoot
  $env:LU_FIN04W2_SERVICE_BINARY=$service
  $env:LU_FIN04W2_SERVICE_SHA256=$serviceSha
  $stdout=Join-Path $ownerRoot 'host.stdout'
  $stderr=Join-Path $ownerRoot 'host.stderr'
  $child=Start-Process -FilePath $nativeHost -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
  $stage=Join-Path $ownerRoot 'staged.json'
  for ($i=0;$i -lt 140 -and !(Test-Path $stage);$i++) {
    Start-Sleep -Milliseconds 100
    $child.Refresh()
    if ($child.HasExited) {throw 'DENIED: native original host exited before fsynced Prepared'}
  }
  if (!(Test-Path $stage)) {throw 'DENIED: no durable original Prepared receipt/manifest'}
  $manifest=Get-Content -LiteralPath $stage -Raw | ConvertFrom-Json
  if ($manifest.approvalKind -ne 'CI_SIMULATED_NOT_PHYSICAL' -or $manifest.hostSha256 -ne $hostSha -or ![string]::Equals([string]$manifest.hostExecutable,$nativeHost,[StringComparison]::OrdinalIgnoreCase) -or $manifest.jobId -notmatch '^job-[0-9a-fA-F]{32}$' -or $manifest.planSha256 -notmatch '^[0-9a-fA-F]{64}$' -or $manifest.plan.recipeId -ne 'dap_lldb_project' -or $manifest.plan.sourceHead -ne $head) {
    throw 'DENIED: original host fixture identity, plan or grant provenance mismatch'
  }
  $journal=[string]$manifest.originalJournal
  $prepared=@(Get-Content -LiteralPath $journal | Where-Object {$_} | ForEach-Object {$_|ConvertFrom-Json})
  if ($prepared.Count -ne 1 -or $prepared[0].body.phase -ne 'prepared' -or $prepared[0].body.outcomeCode -ne 'host_plan_prepared' -or $prepared[0].body.jobId -ne $manifest.jobId) {
    throw 'DENIED: original fsynced Prepared receipt mismatch'
  }
  Write-Host 'FIN04W2_C_ORIGINAL_PREPARED_RECEIPT=PASS'

  & sc.exe create $name binPath= $service type= own start= demand obj= 'NT AUTHORITY\LocalService' | Out-Host
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: restricted service create failed'}
  $created=$true
  & sc.exe sidtype $name restricted | Out-Host
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: service restricted SID configuration failed'}
  & sc.exe privs $name 'SeChangeNotifyPrivilege/SeImpersonatePrivilege' | Out-Host
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: restricted service minimal privileges failed'}
  $sidValue=@(& sc.exe qsidtype $name)
  if ($LASTEXITCODE -ne 0 -or (($sidValue -join ' ') -notmatch 'RESTRICTED')) {
    throw 'DENIED: SCM service SID not restricted'
  }

  # Exact original journal: host user + SYSTEM FULL; the new debugger
  # service READ ONLY; reject inheritance or broad Administrators/Users.
  & icacls.exe $journal /inheritance:r /grant:r ('*'+$userSid+':F') ($system+':F') ($serviceSid+':R') | Out-Null
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: original native journal DACL'}
  $journalAcl=Get-Acl -LiteralPath $journal
  if (!$journalAcl.AreAccessRulesProtected -or @($journalAcl.Access).Count -ne 3) {
    throw 'DENIED: original journal ACE structure not exactly 3'
  }
  New-Item -ItemType Directory -Path $serviceRoot | Out-Null
  $config=[ordered]@{
    schema='lu.ide.fin04w2.c2.v1'
    serviceName=$name
    endpoint='\\.\pipe\LuIdeDebuggerBroker.v1'
    expectedServiceSha256=$serviceSha
    expectedHostSha256=$hostSha
    approvedHostExecutable=$nativeHost
    approvedHostUserSid=$userSid
    reviewOnlyPlan=$manifest.plan
    reviewOnlyPlanHash=$manifest.planSha256
    reviewOnlyNonce=('42'*16)
    originalSupervisorJournal=$journal
    originalJobId=$manifest.jobId
    fixedProbe=[ordered]@{
      executable=$worker
      expectedSha256=$workerSha
      foreignFile=$negative
    }
  }
  $cfg=Join-Path $serviceRoot 'service.json'
  $config|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $cfg -Encoding utf8 -NoNewline
  & icacls.exe $cfg /inheritance:r /grant:r ($system+':F') ($serviceSid+':M') | Out-Null
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: service protected config ACL'}
  & icacls.exe $serviceRoot /inheritance:r /grant:r ($system+':(OI)(CI)F') ($serviceSid+':(OI)(CI)M') | Out-Null
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: service protected root ACL'}
  $acl=Get-Acl -LiteralPath $serviceRoot
  if (!$acl.AreAccessRulesProtected -or @($acl.Access|Where-Object {
    $_.AccessControlType -ne 'Allow' -or $_.IdentityReference.Value -notin @($system,$serviceSid)
  }).Count) {throw 'DENIED: protected SCM root contains unknown principal'}
  & sc.exe start $name | Out-Host
  if ($LASTEXITCODE -ne 0) {throw 'DENIED: restricted SCM service failed start'}
  $state=$null
  for ($i=0;$i -lt 40;$i++) {
    Start-Sleep -Milliseconds 250
    $state=Get-CimInstance Win32_Service -Filter "Name='$name'"
    if ($state -and $state.State -eq 'Running' -and $state.ProcessId -gt 0) {break}
    if ($state -and $state.State -eq 'Stopped') {break}
  }
  if (!$state -or $state.State -ne 'Running' -or $state.ProcessId -le 0) {
    $why=if($state){"state=$($state.State);win32=$($state.ExitCode);stage=$($state.ServiceSpecificExitCode)"}else{'service_missing'}
    throw "DENIED: native restricted SCM could not attest original journal/Job/pipe ($why)"
  }
  Write-Host 'FIN04W2_C_RESTRICTED_NATIVE_SERVICE_READY=PASS'
  # The marker only permits the test harness to execute its FIXED,
  # clearly CI-simulated approval. It is NEVER an OS operator click.
  Set-Content -LiteralPath (Join-Path $ownerRoot 'allow-service') -NoNewline -Value 'CI_NATIVE_PROVISIONED'
  $deadline=(Get-Date).AddSeconds(35)
  while ((Get-Date) -lt $deadline) {
    $child.Refresh()
    if ($child.HasExited) {break}
    Start-Sleep -Milliseconds 250
  }
  $child.Refresh()
  if (!$child.HasExited) {throw 'DENIED: native host did not complete within bounded acceptance'}
  if ($child.ExitCode -ne 0) {
    if (Test-Path $stderr) {Get-Content $stderr | Select-Object -Last 35 | Out-Host}
    throw "DENIED: original native host denied execution or fsynced receipt ($($child.ExitCode))"
  }
  if (Test-Path $stdout) {Get-Content $stdout | Out-Host}
  $resultPath=Join-Path $ownerRoot 'result.json'
  if (!(Test-Path $resultPath)) {throw 'DENIED: durable final original host result missing'}
  $result=Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
  if ($result.schema -ne 'lu.ide.fin04w2.c2.native.final.v1' -or $result.sourceHead -ne $head -or $result.jobId -ne $manifest.jobId -or $result.planSha256 -ne $manifest.planSha256 -or $result.originalEventCount -ne 6 -or $result.terminalOutcomeCode -ne 'native_worker_completed' -or $result.restartReplay -ne 'CompletedNoGrant' -or $result.secondUse -ne 'Denied' -or $result.approvalKind -ne 'CI_SIMULATED_NOT_PHYSICAL') {
    throw 'DENIED: original native terminal/restart/replay receipt mismatch'
  }
  $nativeRows=@(Get-Content -LiteralPath $journal | Where-Object {$_} | ForEach-Object {$_|ConvertFrom-Json})
  $phases=@('prepared','authorized','admission_pending','running','release_pending','completed')
  if ($nativeRows.Count -ne 6) {throw 'DENIED: original journal did not fsync exactly six phases'}
  for ($i=0;$i -lt 6;$i++) {
    if ($nativeRows[$i].body.phase -ne $phases[$i] -or $nativeRows[$i].body.jobId -ne $manifest.jobId -or $nativeRows[$i].body.planSha256 -ne $manifest.planSha256 -or $nativeRows[$i].body.sessionId -ne $nativeRows[0].body.sessionId) {
      throw "DENIED: phase or immutable scope mismatch at original receipt $i"
    }
  }
  if ($nativeRows[5].entryHash -ne $result.terminalEntrySha256) {
    throw 'DENIED: original end-to-end fsynced terminal hash mismatch'
  }
  $nativeAccepted=$true
  Write-Host 'FIN04W2_C_ORIGINAL_NATIVE_SIX_DURABLE_RECEIPTS=PASS'
  Write-Host 'FIN04W2_C_ORIGINAL_NATIVE_RESTART_AND_ONE_USE_DENIAL=PASS'
  Write-Host 'FIN04W2_C_NATIVE_HUMAN_APPROVAL=CI_SIMULATED_NOT_PHYSICAL'
  Write-Host 'FIN04W2_C_LLDB_THREAD_RIP_PDB=NOT_TESTED'
} finally {
  if ($child) {
    $child.Refresh()
    if (!$child.HasExited) {
      Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
    }
  }
  if ($created) {
    & sc.exe stop $name 2>$null | Out-Null
    Start-Sleep -Seconds 2
    & sc.exe delete $name 2>$null | Out-Null
  }
  if ($wfpCreated) {
    Remove-NetFirewallRule -Name $ruleName -ErrorAction Stop
    if (Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue) {
      throw 'DENIED: WFP block rule not revoked after hosted test'
    }
  }
  if ($nativeAccepted -and (Test-Path -LiteralPath $serviceRoot)) {
    # ONLY after SCM stop is it permissible for disposable native CI admin
    # to reclaim the service-only evidence directory for independent audit.
    # It must remain inaccessible to the untrusted original desktop user
    # for the entire service execution.
    $stopped=$false
    for ($attempt=0;$attempt -lt 40;$attempt++) {
      $serviceState=Get-CimInstance Win32_Service -Filter "Name='$name'" -ErrorAction SilentlyContinue
      if (!$serviceState -or $serviceState.State -eq 'Stopped') {$stopped=$true;break}
      Start-Sleep -Milliseconds 250
    }
    if (!$stopped) {throw 'DENIED: cannot reclaim evidence while protected SCM still running'}
    & takeown.exe /F $serviceRoot /A /R /D Y | Out-Null
    if ($LASTEXITCODE -ne 0) {throw 'DENIED: post-SCM protected evidence takeover failed'}
    & icacls.exe $serviceRoot /grant:r 'Administrators:(OI)(CI)F' /T | Out-Null
    if ($LASTEXITCODE -ne 0) {throw 'DENIED: post-SCM evidence subtree ACL reclaim failed'}
    # The installer-protected config has a non-inheriting DACL. A parent
    # grant alone need not grant DELETE on this leaf. Reclaim ONLY after
    # independent service stop; never relax the runtime or source ACL.
    & icacls.exe $cfg /grant:r 'Administrators:F' | Out-Null
    if ($LASTEXITCODE -ne 0) {throw 'DENIED: post-SCM protected config ACL reclaim failed'}
    $oneUse=@(Get-ChildItem -LiteralPath $serviceRoot -File -Filter 'fin04w2-c2-*.receipt')
    if ($oneUse.Count -ne 1) {
      throw 'DENIED: exactly one original service-side durable one-use record required'
    }
    $text=Get-Content -LiteralPath $oneUse[0].FullName -Raw
    if ($text -notmatch 'fin04w2-c2-one-use-v1' -or $text -notmatch [regex]::Escape([string]$manifest.jobId) -or $text -notmatch [regex]::Escape([string]$manifest.planSha256)) {
      throw 'DENIED: actual service-side admission record scope mismatch'
    }
    $ownerCopy=Join-Path $ownerRoot 'service-one-use-verified.receipt'
    Copy-Item -LiteralPath $oneUse[0].FullName -Destination $ownerCopy -ErrorAction Stop
    $copy=Get-Content -LiteralPath $ownerCopy -Raw
    if ($copy -ne $text) {
      throw 'DENIED: independent original service receipt copy changed bytes'
    }
    foreach ($name in @('original-supervisor.jsonl','result.json','service-one-use-verified.receipt')) {
      $file=Join-Path $ownerRoot $name
      $hash=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
      Write-Host "FIN04W2_C_DURABLE_ARTIFACT_SHA256=${name}:$hash"
    }
    Write-Host 'FIN04W2_C_INDEPENDENT_SERVICE_SIDE_FSYNC_RECEIPT=PASS'
    Remove-Item -LiteralPath $serviceRoot -Recurse -Force -ErrorAction Stop
  }
  if (Test-Path -LiteralPath $negative) {Remove-Item -LiteralPath $negative -Force}
}
