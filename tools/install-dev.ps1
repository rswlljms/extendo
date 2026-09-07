#Requires -Version 5.1
<#
.SYNOPSIS
  Dev installer for extendo (closest thing to setup.exe before the Phase-3 WiX bundle).
  No admin needed except for the firewall rule and VDD driver install (both optional).
.USAGE
  powershell -ExecutionPolicy Bypass -File .\tools\install-dev.ps1 [-Port 9577] [-Firewall] [-Startup] [-Vdd]
  -Firewall : add Windows Firewall allow rule for the host port (needs admin)
  -Startup  : register a logon Scheduled Task running the host (current user)
  -Vdd      : open the signed VDD release page for manual install (driver/README.md)
#>
param([int]$Port = 9577, [switch]$Firewall, [switch]$Startup, [switch]$Vdd)

$root = Split-Path -Parent $PSScriptRoot
$fail = $false

# 1. Node >= 18 (host + viewer runtime)
try {
  $v = (& node --version) -replace "v", ""
  if ([version]$v -lt [version]"18.0.0") { Write-Error "Node >= 18 required (found $v). https://nodejs.org"; $fail = $true }
  else { Write-Output "Node $v OK" }
} catch { Write-Error "Node.js not found. Install LTS from https://nodejs.org then re-run."; $fail = $true }

# 2. Port free?
$busy = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
if ($busy) { Write-Warning "Port $Port already listening (another host running?). Use -Port or stop it." }

# 3. First-run config (random PIN + token) so the viewer can pair immediately
$cfgDir = Join-Path $env:APPDATA "extendo"
if (-not (Test-Path -LiteralPath (Join-Path $cfgDir "config.json"))) {
  Write-Output "First run: generating config (PIN + token)..."
  Push-Location (Join-Path $root "host")
  Start-Job -Name extendo-init -ScriptBlock { param($r, $p) Set-Location $r; node src/index.js --port $p } -ArgumentList (Join-Path $root "host"), $Port | Out-Null
  Start-Sleep -Seconds 3
  Stop-Job -Name extendo-init -ErrorAction SilentlyContinue | Out-Null
  Remove-Job -Name extendo-init -Force -ErrorAction SilentlyContinue | Out-Null
  Pop-Location
}
$cfg = Get-Content (Join-Path $cfgDir "config.json") | ConvertFrom-Json
Write-Output ("Host: http://<this-PC-IP>:{0}   PIN: {1}" -f $Port, $cfg.pin)
& powershell -ExecutionPolicy Bypass -File (Join-Path $root "tools\iface-probe.ps1") -Port $Port

# 4. Firewall rule (admin)
if ($Firewall) {
  $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  if (-not $admin) { Write-Error "Re-run as admin for -Firewall (right-click PowerShell > Run as administrator)."; $fail = $true }
  elseif (-not (Get-NetFirewallRule -DisplayName "extendo host" -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -DisplayName "extendo host" -Direction Inbound -Action Allow -Protocol TCP -LocalPort $Port | Out-Null
    Write-Output "Firewall rule added for TCP $Port"
  } else { Write-Output "Firewall rule already present" }
}

# 5. Logon task (current user, no admin)
if ($Startup) {
  $act = New-ScheduledTaskAction -Execute "node" -Argument "`"$root\host\src\index.js`" --port $Port" -WorkingDirectory "$root\host"
  $trg = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
  Register-ScheduledTask -TaskName "extendo host" -Action $act -Trigger $trg -Force | Out-Null
  Write-Output "Logon task 'extendo host' registered (schtasks /delete /tn `"extendo host`" /f to remove)"
}

# 6. Virtual display driver (manual, admin, reboot may apply)
if ($Vdd) {
  Write-Output "Opening signed VDD release page - install, reboot if asked, then POST /displays to extend."
  Start-Process "https://github.com/VirtualDrivers/Virtual-Display-Driver/releases"
}

if ($fail) { exit 1 }
Write-Output "Done. Start host: node host/src/index.js --port $Port | Viewer: node client-web/serve.js 8080"
