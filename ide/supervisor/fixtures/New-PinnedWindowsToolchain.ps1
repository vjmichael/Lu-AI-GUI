#requires -Version 7.2
# D7B acceptance ONLY. Never interpret a self-measured digest as
# trusted release provenance without a separately reviewed expected SHA.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$OutputRoot,
  [string]$RustVersion='1.99.1',
  [string]$SdkVersion='10.0.26100.0',
  [string]$VsVersionPrefix='17.14.',
  [string]$MsvcVersionPrefix='14.44.',
  [string]$ExpectedBundleSha256='',
  [string]$LockPath=''
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
function Require([string]$Path) {
  if (!(Test-Path -LiteralPath $Path)) { throw "DENIED: missing $Path" }
}
function No-Reparse([string]$Path) {
  Require $Path
  $all=@(Get-Item -LiteralPath $Path -Force)+@(Get-ChildItem -LiteralPath $Path -Force -Recurse)
  foreach ($item in $all) {
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "DENIED: reparse point: $($item.FullName)"
    }
  }
}
function Copy-Tree([string]$Source,[string]$Destination) {
  No-Reparse $Source
  New-Item -ItemType Directory -Path $Destination -Force | Out-Null
  Get-ChildItem -LiteralPath $Source -Force | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $Destination -Recurse -Force
  }
  No-Reparse $Destination
}
function Feed([System.Security.Cryptography.HashAlgorithm]$Hash,[byte[]]$Bytes) {
  if ($Bytes.Length -gt 0) { $null=$Hash.TransformBlock($Bytes,0,$Bytes.Length,$Bytes,0) }
}
function Ordered-Files([string]$Directory, [string]$Root) {
  # Match the native Rust tree_transfer exactly: ordinal sibling ordering,
  # recursive depth-first visitation, then relative POSIX separators.
  # A globally sorted list is *not* equivalent when a directory name is
  # also a prefix of a sibling filename.
  [string[]]$names=[IO.Directory]::GetFileSystemEntries($Directory)
  [Array]::Sort($names,[StringComparer]::Ordinal)
  foreach ($name in $names) {
    $item=Get-Item -LiteralPath $name -Force
    if ($item.PSIsContainer) {
      Ordered-Files $item.FullName $Root
    } else {
      [pscustomobject]@{
        path=$item.FullName.Substring($Root.TrimEnd('\').Length+1).Replace('\','/')
        full=$item.FullName
        bytes=$item.Length
      }
    }
  }
}
if ($env:OS -ne 'Windows_NT') { throw 'DENIED: native Windows required' }
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot)
if (!$LockPath) { $LockPath="$OutputRoot.lock.json" }
$LockPath=[IO.Path]::GetFullPath($LockPath)
if (Test-Path -LiteralPath $OutputRoot) { throw 'DENIED: bundle root must be exclusive' }
if ($LockPath.StartsWith(($OutputRoot.TrimEnd('\')+'\'),[StringComparison]::OrdinalIgnoreCase)) {
  throw 'DENIED: lock manifest must be outside executable bundle'
}
$rustcVersion=(& rustc --version).Trim()
if ($LASTEXITCODE -ne 0 -or !$rustcVersion.StartsWith("rustc $RustVersion (")) {
  throw "DENIED: pinned Rust version not installed: $rustcVersion"
}
$sysroot=(& rustc --print sysroot).Trim()
if ($LASTEXITCODE -ne 0) { throw 'DENIED: sysroot unavailable' }
$pf86=[Environment]::GetFolderPath('ProgramFilesX86')
$vswhere=Join-Path $pf86 'Microsoft Visual Studio\Installer\vswhere.exe'
Require $vswhere
$vsroot=(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath).Trim()
$vsver=(& $vswhere -latest -products '*' -property installationVersion).Trim()
if (!$vsroot -or !$vsver.StartsWith($VsVersionPrefix)) { throw "DENIED: VS pin $VsVersionPrefix not installed" }
$found=@(Get-ChildItem -LiteralPath (Join-Path $vsroot 'VC\Tools\MSVC') -Directory |
  Where-Object { $_.Name.StartsWith($MsvcVersionPrefix) })
if ($found.Count -ne 1) { throw "DENIED: exactly one pinned MSVC $MsvcVersionPrefix required" }
$vc=$found[0].FullName
$kit=Join-Path $pf86 'Windows Kits\10'
$inc=Join-Path $kit "Include\$SdkVersion"
$lib=Join-Path $kit "Lib\$SdkVersion"
$bin=Join-Path $kit "bin\$SdkVersion\x64"
$source=@(
  (Join-Path $sysroot 'bin'), (Join-Path $sysroot 'lib'),
  (Join-Path $vc 'bin\Hostx64\x64'), (Join-Path $vc 'lib\x64'),
  (Join-Path $vc 'include'), $bin,
  (Join-Path $inc 'um'),(Join-Path $inc 'ucrt'),(Join-Path $inc 'shared'),
  (Join-Path $lib 'um\x64'),(Join-Path $lib 'ucrt\x64')
)
foreach ($item in $source) { No-Reparse $item }
New-Item -ItemType Directory -Path $OutputRoot | Out-Null
try {
  Copy-Tree (Join-Path $sysroot 'bin') (Join-Path $OutputRoot 'bin')
  Copy-Tree (Join-Path $sysroot 'lib') (Join-Path $OutputRoot 'lib')
  Copy-Tree (Join-Path $vc 'bin\Hostx64\x64') (Join-Path $OutputRoot 'bin')
  Copy-Tree (Join-Path $vc 'lib\x64') (Join-Path $OutputRoot 'msvc\lib\x64')
  Copy-Tree (Join-Path $vc 'include') (Join-Path $OutputRoot 'msvc\include')
  Copy-Tree $bin (Join-Path $OutputRoot 'sdk-bin')
  foreach ($part in @('um','ucrt','shared')) {
    Copy-Tree (Join-Path $inc $part) (Join-Path $OutputRoot "windows-sdk\include\$part")
  }
  foreach ($part in @('um','ucrt')) {
    Copy-Tree (Join-Path $lib "$part\x64") (Join-Path $OutputRoot "windows-sdk\lib\$part\x64")
  }
  New-Item -ItemType Directory -Path (Join-Path $OutputRoot 'cargo-home') | Out-Null
  foreach ($need in @('bin\cargo.exe','bin\rustc.exe','bin\link.exe','bin\cl.exe',
    'sdk-bin\rc.exe','sdk-bin\mt.exe','msvc\lib\x64\libcmt.lib',
    'windows-sdk\lib\um\x64\kernel32.lib','windows-sdk\lib\ucrt\x64\ucrt.lib')) {
    Require (Join-Path $OutputRoot $need)
  }
  No-Reparse $OutputRoot
  $files=@(Ordered-Files $OutputRoot $OutputRoot)
  if ($files.Count -gt 60000) { throw 'DENIED: >60,000 bundle files' }
  $total=($files|Measure-Object -Property bytes -Sum).Sum
  if ($total -gt 8GB) { throw 'DENIED: >8GiB bundle' }
  $hash=[Security.Cryptography.SHA256]::Create()
  $entries=New-Object 'System.Collections.Generic.List[object]'
  foreach ($file in $files) {
    if ($file.bytes -gt 1GB) { throw "DENIED: oversized file $($file.path)" }
    $digest=(Get-FileHash -LiteralPath $file.full -Algorithm SHA256).Hash.ToLowerInvariant()
    Feed $hash ([Text.Encoding]::UTF8.GetBytes($file.path))
    Feed $hash ([byte[]]@(0))
    Feed $hash ([BitConverter]::GetBytes([long]$file.bytes))
    Feed $hash ([Text.Encoding]::ASCII.GetBytes($digest))
    Feed $hash ([byte[]]@(0))
    $entries.Add([pscustomobject]@{path=$file.path;bytes=$file.bytes;sha256=$digest})
  }
  $null=$hash.TransformFinalBlock([byte[]]::new(0),0,0)
  $bundleDigest=[Convert]::ToHexString($hash.Hash).ToLowerInvariant()
  if ($ExpectedBundleSha256 -and
      $ExpectedBundleSha256.ToLowerInvariant() -cne $bundleDigest) {
    throw "DENIED: reviewed bundle digest mismatch: $bundleDigest"
  }
  $lock=[ordered]@{
    schema='lu.ide.windows-toolchain.v1'
    acceptanceOnly=$true
    reviewedPinMatched=[bool]$ExpectedBundleSha256
    rust=$rustcVersion
    visualStudio=$vsver
    msvc=$found[0].Name
    windowsSdk=$SdkVersion
    target='x86_64-pc-windows-msvc'
    bundleSha256=$bundleDigest
    fileCount=$files.Count
    totalBytes=$total
    files=$entries.ToArray()
  }
  New-Item -ItemType Directory -Path (Split-Path -Parent $LockPath) -Force | Out-Null
  $lock|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $LockPath -Encoding utf8 -NoNewline
  Write-Host "BUNDLE_SHA256=$bundleDigest"
  if ($env:GITHUB_ENV) {
    Add-Content $env:GITHUB_ENV "LU_IDE_D7_NATIVE_TOOLCHAIN=$OutputRoot"
    Add-Content $env:GITHUB_ENV "LU_IDE_D7_BUNDLE_LOCK=$LockPath"
  }
} catch {
  Remove-Item -LiteralPath $OutputRoot -Recurse -Force -ErrorAction SilentlyContinue
  throw
}
