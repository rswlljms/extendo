# client-android/ — native Kotlin viewer

Stack: Kotlin + Jetpack Compose + `HttpURLConnection`/`MediaCodec`.
Reference: `Moonlight-android` (Sunshine-protocol client). No Flutter/RN in the video path.

Milestone 1 (done, needs only a device — no driver, no H.264 yet):
`POST /pair` + render `/video.mjpg` on canvas with RTT/fps overlay,
transport switcher (Wi-Fi ↔ USB, no re-pairing), quality presets, rotate,
touch/keyboard backchannel, foreground service (Doze-proof), automation
extras (`--es host --es token --ez autoconnect`, also used by QR-join later).

Milestone 2 (next): swap MJPEG for `MediaCodec` H.264 over `/video.h264`
(Annex B, keyframe-primed, SPS/PPS on every IDR — contract in
`proto/PROTOCOL.md`), Android→Windows-VK key mapping, per-monitor binding.

## Build

Needs a JDK (Android Studio's bundled runtime works) and the SDK:

```powershell
$env:JAVA_HOME = "C:\Program Files\Android\Android Studio\jbr"
# one-time: point at your SDK (untracked local.properties, never commit)
" sdk.dir=C\:\\Users\\YOU\\AppData\\Local\\Android\\Sdk" | Set-Content local.properties
.\gradlew.bat :app:testDebugUnitTest   # 16 JVM tests (models, MJPEG framing, API parsing)
.\gradlew.bat :app:assembleDebug       # app/build/outputs/apk/debug/app-debug.apk
```

`minSdk 28`, `compileSdk/targetSdk 35`. Emulator decode lies — verify video
on a physical device (AGENTS.md §7.7).

## Manual test (5 min, physical device)

```powershell
# 1. host MJPEG stack (any PC display; no driver needed)
node host/src/index.js                                   # :9577, note the token
Invoke-RestMethod -Method Post http://127.0.0.1:9577/capture `
  -Headers @{"x-extendo-token"="<tok>"} -ContentType "application/json" `
  -Body '{"backend":"wgc","monitor_id":-1}'              # needs: cargo build --release -p extendo-core
# 2. install + connect (same LAN, or USB tethering / adb reverse)
adb install -r app/build/outputs/apk/debug/app-debug.apk
# enter http://<pc-ip>:9577 + token, Connect — or headless:
adb shell am start -n com.extendo.viewer/.MainActivity `
  --es host http://<pc-ip>:9577 --es token <tok> --ez autoconnect true
```

Expect: live desktop in <2s, overlay `rtt …ms · fps … · drops … · WxH mjpeg`,
transport buttons showing probed RTT, 720p30 instant / 1080p60 gated on free
tier, touch drag moves the PC cursor (Pro), rotate swaps the overlay dims.
Kill via the "leave" button (stops the foreground service).

## Models (mirror `proto/extendo.proto` JSON names)

- `Transport(kind, addr, port, rttMs)` — `kind ∈ {WIFI, RNDIS, LOCALHOST}`, `127.0.0.1` always legal.
- `VideoConfig(width, height, fps, bitrateKbps, codec="h264")` — no B-frames.
- `InputEvent(type, x, y, key_code, down)` → host `SendInput()`.
- `Stats(rttMs, fps, drops, bitrateKbps)` overlay.
