#Requires -Version 5.1
<#
.SYNOPSIS
  Probe local interfaces and classify extendo transports (wifi | rndis | localhost).
.USAGE
  powershell -ExecutionPolicy Bypass -File .\tools\iface-probe.ps1 [-Port 9577]
#>
param([int]$Port = 9577)

$adapters = Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq "Up" }
$addrs = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike "127.*" -and $_.IPAddress -notlike "169.254.*" }

foreach ($a in $addrs) {
  $kind = "wifi"
  if ($a.IPAddress -like "192.168.42.*" -or $a.IPAddress -like "192.168.43.*" -or $a.IPAddress -like "192.168.137.*") { $kind = "rndis" }
  $ifName = ($adapters | Where-Object { $_.ifIndex -eq $a.InterfaceIndex } | Select-Object -First 1).InterfaceDescription
  [pscustomobject]@{
    Kind = $kind
    Addr = $a.IPAddress
    Port = $Port
    Interface = $ifName
    Url = "http://$($a.IPAddress):$Port/info"
  } | Format-Table -AutoSize | Out-String | Write-Output
}
Write-Output "localhost  127.0.0.1:$Port  (ADB reverse target)"
