#Requires -Version 5.1
<#
.SYNOPSIS
  Collect extendo diagnostics bundle (no PII beyond IPs needed for LAN debug).
.USAGE
  powershell -ExecutionPolicy Bypass -File .\tools\diag-collect.ps1 [-HostPort 9577]
#>
param([int]$HostPort = 9577)

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$out = Join-Path ([IO.Path]::GetTempPath()) "extendo-diag-$stamp"
New-Item -ItemType Directory -Force -Path $out | Out-Null

Get-NetAdapter | Where-Object { $_.Status -eq "Up" } | Format-Table -AutoSize Name, InterfaceDescription, Status | Out-File (Join-Path $out "adapters.txt")
Get-NetIPAddress -AddressFamily IPv4 | Format-Table -AutoSize IPAddress, InterfaceIndex | Out-File (Join-Path $out "ips.txt") -Append
try { (Invoke-WebRequest -Uri "http://127.0.0.1:$HostPort/stats" -TimeoutSec 5).Content | Out-File (Join-Path $out "host-stats.json") }
catch { "host not reachable on 127.0.0.1:$HostPort ($($_.Exception.Message))" | Out-File (Join-Path $out "host-stats.json") }
try { (Invoke-WebRequest -Uri "http://127.0.0.1:$HostPort/info" -TimeoutSec 5).Content | Out-File (Join-Path $out "host-info.json") }
catch { "no /info" | Out-File (Join-Path $out "host-info.json") }

Write-Output "Diagnostics written to $out — attach host-stats.json + adapters.txt to bug reports."
