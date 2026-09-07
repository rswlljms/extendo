#Requires -Version 5.1
<#
.SYNOPSIS
  Build the extendo installer (needs .NET SDK; run once: dotnet tool restore).
  Validates version sync (host/package.json == client-web/package.json == wxs)
  before invoking WiX. Produces installer/out/extendo-0.4.0.msi (+ bundle exe).
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $root "installer")

$hv = (Get-Content (Join-Path $root "host\package.json") | ConvertFrom-Json).version
$wv = (Get-Content (Join-Path $root "client-web\package.json") | ConvertFrom-Json).version
$wxv = (Select-String -Path "Package.wxs" -Pattern 'Version="([0-9.]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if ($hv -ne $wv -or $hv -ne $wxv) {
  Write-Error "Version drift: host=$hv web=$wv wxs=$wxv. Sync all three first."
}
if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
  Write-Error ".NET SDK required: https://dotnet.microsoft.com/download (then: dotnet tool restore)"
}

New-Item -ItemType Directory -Force -Path "out" | Out-Null
& dotnet tool restore
# Compile the viewer (index.html loads dist/viewer.js, never raw .ts)
& npx -y -p typescript tsc (Join-Path $root "client-web\src\viewer.ts") `
  --outDir (Join-Path $root "client-web\dist") --module esnext --moduleResolution bundler `
  --target es2022 --lib es2022,dom --strict --skipLibCheck
& dotnet wix build Package.wxs -ext WixToolset.Firewall.wixext -o "out\extendo-$hv.msi"
Write-Output "MSI at installer/out/extendo-$hv.msi"
Write-Output "Sign it before release: signtool sign /fd SHA256 /a out\extendo-$hv.msi (EV cert, AGENTS.md 7.2)"
