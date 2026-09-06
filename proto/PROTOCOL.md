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

## Host HTTP endpoints (Phase 0, no TLS on LAN yet — PIN required)

- `GET /info` → `{ name, version, transports[], video, qr }`
- `GET /ping?t0=<client_ms>` → `{ t0, server_ts_ms }` (RTT = now - t0)
- `GET /stats` → `Stats` JSON `{ rtt_ms, fps, drops, bitrate_kbps }`
- `GET /frames` (SSE) → `data: {"seq":N,"server_ts_ms":T,"x":0..1,"y":0..1}` at video.fps.
  Phase 0 test pattern (moving box). Phase 1 replaces payload with H.264
  over UDP/RTP; field names stay stable.
- `POST /pair` body `PairRequest` → `PairResponse`
- `POST /input` body `InputEvent` → `{ ok:true }` (→ SendInput in Phase 2)

## Video

- Codec `h264`, baseline/main, **no B-frames**, 720p/1080p, 30/60fps, 2–12 Mbps.
- Phase 0 serves the SSE test pattern so transport/RTT/overlay can be
  verified without a capture driver. Real capture plugs into the same
  `Transport` + `VideoConfig` in Phase 1 (Rust core).

## Rules

- Never reuse proto field numbers. Add-only.
- `127.0.0.1` is always a legal `addr` (ADB).
- Private ranges `192.168.x`, `172.16-31.x`, `10.x` are all legal (no `192.168.1.x` assumption).
