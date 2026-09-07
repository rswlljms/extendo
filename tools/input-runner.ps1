#Requires -Version 5.1
<#
.SYNOPSIS
  Persistent input runner for extendo (Phase 2). Reads NDJSON actions from
  STDIN and injects them via user32 (mouse_event / keybd_event).
  Spawned once by host/src/input.js - never run by hand (no auth here;
  the host HTTP layer gates callers by token first).
Protocol (one JSON object per line):
  {"a":"move","x":0..65535,"y":0..65535}  absolute mouse move
  {"a":"rel","dx":n,"dy":n}               relative mouse move
  {"a":"left","down":true|false}          left button
  {"a":"wheel","d":120|-120}              vertical scroll (WHEEL_DELTA units)
  {"a":"key","vk":65,"down":true|false}   virtual-key up/down
#>
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Inj {
  [DllImport("user32.dll")] public static extern void mouse_event(int f,int dx,int dy,int d,int e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte v,byte s,int f,int e);
}
"@

$MOVE = 0x8000; $LDOWN = 0x0002; $LUP = 0x0004; $WHEEL = 0x0800; $REL = 0x0001; $KUP = 0x0002

while (($line = [Console]::In.ReadLine()) -ne $null) {
  if (-not $line.Trim()) { continue }
  try { $m = $line | ConvertFrom-Json } catch { continue }
  switch ($m.a) {
    "move"  { [Inj]::mouse_event($MOVE -bor 0x0001, [int]$m.x, [int]$m.y, 0, 0) }
    "rel"   { [Inj]::mouse_event($REL, [int]$m.dx, [int]$m.dy, 0, 0) }
    "left"  { if ($m.down) { [Inj]::mouse_event($LDOWN, 0, 0, 0, 0) } else { [Inj]::mouse_event($LUP, 0, 0, 0, 0) } }
    "wheel" { [Inj]::mouse_event($WHEEL, 0, 0, [int]$m.d, 0) }
    "key"   { if ($m.down) { [Inj]::keybd_event([byte]$m.vk, 0, 0, 0) } else { [Inj]::keybd_event([byte]$m.vk, 0, $KUP, 0) } }
  }
}
