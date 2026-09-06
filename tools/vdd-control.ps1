#Requires -Version 5.1
<#
.SYNOPSIS
  Control the extendo virtual display via \\.\pipe\extendo-vdd (Phase 1).
.USAGE
  powershell -ExecutionPolicy Bypass -File .\tools\vdd-control.ps1 list
  powershell -ExecutionPolicy Bypass -File .\tools\vdd-control.ps1 add -Width 1280 -Height 720 -Fps 60 -Edid phone-720p
  powershell -ExecutionPolicy Bypass -File .\tools\vdd-control.ps1 remove -Id 1
Requires the VDD helper running (driver/README.md). Needs no admin for pipe talk.
#>
param(
  [Parameter(Position = 0)][ValidateSet("list", "add", "remove")][string]$Cmd = "list",
  [int]$Width = 1280, [int]$Height = 720, [int]$Fps = 60,
  [string]$Edid = "phone-720p", [int]$Id = 0
)

$body = switch ($Cmd) {
  "list" { @{ cmd = "list" } }
  "add" { @{ cmd = "add"; width = $Width; height = $Height; fps = $Fps; edid = $Edid } }
  "remove" { @{ cmd = "remove"; id = $Id } }
}

$pipe = New-Object IO.Pipes.NamedPipeClientStream(".", "extendo-vdd", [IO.Pipes.PipeDirection]::InOut)
try { $pipe.Connect(2000) }
catch { Write-Error "VDD helper not reachable on \\.\pipe\extendo-vdd. Install the signed VDD fork first (driver/README.md)."; exit 1 }

$writer = New-Object IO.StreamWriter($pipe)
$reader = New-Object IO.StreamReader($pipe)
$writer.WriteLine(($body | ConvertTo-Json -Compress))
$writer.Flush()
Write-Output ($reader.ReadLine())
