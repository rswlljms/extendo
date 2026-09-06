# extendo — phone as true extended second monitor (v1 scaffold)

Windows host + Android client over Wi-Fi LAN + USB (tethering + ADB reverse).
Full agent instructions: [`AGENTS.md`](AGENTS.md). Wire contract: [`proto/PROTOCOL.md`](proto/PROTOCOL.md).

## Quick start (Phase 0, no driver yet)

```powershell
# 1. host — signaling + transport picker + test-pattern stream
node host/src/index.js
# 2. probe interfaces (finds rndis tethering IP + localhost ADB target)
powershell -ExecutionPolicy Bypass -File tools/iface-probe.ps1
# 3. USB ADB mode (optional)
powershell -ExecutionPolicy Bypass -File tools/adb-reverse.ps1
# 4. web viewer
node client-web/serve.js 8080
# open http://127.0.0.1:8080, enter host http://<pc-ip>:9577 + token/PIN
```

Overlay shows `rtt · fps · drops`. Phase 0 streams a test pattern; Phase 1
plugs real IddCx capture + H.264 into the same `Transport` + `VideoConfig`.

## Extend demo (Phase 1, no driver needed — stub registry)

```powershell
# extend a 720p virtual monitor (idempotent: reuses a matching one)
Invoke-RestMethod -Method Post http://127.0.0.1:9577/displays `
  -ContentType "application/json" -Body '{"mode":{"width":1280,"height":720,"fps":30},"edid":"phone-720p"}'
# bind capture to it, then open the viewer — overlay shows the synced mode
Invoke-RestMethod -Method Post http://127.0.0.1:9577/capture `
  -ContentType "application/json" -Body '{"backend":"test","monitor_id":1}'
# depart when done
Invoke-RestMethod -Method Delete http://127.0.0.1:9577/displays/1
```

## Layout

- `proto/` — `extendo.proto` + `PROTOCOL.md` (add-only field numbers)
- `host/` — runnable Node core (transport/config/license/server) + `tests/`
- `client-web/` — fallback viewer (connect/pair/frames/quality/rotate)
- `client-android/` — Kotlin models + Phase-2 MediaCodec plan
- `driver/` — signed VDD fork notes + IPC contract (no binaries in git)
- `tools/` — `iface-probe.ps1`, `adb-reverse.ps1`, `diag-collect.ps1`
- `docs/` — pairing, usb-setup, troubleshooting

## Verify

```powershell
node --test host/tests/transport.test.js host/tests/license.test.js host/tests/server.test.js
npx -y tsc --noEmit -p client-web/tsconfig.json
```
