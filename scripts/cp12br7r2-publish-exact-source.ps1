# CP-12B-R7-R2: deterministic, no-force first publication of an original Git bundle.
# No credentials or diagnostic data are embedded. Preflight is read-only unless -Publish is passed.
[CmdletBinding()]
param(
  [string]$BundlePath = (Join-Path $PSScriptRoot 'Lu-CP-12B-R7-R2-Full-Source.bundle'),
  [string]$ManifestPath = (Join-Path $PSScriptRoot 'Lu-CP-12B-R7-R2-Publication-Manifest.json'),
  [switch]$Publish
)
$ErrorActionPreference = 'Stop'
$repo = 'vjmichael/Lu-AI-GUI'
$branch = 'lu/cp12b-r7-windows-stage-eval'
$remote = "https://github.com/$repo.git"
function Assert-Exit([string]$step) {
  if ($LASTEXITCODE -ne 0) { throw "$step exited $LASTEXITCODE" }
}
function Git([string[]]$arguments) {
  $output = & git @arguments
  Assert-Exit 'git'
  return $output
}
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'git is required' }
if (-not (Get-Command node -ErrorAction SilentlyContinue)) { throw 'Node.js is required for the exact-source lock verifier' }
$BundlePath = (Resolve-Path -LiteralPath $BundlePath).Path
$ManifestPath = (Resolve-Path -LiteralPath $ManifestPath).Path
$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
$sha = [string]$manifest.source_sha
$tree = [string]$manifest.tree_sha
$blobDigest = [string]$manifest.bundle_sha256
$trackedCount = [int]$manifest.tracked_entries
if ($manifest.repository -cne $repo -or $manifest.branch -cne $branch) { throw 'Unrecognized publication destination' }
foreach ($hash in @($sha, $tree, $blobDigest)) {
  if ($hash -cnotmatch '^[0-9a-f]{40}([0-9a-f]{24})?$') { throw 'Invalid SHA format' }
}
if ($sha.Length -ne 40 -or $tree.Length -ne 40 -or $blobDigest.Length -ne 64 -or $trackedCount -lt 1) {
  throw 'Invalid SHA lengths or source count'
}
$actualBundle = (Get-FileHash -LiteralPath $BundlePath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualBundle -cne $blobDigest) { throw 'Bundle checksum mismatch; refusal before any remote access' }
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ('lu-cp12br7r2-' + [Guid]::NewGuid().ToString('N'))
$checkout = Join-Path $tempRoot 'source'
New-Item -ItemType Directory -Path $tempRoot | Out-Null
try {
  Git @('clone', '--quiet', '--no-checkout', '--', $BundlePath, $checkout) | Out-Null
  Git @('-C', $checkout, 'checkout', '--quiet', '--detach', $sha) | Out-Null
  $actualSha = [string](Git @('-C', $checkout, 'rev-parse', 'HEAD'))
  $actualTree = [string](Git @('-C', $checkout, 'rev-parse', 'HEAD^{tree}'))
  if ($actualSha.Trim() -cne $sha -or $actualTree.Trim() -cne $tree) { throw 'Exact local commit or Git tree mismatch' }
  $entries = @(Git @('-C', $checkout, 'ls-files'))
  if (($entries.Count - 1) -ne $trackedCount) { throw 'Tracked inventory count mismatch' }
  if (@(Git @('-C', $checkout, 'status', '--porcelain')).Count -ne 0) { throw 'Restored checkout is dirty' }
  Push-Location $checkout
  try {
    & node scripts/verify-source-tree.mjs
    Assert-Exit 'source lock verification'
  } finally { Pop-Location }
  Write-Output "LOCAL_SOURCE_VERIFIED=$sha"
  Write-Output "LOCAL_TREE_VERIFIED=$tree"
  if (-not $Publish) {
    Write-Output 'REMOTE_PUBLICATION=NOT_ATTEMPTED (preflight; use -Publish only in an authenticated Git environment)'
    return
  }
  $remoteLine = @(& git -C $checkout ls-remote --heads $remote "refs/heads/$branch")
  Assert-Exit 'remote branch discovery'
  $remoteLine = @($remoteLine | Where-Object { $_ })
  if ($remoteLine.Count -gt 1) { throw 'Multiple remote branch matches; refusal' }
  if ($remoteLine.Count -eq 1) {
    $existing = ($remoteLine[0] -split '\s+')[0]
    if ($existing -cne $sha) { throw "Remote branch unexpectedly exists at $existing; no overwrite or force-push allowed" }
    Write-Output "REMOTE_ALREADY_MATCHED=$sha"
  } else {
    & git -C $checkout push --porcelain $remote "${sha}:refs/heads/$branch"
    Assert-Exit 'Git push of exact original commit'
  }
  $readback = @(& git -C $checkout ls-remote --heads $remote "refs/heads/$branch")
  Assert-Exit 'remote readback'
  if ($readback.Count -ne 1 -or (($readback[0] -split '\s+')[0] -cne $sha)) { throw 'Remote SHA readback did not match' }
  Write-Output "REMOTE_SOURCE_VERIFIED=$sha"
  Write-Output 'HOSTED_CI=NOT_YET_VERIFIED (the scoped first-push workflow is eligible; inspect actual run ID and logs separately)'
  Write-Output "ACTIONS=https://github.com/$repo/actions/workflows/lu-cp12br7r1-windows-collaboration.yml"
} finally {
  Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
