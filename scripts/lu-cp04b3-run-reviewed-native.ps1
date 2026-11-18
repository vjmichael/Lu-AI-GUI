# Unattended native acceptance only AFTER an independent review record has
# been committed. This script does not author or update any approval record.
param(
  [Parameter(Mandatory=$true)][string]$CandidateDir,
  [Parameter(Mandatory=$true)][string]$EnginePath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$EngineSha256,
  [Parameter(Mandatory=$true)][string]$ModelPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ModelSha256,
  [Parameter(Mandatory=$true)][string]$EvidencePath
)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'Requires network-capable native Windows'}
$repo=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $repo
$owned=@();$staged=$false;$restoreManifest=$null;$reviewed=$null
try {
  node scripts/verify-source-tree.mjs
  if($LASTEXITCODE -ne 0){throw 'Source inventory mismatch'}
  $CandidateDir=(Resolve-Path -LiteralPath $CandidateDir).Path
  $EnginePath=(Resolve-Path -LiteralPath $EnginePath).Path
  $ModelPath=(Resolve-Path -LiteralPath $ModelPath).Path
  $EvidencePath=[IO.Path]::GetFullPath($EvidencePath)
  if(Test-Path -LiteralPath $EvidencePath){throw 'Refuse to overwrite acceptance evidence'}
  if((Get-FileHash -LiteralPath $EnginePath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $EngineSha256.ToLowerInvariant() -or (Get-FileHash -LiteralPath $ModelPath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $ModelSha256.ToLowerInvariant()){throw 'Engine/model pin mismatch'}
  $review=Get-Content 'docs/acceptance/LU_C62_WINDOWS_BROWSER_REVIEW.json' -Raw|ConvertFrom-Json
  $manifest=Get-Content -LiteralPath (Join-Path $CandidateDir 'manifest.json') -Raw|ConvertFrom-Json
  $canonical=(Get-FileHash 'src-tauri/lu-core/workers/lu_browser_worker.mjs' -Algorithm SHA256).Hash.ToLowerInvariant()
  $worker=@($manifest.files|Where-Object path -ceq 'lu_browser_worker.mjs')
  if($worker.Count -ne 1 -or $worker[0].sha256 -cne $canonical){throw 'Candidate worker is stale'}
  $root=Join-Path $repo 'lu-desktop/src-tauri/resources/lu-browser'
  $stub=Join-Path $root 'manifest.json'
  if(@(Get-ChildItem -LiteralPath $root -Force).Count -ne 1 -or (Get-Content -LiteralPath $stub -Raw|ConvertFrom-Json).status -cne 'not-packaged'){throw 'Requires a clean placeholder browser resource directory'}
  $restoreManifest=[IO.File]::ReadAllBytes($stub)
  $reviewed=Join-Path ([IO.Path]::GetTempPath()) ('lu-cp04b3-reviewed-'+[guid]::NewGuid().ToString('N'))
  $env:LU_BROWSER_CANDIDATE_DIR=$CandidateDir;$env:LU_BROWSER_REVIEWED_OUT=$reviewed
  node lu-desktop/scripts/promote-reviewed-browser-candidate.mjs
  if($LASTEXITCODE -ne 0){throw 'Independent review promotion failed'}
  $env:LU_BROWSER_PACKAGE_DIR=$reviewed;$env:LU_BROWSER_MANIFEST_SHA256=$review.reviewedManifestSha256
  node lu-desktop/scripts/stage-browser-runtime.mjs
  if($LASTEXITCODE -ne 0){throw 'Reviewed browser staging failed'}
  $staged=$true
  npm --prefix lu-desktop ci
  if($LASTEXITCODE -ne 0){throw 'Frontend dependencies failed'}
  npm --prefix lu-desktop run build
  if($LASTEXITCODE -ne 0){throw 'Frontend build failed'}
  $inference=Join-Path $repo 'lu-desktop/src-tauri/resources/lu-inference'
  $resources=@{'llama-server.exe'=$EnginePath}
  foreach($name in @('msvcp140.dll','vcruntime140.dll','vcruntime140_1.dll')){$resources[$name]=Join-Path $env:SystemRoot "System32/$name"}
  foreach($name in $resources.Keys){
    $path=Join-Path $inference $name
    if(Test-Path -LiteralPath $path){throw 'Refuse to overwrite existing inference resource'}
    if(-not(Test-Path -LiteralPath $resources[$name] -PathType Leaf)){throw "Missing genuine resource $name"}
    Copy-Item -LiteralPath $resources[$name] -Destination $path;$owned+= $path
  }
  $env:LU_CP04B2_LIVE_ACCEPTANCE='1';$env:LU_SERVER_EXE=$EnginePath;$env:LU_SERVER_SHA256=$EngineSha256
  $env:LU_TEST_GGUF=$ModelPath;$env:LU_TEST_GGUF_SHA256=$ModelSha256;$env:LU_SERVER_API_KEY=[guid]::NewGuid().ToString('N')
  cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --lib cp04b2_live_protected_provider_and_genuine_gguf -- --ignored --nocapture 2>&1|Tee-Object -FilePath $EvidencePath
  if($LASTEXITCODE -ne 0){throw 'Protected native live provider-to-GGUF gate failed'}
  if(-not(Select-String -LiteralPath $EvidencePath -Pattern 'CP04B2_LIVE_PROVIDER_PASS receipt=' ) -or -not(Select-String -LiteralPath $EvidencePath -Pattern 'CP04B2_GENUINE_GGUF_CITATION_PASS') -or -not(Select-String -LiteralPath $EvidencePath -Pattern 'test result: ok\. 1 passed; 0 failed;')){throw 'Missing actual native browser or GGUF receipt'}
} finally {
  foreach($path in $owned){Remove-Item -LiteralPath $path -Force}
  if($staged){Remove-Item -LiteralPath $root -Recurse -Force;New-Item -ItemType Directory -Path $root|Out-Null;[IO.File]::WriteAllBytes($stub,$restoreManifest)}
  if($reviewed -and (Test-Path -LiteralPath $reviewed)){Remove-Item -LiteralPath $reviewed -Recurse -Force}
  foreach($name in @('LU_BROWSER_CANDIDATE_DIR','LU_BROWSER_REVIEWED_OUT','LU_BROWSER_PACKAGE_DIR','LU_BROWSER_MANIFEST_SHA256','LU_CP04B2_LIVE_ACCEPTANCE','LU_SERVER_EXE','LU_SERVER_SHA256','LU_TEST_GGUF','LU_TEST_GGUF_SHA256','LU_SERVER_API_KEY')){Remove-Item "Env:$name" -ErrorAction SilentlyContinue}
  Pop-Location
}
