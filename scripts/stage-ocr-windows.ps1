# TOOLS-COMPLETE-B: build-time, trusted OFFLINE OCR packaging.
# No downloads, elevation, PATH lookup or runtime install on end-user machine.
# A reviewed release manifest lists EVERY executable, DLL and English data
# asset with its exact SHA256. Do not ship Tesseract until licenses are cleared.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$RuntimeSource,
  [Parameter(Mandatory=$true)][string]$ApprovedManifest
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$destination = Join-Path $repo 'lu-desktop/src-tauri/resources/lu-ocr'
$source = (Resolve-Path -LiteralPath $RuntimeSource).Path
$manifestFile = (Resolve-Path -LiteralPath $ApprovedManifest).Path
$manifest = Get-Content -LiteralPath $manifestFile -Raw | ConvertFrom-Json
if (@($manifest.files).Count -lt 2 -or @($manifest.files).Count -gt 80) {
  throw 'Approved OCR runtime must contain 2–80 specifically pinned files'
}
if (-not (Test-Path -LiteralPath $source -PathType Container)) {
  throw 'OCR source must be a directory'
}
$files = @{}
foreach($item in $manifest.files) {
  $relative = [string]$item.relative_path
  $expected = ([string]$item.sha256).ToLowerInvariant()
  if ($relative.Length -gt 240 -or $relative -notmatch '^[A-Za-z0-9][A-Za-z0-9._/-]*$' -or
      $relative.Contains('//') -or $relative.Contains('..') -or
      $relative.StartsWith('/') -or $relative.EndsWith('/') -or
      $relative.Contains('\') -or $relative.Contains(':') -or
      $expected -notmatch '^[0-9a-f]{64}$') {
    throw 'Unsafe OCR manifest file or digest'
  }
  $key = $relative.ToLowerInvariant()
  if ($files.ContainsKey($key)) { throw 'Duplicate OCR runtime file' }
  $files[$key] = $expected
}
if (-not $files.ContainsKey('tesseract.exe') -or
    -not $files.ContainsKey('tessdata/eng.traineddata')) {
  throw 'OCR runtime requires pinned Tesseract.exe and English traineddata'
}
$listed = @()
foreach($item in Get-ChildItem -LiteralPath $source -File -Recurse -Force) {
  if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw 'OCR source may not include junctions or symlinks'
  }
  $relative = [IO.Path]::GetRelativePath($source,$item.FullName).Replace('\','/')
  $key = $relative.ToLowerInvariant()
  if (-not $files.ContainsKey($key)) {
    throw "Unreviewed OCR executable or data in runtime folder: $relative"
  }
  $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $item.FullName).Hash.ToLowerInvariant()
  if ($hash -cne $files[$key]) {
    throw "OCR reviewed content checksum mismatch: $relative"
  }
  $listed += $key
}
if ($listed.Count -ne $files.Count) { throw 'OCR manifest references missing runtime files' }
if (Test-Path -LiteralPath $destination) {
  Remove-Item -LiteralPath $destination -Recurse -Force
}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
foreach($item in $manifest.files) {
  $relative = [string]$item.relative_path
  $src = Join-Path $source ($relative.Replace('/',[IO.Path]::DirectorySeparatorChar))
  $dst = Join-Path $destination ($relative.Replace('/',[IO.Path]::DirectorySeparatorChar))
  $parent = Split-Path -Parent $dst
  New-Item -ItemType Directory -Path $parent -Force | Out-Null
  Copy-Item -LiteralPath $src -Destination $dst
  $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $dst).Hash.ToLowerInvariant()
  if ($actual -cne $files[$relative.ToLowerInvariant()]) {
    throw "Staged OCR content changed: $relative"
  }
}
$exeDigest = $files['tesseract.exe']
$env:LU_OCR_TESSERACT_SHA256 = $exeDigest
$engDigest = $files['tessdata/eng.traineddata']
$env:LU_OCR_ENG_SHA256 = $engDigest
if ($env:GITHUB_ENV) {
  Add-Content -LiteralPath $env:GITHUB_ENV -Value "LU_OCR_TESSERACT_SHA256=$exeDigest"
  Add-Content -LiteralPath $env:GITHUB_ENV -Value "LU_OCR_ENG_SHA256=$engDigest"
}
Write-Output 'Offline OCR resources staged and SHA-256 checked. Build with the Tauri OCR overlay.'
Write-Output 'Bundling still requires a reviewed license manifest and independent native acceptance.'
