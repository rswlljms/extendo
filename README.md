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
