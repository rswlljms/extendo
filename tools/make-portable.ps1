#Requires -Version 5.1
<#
.SYNOPSIS
  Build a portable extendo ZIP (no admin, no installer): compiles the viewer,
  stages host + viewer + tools + docs, and zips to dist/extendo-portable-<ver>.zip.
.USAGE
  powershell -ExecutionPolicy Bypass -File .\tools\make-portable.ps1
Unzip anywhere, then: node host/src/index.js | node client-web/serve.js 8080
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$ver = (Get-Content (Join-Path $root "host\package.json") | ConvertFrom-Json).version
$stage = Join-Path $root "dist\stage\extendo-$ver"
$zip = Join-Path $root "dist\extendo-portable-$ver.zip"

if (Test-Path -LiteralPath (Join-Path $root "dist\stage") ) { Remove-Item -Recurse -Force (Join-Path $root "dist\stage") }
New-Item -ItemType Directory -Force -Path $stage, "$stage\host", "$stage\client-web" | Out-Null

# 1. Compile viewer.ts -> dist/viewer.js (browsers cannot run raw .ts)
& npx -y -p typescript tsc (Join-Path $root "client-web\src\viewer.ts") `
  --outDir "$stage\client-web" --module esnext --moduleResolution bundler `
  --target es2022 --lib es2022,dom --strict --skipLibCheck
if (-not (Test-Path -LiteralPath "$stage\client-web\viewer.js")) { Write-Error "viewer compile failed" }

# 2. Stage files (ASCII-only ps1 enforced by repo checks; never bundle secrets)
Copy-Item "$root\host\src" "$stage\host\src" -Recurse
Copy-Item "$root\host\package.json" "$stage\host\package.json"
Copy-Item "$root\client-web\index.html" "$stage\client-web\index.html"
Copy-Item "$root\client-web\serve.js" "$stage\client-web\serve.js"
Copy-Item "$root\client-web\package.json" "$stage\client-web\package.json"
Copy-Item "$root\tools" "$stage\tools" -Recurse
Copy-Item "$root\proto" "$stage\proto" -Recurse
Copy-Item "$root\docs" "$stage\docs" -Recurse
Copy-Item "$root\driver\README.md" "$stage\driver-README.md"
Copy-Item "$root\README.md", "$root\CHANGELOG.md", "$root\AGENTS.md" $stage
(Get-Content "$stage\client-web\index.html") -replace "src/viewer.ts", "viewer.js" |
  Set-Content "$stage\client-web\index.html" -Encoding Ascii
"extendo portable $ver`nnode host/src/index.js`nnode client-web/serve.js 8080`n" |
  Set-Content "$stage\RUN.txt" -Encoding Ascii

# 3. Zip (overwrite)
if (Test-Path -LiteralPath $zip) { Remove-Item -Force $zip }
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Output "Portable ZIP: $zip"
