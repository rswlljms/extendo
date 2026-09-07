# PROTOCOL.md — extendo wire contract (v1)

Source of truth: `extendo.proto`. JSON names below match proto field names
so the Web fallback viewer can speak the same contract without protobuf.

## Transports

All transports are plain TCP/IP. Video/input code must use
`Transport { kind, addr, rtt }` — never a hardcoded IP.

| kind | addr example | how |
|---|---|---|
| `wifi` | `192.168.1.10:9577` | mDNS + QR (`extendo://ip:port?token=…`) + manual IP |
| `rndis` | `192.168.42.129:9577` | USB tethering (RNDIS / iPhone hotspot). Same sockets, different iface. |
| `localhost` | `127.0.0.1:9577` | `adb reverse tcp:9577 tcp:9577`, client connects to loopback |

`pickBest()`: probe RTT to each candidate, pick lowest. Priority on tie:
`localhost` > `rndis` > `wifi`.

## Host HTTP endpoints (Phase 2; LAN, PIN-gated)

Open (discovery + measurement): `GET /info`, `GET /ping`, `GET /stats`, `GET /best`.
Paired only — send `x-extendo-token: <token|PIN>`, `?token=`, or body `token`:

- `POST /pair` body `PairRequest` → `PairResponse` (5 fails/min/IP → HTTP 429, 30s lockout)
- `GET /frames?token=` (SSE test pattern; Phase 1 H.264 replaces payload)
- `POST /input` body `InputEvent` → injected via SendInput bridge (`tools/input-runner.ps1`); `touch`/`key` require Pro (402 on free), `mouse_*`/`scroll` free
- `POST /report` body `{fps, drops}` → `{ok, video, adaptive}` — viewer heartbeat feeding the adaptive loop
- `POST /quality` (`/displays`, `/capture` likewise gated — see §Display control)

## Input mapping (normalized → absolute 0..65535, primary display)

`touch down` = move + left-down; `touch` move = move; `touch up` = left-up.
`mouse_move` = absolute move (or relative with `dx/dy`); `scroll dy` × −12 wheel units.
`key` = `keybd_event(VK)` down/up via `key_code`. Runner is one persistent
PowerShell process fed NDJSON on STDIN — never one spawn per event.

## Adaptive (closed loop, host-side, cooldown 10s)

Viewer reports observed `fps/drops` every 5s (`/report`); host tick re-probes
RTT (`/best`) and runs: drop to 720p30 when RTT>80ms or drop-rate>5%;
restore previous mode after 4 consecutive good checks (RTT<40ms, 0 drops).
`POST /quality {"auto":false}` opts out (manual slider wins).

## Shapes (JSON names = proto field names)

- `GET /info` → `{ name, version, transports[], video, qr, tier, displays, displays_stub, capture }`
- `GET /ping?t0=<client_ms>` → `{ t0, server_ts_ms }` (RTT = now - t0)
- `GET /stats` → `Stats` + `uptime_s`
- `GET /best` → `{ best, probed }` (RTT-probed transports)
- `GET /frames` (SSE) → `data: {"seq":N,"server_ts_ms":T,"monitor_id":M,"x":0..1,"y":0..1}` at video.fps.
  `?monitor=<id>` binds one viewer to one virtual monitor (multi-phone);
  unknown id → 400. Phase-shifted per monitor so multi-viewer routing is visible.

## Display control (Phase 1 — true extend)

Virtual monitor lifecycle over named pipe `\\.\pipe\extendo-vdd`
(JSON lines, mirrored on HTTP so any client can drive it):

- `{"cmd":"add","width":1280,"height":720,"fps":60,"edid":"phone-1080p"}` → `{"ok":true,"monitor":{"id":1,...}}`
- `{"cmd":"remove","id":1}` → `{"ok":true}` (maps to `IddCxMonitorDeparture`)
- `{"cmd":"list"}` → `{"ok":true,"monitors":[...]}`

HTTP mirror: `GET /displays`, `POST /displays` (DisplayAdd), `DELETE /displays/:id`,
`POST /capture` (CaptureSource select). Capture binds to the virtual monitor
only; `monitor_id:-1` is the mirror fallback. Resolution changes re-negotiate
`VideoConfig` with the viewer (see `/quality`).

Reconnect: host watches interfaces + power resume; on new iface or resume it
re-probes (`/best`), re-adds the monitor if departed, and viewers re-pair
with the same token. No re-pairing needed on USB replug.

## Video

- Codec `h264`, baseline/main, **no B-frames**, 720p/1080p, 30/60fps, 2–12 Mbps.
- Phase 0 serves the SSE test pattern so transport/RTT/overlay can be
  verified without a capture driver. Real capture plugs into the same
  `Transport` + `VideoConfig` in Phase 1 (Rust core).

## Rules

- Never reuse proto field numbers. Add-only.
- `127.0.0.1` is always a legal `addr` (ADB).
- Private ranges `192.168.x`, `172.16-31.x`, `10.x` are all legal (no `192.168.1.x` assumption).
