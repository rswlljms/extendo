# AGENTS.md — extendo

> Phone as true extended second monitor for Windows (Spacedesk-class clone).
> This file is the source of truth for all agents working in this repo.

## 1. Goal

Turn a mobile phone / tablet into a **true extended display** (not just mirror):
- Appears in Windows Settings > System > Display as a real monitor.
- Drag windows onto it, set position (left/right/above), resolution, scale.
- Works over **Wi-Fi LAN + USB** (USB tethering + ADB reverse). No custom USB bulk driver in v1.
- Target: office/dashboard/code use at 1080p60, 60–150ms glass-to-glass on 5GHz Wi-Fi / USB. Gaming-grade (<30ms) is explicitly out of scope for v1.

Non-goals for v1: iOS client, 4K/HDR/120Hz+, cloud relay / WAN, custom WinUSB driver, macOS/Linux host.
No billing / accounts / license server in v1 — but code must be licensing-ready (see §11).

## 2. Locked Scope (v1)

- **Host OS:** Windows 10 22H2+ / Windows 11 64-bit only.
- **Client OS:** Android first (API 28+), + Web fallback viewer. iOS deferred.
- **Transports (all TCP/IP, one socket abstraction):**
  1. `wifi` — same LAN via mDNS + QR + manual IP.
  2. `usb-tethering` — RNDIS / iPhone hotspot USB. Zero extra code, just detect new iface (e.g. `192.168.42.x`, `192.168.137.x`) and prefer lowest RTT.
  3. `adb-reverse` — bundled `adb.exe`, `adb reverse tcp:9577 tcp:9577`, client connects to `127.0.0.1:9577`. Requires USB debugging + RSA auth. Best latency / no router.
- **Display:** true extend via Indirect Display Driver (IddCx). Mirror-only mode kept as fallback / Phase 0 test.

## 3. Architecture

```
┌─ Windows Host ───────────────────────────────┐    ┌─ Android ──────┐
│ IddCx VDD (virtual monitor)                  │    │ Kotlin app     │
│   └─ IDDCX_SWAPCHAIN (DXGI surface)          │    │ MediaCodec     │
│ Host service (Rust): capture → encode → send │───▶│  H.264 decode  │
│   WGC / Desktop Duplication                  │ UDP/RTP video │ fullscreen render │
│   NVENC/QSV/AMF → MF/x264 fallback          │    │ touch/keyboard │
│ Transport picker: wifi | rndis | localhost   │◀───│  input events  │
│ mDNS + QR pairing + PIN + WebSocket control  │ TCP control   └────────────────┘
│ Tauri UI: display layout, quality, devices   │
└──────────────────────────────────────────────┘
```

Video: H.264 baseline/main, **no B-frames** (breaks WebRTC/browsers, adds latency). 720p/1080p, 30/60fps, 2–12 Mbps adaptive.
Control: Protobuf (preferred) or JSON over TLS WebSocket: `offer/answer, resolution, bitrate, input {touch,mouse,key}, heartbeat, stats {rtt,fps,drops}`.
Input injection on host via `SendInput()` (absolute + relative mouse modes).

## 4. Tech Stack (do not deviate without discussion)

| Layer | Choice | Why / notes |
|---|---|---|
| Virtual display | Fork `VirtualDrivers/Virtual-Display-Driver` (IddCx 1.10, signed) | Do NOT write IddCx from scratch. Reuse signed binary for v1. Control via IPC `AddMonitor(w,h,fps)` / `RemoveMonitor(id)`. Ref: `Microsoft/Windows-driver-samples/video/IndirectDisplay`. |
| Host core | Rust + tokio + `windows-capture` + `ffmpeg-sys` (NVENC/QSV/AMF, MF fallback) | Alt if C++ team: fork `LizardByte/Sunshine`. No Go, no pure-Electron core (CPU/latency). |
| Host UI | Tauri v2 + React | 10MB vs 200MB Electron. Calls Rust core directly. Deskreen uses Electron — we use Tauri. |
| Protocol | Sunshine/Moonlight-style UDP/RTP for native client, WebRTC/MJPEG only for browser fallback | Lowest latency. H.264 universal HW decode. |
| Android | Kotlin + Jetpack Compose + MediaCodec (+ Moonlight-android as reference/fork) | Native only. No Flutter/RN in video path (adds 30–60ms). |
| Discovery | mDNS + QR (`ip:port:token`) + manual IP + PIN | LAN-only v1, no cloud signaling. |
| USB helpers | Bundled `platform-tools/adb.exe`, WMI iface watcher | `adb reverse`, RNDIS detect. No WinUSB/libusb in v1. |
| Installer | WiX Toolset bundle (VDD + service + adb.exe + firewall rule) | Admin required. Handle reboot + cert trust. |
| Shared | `proto/` Protobuf schema, `client-web/` fallback viewer | Single contract for all clients. |

## 5. Repo Layout (create as needed)

```
extendo/
  AGENTS.md
  driver/            # forked VDD + IPC control service (C++)
  host/              # Rust core (capture/encode/net) + Tauri UI
    src-tauri/
    src/             # React UI
  client-android/    # Kotlin app
  client-web/        # fallback WebRTC/MJPEG viewer + QR join
  proto/             # .proto + generated bindings + PROTOCOL.md
  tools/             # adb bundle scripts, iface probe, diag collector
  docs/              # pairing, usb-setup, troubleshooting
```

Do not create new top-level dirs without updating this file.

## 6. Phased Plan

- **Phase 0 — Wi-Fi mirror MVP (1–2w):** capture main display via WGC, H.264 → Android/Web over LAN. Prove transport picker + RTT stats overlay. No driver.
- **Phase 1 — True extend + USB (3–5w):** integrate signed VDD IPC; capture only virtual monitor; sync resolution to phone EDID; transport abstraction `pickBest(wifi, rndis, localhost)`; USB-tethering detect via `0.0.0.0:9577` + WMI; ADB-reverse button with RSA-auth handling; reconnect on sleep/replug (`IddCxMonitorDeparture`).
- **Phase 2 — Input + adaptive (2–3w):** touch/keyboard backchannel → SendInput; quality slider + auto-drop to 720p if RTT>80ms or drops>5%; portrait/landscape rotate; PIN auth.
- **Phase 3 — Polish:** multi-phone (multi-monitor), diagnostics bundle, WiX installer, docs.

Each phase must demo: extend (drag window from PC to phone), rotate, unplug/replug, Wi-Fi vs USB switch with RTT shown.

## 7. Agent Rules

1. **Read before edit:** inspect `driver/`, `host/`, `proto/` before changing protocol or capture code. Never change `.proto` field numbers — only add new fields.
2. **Driver safety:** never commit unsigned driver binaries to git. Never require Test Mode. Prefer using prebuilt signed VDD. Driver changes need `option.txt`/EDID review.
3. **Latency budget:** capture <8ms, encode <12ms, network <40ms, decode+render <20ms. No B-frames, no software x264 `veryslow`, no 4K in v1. Cap v1 at 1080p60 SDR.
4. **Transport abstraction:** all video/input code must go through `Transport { kind, addr, rtt }`. No hardcoded IPs. Support `192.168.x`, `172.x`, `10.x`, and `127.0.0.1` (ADB).
5. **USB specifics:** check data cable (not charge-only); handle Samsung USB driver / Smart Connect conflicts in docs; handle ADB unauthorized/offline states with clear UI error; never silently disable firewall.
6. **Security (LAN v1):** PIN/token required, no open `0.0.0.0` without auth in release builds. Log IPs minimally. No cloud relay keys in repo.
7. **Android:** MediaCodec hardware path only; test on at least one physical device (emulator decode lies). Handle Doze/battery-optimization kill with foreground service.
8. **Verification:** after each change run the relevant check and paste output: `cargo test` / `cargo clippy` for host, `./gradlew test` for Android, `tsc --noEmit` for UI. For streaming changes, attach RTT/fps log (overlay or `tools/diag-collect.ps1`).
9. **Commits/PRs:** only when asked. Keep diffs small, one transport or one layer per PR. Update `proto/PROTOCOL.md` if wire format changes.

## 8. Common Commands (PowerShell 5.1)

```powershell
# repo sanity
Test-Path -LiteralPath "C:\Projects\extendo"
Get-ChildItem -Force "C:\Projects\extendo"

# host (Rust + Tauri) — run from host/
# cargo test; if ($?) { cargo clippy -- -D warnings }
# npm run tauri dev   # UI dev; npm run build for bundle

# android — run from client-android/
# .\gradlew test

# usb helpers
# .\tools\adb\adb.exe devices
# .\tools\adb\adb.exe reverse tcp:9577 tcp:9577
# Get-NetAdapter | Where-Object {$_.InterfaceDescription -match "RNDIS|Remote NDIS"}
```

If a command needs admin (driver install, firewall), stop and ask — do not self-elevate silently.

## 9. Troubleshooting Checklist (link from UI errors)

- Same LAN? 5GHz preferred. VPN / AP isolation blocks mDNS.
- USB: data cable? Tethering ON? `adb devices` shows `device` not `unauthorized`? Try USB-2 port, PTP/Image mode, different cable.
- Black screen: kill Smart Connect / Samsung USB driver conflict; restart phone app (force-stop + clear cache) after quality change; restart PC after resolution change.
- Lag: drop to 720p30, 4–6 Mbps; close router-heavy apps; prefer USB tethering over Wi-Fi.

## 10. Commercial / Paid-Ready (no billing in v1, but do not block it)

Verdict: pure SaaS (monthly just to use LAN extend) is NOT recommended — users hate it for offline utilities, Spacedesk/Duet backlash proves it. Recommended: **freemium perpetual + optional Pro/Team subscription**.

Suggested tiers (enforce via flags only, no server in v1):
- Free: 1 phone, 720p30, Wi-Fi only, mirror + 1 virtual display, community support.
- Pro ($29-49 one-time or $3-5/mo): 1080p60, USB (tethering + adb-reverse), multi-monitor, clipboard/touch, no watermark, priority encoder (NVENC/HEVC).
- Team/Enterprise (per-seat/yr): offline MSI, volume keys, MDM policy, SSO, audit log, SLA.

Agent rules for licensing-ready code:
1. Centralize all paywalled checks in one `Entitlement { tier, features }` module per platform (`host/src/license/`, `client-android/.../billing/`). UI calls `canUse("usb")`, `canUse("1080p60")`, never hardcodes limits. Default `tier=free`, `offline=true`.
2. No accounts/cloud required for LAN use. Future license server will be Stripe/LemonSqueezy + signed JWT (Ed25519) verified offline, 30-day grace. Leave `license.key`, `license.tier`, `device_id` fields in config now; validate as `trial/free` stub.
3. No DB in v1 (see §7 rule 6). Future server only: Postgres `users, licenses, devices, invoices` + Stripe webhooks. Never put billing keys, API secrets, or PII in repo.
4. Feature-flag paywalled encoders/resolutions in config, not in scattered `if`s, so flipping a flag unlocks Pro without refactor.
5. Telemetry/analytics hooks must be opt-in, stubbed OFF in v1. No tracking in video path.
6. Keep free core fully usable offline forever — Pro only gates the list above. Never gate basic extend behind network call.

## 11. References

- IddCx overview + sample: `learn.microsoft.com/.../indirect-display-driver-model-overview`, `github.com/Microsoft/Windows-driver-samples/tree/main/video/IndirectDisplay`
- Signed fork: `github.com/VirtualDrivers/Virtual-Display-Driver`
- Deskreen (Electron+WebRTC mirror ref): `github.com/pavlobu/deskreen`, `deskreen.com`
- Sunshine/Moonlight (low-latency UDP ref): `github.com/LizardByte/Sunshine`
- Spacedesk USB modes: Driver Console > Communication Interfaces > USB Cable Android; USB-tethering docs; `adb reverse tcp:28252 tcp:28252` community pattern.
