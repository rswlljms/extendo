#Requires -Version 5.1
<#
.SYNOPSIS
  Set up ADB reverse forwarding for extendo USB mode.
.USAGE
  powershell -ExecutionPolicy Bypass -File .\tools\adb-reverse.ps1 [-Port 9577] [-AdbPath auto]
Steps: adb devices -> authorize RSA on phone -> adb reverse tcp:PORT tcp:PORT.
Client then connects to 127.0.0.1:PORT (no router needed).
#>
param([int]$Port = 9577, [string]$AdbPath = "auto")

if ($AdbPath -eq "auto") {
  $sdk = Join-Path $env:LOCALAPPDATA "Android\Sdk\platform-tools\adb.exe"
  $bundled = Join-Path $PSScriptRoot "adb\adb.exe"
  if (Test-Path -LiteralPath $bundled) { $AdbPath = $bundled }
  elseif (Test-Path -LiteralPath $sdk) { $AdbPath = $sdk }
  else { $AdbPath = "adb" }
}

Write-Output "Using adb: $AdbPath"
& $AdbPath devices
if (-not $?) { Write-Error "adb failed. Install platform-tools and enable USB debugging on the phone."; exit 1 }

& $AdbPath reverse "tcp:$Port" "tcp:$Port"
if ($?) { Write-Output "OK: phone 127.0.0.1:$Port -> PC 0.0.0.0:$Port. In the app connect to 127.0.0.1." }
else { Write-Error "adb reverse failed. States: unauthorized (accept RSA on phone) / offline (replug, USB-2 port, data cable, PTP mode)." }
