#Requires -Version 5.1
<#
.SYNOPSIS
  Build the extendo installer (needs .NET SDK; run once: dotnet tool restore).
  Validates version sync (host/package.json == client-web/package.json == wxs)
  before invoking WiX. Produces installer/out/extendo-0.5.0.msi (+ bundle exe).
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $root "installer")

$hv = (Get-Content (Join-Path $root "host\package.json") | ConvertFrom-Json).version
$wv = (Get-Content (Join-Path $root "client-web\package.json") | ConvertFrom-Json).version
$wxv = (Select-String -Path "Package.wxs" -Pattern 'Version="([0-9.]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$wxb = (Select-String -Path "Bundle.wxs" -Pattern 'Version="([0-9.]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if ($hv -ne $wv -or $hv -ne $wxv -or $hv -ne $wxb) {
  Write-Error "Version drift: host=$hv web=$wv package=$wxv bundle=$wxb. Sync all four first."
}
if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
  Write-Error ".NET SDK required: https://dotnet.microsoft.com/download (then: dotnet tool restore)"
}

New-Item -ItemType Directory -Force -Path "out" | Out-Null
& dotnet tool restore
if ($LASTEXITCODE -ne 0) { Write-Error "dotnet tool restore failed ($LASTEXITCODE)" }
# WiX extensions are binaries (not in git) — restore them into installer/.wix per build.
$wixVer = (Get-Content ".config\dotnet-tools.json" | ConvertFrom-Json).tools.wix.version
foreach ($ext in "WixToolset.Firewall.wixext", "WixToolset.Bal.wixext") {
  & dotnet wix extension add "$ext/$wixVer"
  if ($LASTEXITCODE -ne 0) { Write-Error "wix extension add $ext failed ($LASTEXITCODE)" }
}
# Compile the viewer (index.html loads dist/viewer.js, never raw .ts)
& npx -y -p typescript tsc (Join-Path $root "client-web\src\viewer.ts") `
  --outDir (Join-Path $root "client-web\dist") --module esnext --moduleResolution bundler `
  --target es2022 --lib es2022,dom --strict --skipLibCheck
if ($LASTEXITCODE -ne 0) { Write-Error "viewer compile failed ($LASTEXITCODE)" }
& dotnet wix build Package.wxs -ext WixToolset.Firewall.wixext -o "out\extendo-$hv.msi"
if ($LASTEXITCODE -ne 0) { Write-Error "MSI build failed ($LASTEXITCODE)" }
& dotnet wix build Bundle.wxs -ext WixToolset.Bal.wixext -d MsiPath="out\extendo-$hv.msi" -o "out\extendo-setup-$hv.exe"
if ($LASTEXITCODE -ne 0) { Write-Error "Bundle build failed ($LASTEXITCODE)" }
Write-Output "MSI at installer/out/extendo-$hv.msi"
Write-Output "Bundle at installer/out/extendo-setup-$hv.exe"
Write-Output "Sign it before release: signtool sign /fd SHA256 /a out\extendo-$hv.msi (EV cert, AGENTS.md 7.2)"
