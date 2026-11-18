# WAVE2-CODECS: stage vetted Windows libarchive and its native dependencies
# into the Lu desktop NSIS resource directory and optional test executable dir.
# This script must run on a trusted build agent AFTER pinned vcpkg build.
# It NEVER downloads arbitrary binaries or changes the user's PATH.
param(
  [Parameter(Mandatory=$true)][string]$VcpkgInstalledRoot,
  [string]$ResourceDir = "lu-desktop/src-tauri/resources/lu-native-codecs",
  [string]$TestExecutableDir = ""
)
$ErrorActionPreference = "Stop"
$source = Join-Path $VcpkgInstalledRoot "bin"
if (!(Test-Path -LiteralPath $source -PathType Container)) {
  throw "Pinned vcpkg triplet bin directory unavailable: $source"
}
$dlls = @(Get-ChildItem -LiteralPath $source -Filter '*.dll' -File)
$archive = @($dlls | Where-Object {
  $_.Name -ieq 'archive.dll' -or $_.Name -ieq 'libarchive.dll'
})
if ($archive.Count -ne 1) {
  throw "Expected exactly one native libarchive.dll or archive.dll, found $($archive.Count)"
}
if ($dlls.Count -lt 2) {
  throw "Native libarchive binary set lacks transitive codecs; refusing incomplete package"
}
$targets = @($ResourceDir)
if ($TestExecutableDir -ne "") { $targets += (Join-Path $TestExecutableDir "lu-native-codecs") }
foreach ($dest in $targets) {
  New-Item -ItemType Directory -Force -Path $dest | Out-Null
  Get-ChildItem -LiteralPath $dest -Filter '*.dll' -File | Remove-Item -Force
  foreach ($item in $dlls) {
    Copy-Item -LiteralPath $item.FullName -Destination (Join-Path $dest $item.Name) -Force
  }
  $files = @(Get-ChildItem -LiteralPath $dest -Filter '*.dll' -File | Sort-Object Name |
    ForEach-Object {
      @{
        name = $_.Name
        sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
        bytes = $_.Length
      }
    })
  $manifest = @{
    schema = 1
    origin = "pinned vcpkg x64-windows libarchive with bzip2,lzma,zstd"
    purpose = "Windows native file archive/codec decoding only, never shell"
    files = $files
  }
  ($manifest | ConvertTo-Json -Depth 9) | Set-Content -LiteralPath (Join-Path $dest "manifest.json") -Encoding UTF8
}
# Preserve the upstream and codec license texts for the build's legal review.
$share = Join-Path $VcpkgInstalledRoot "share"
$licenseDir = Join-Path $ResourceDir "licenses"
New-Item -ItemType Directory -Force -Path $licenseDir | Out-Null
foreach ($project in @('libarchive','zlib','bzip2','liblzma','zstd','lz4','libiconv','openssl')) {
  foreach ($file in @('copyright','LICENSE','license.txt')) {
    $candidate = Join-Path (Join-Path $share $project) $file
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
      Copy-Item -LiteralPath $candidate -Destination (Join-Path $licenseDir "$project-$file") -Force
      break
    }
  }
}
Write-Host "Native Lu codecs staged: $($dlls.Count) vetted vcpkg DLLs, manifests and available licensing files"
