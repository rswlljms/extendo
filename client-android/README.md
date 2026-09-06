# client-android/ — native Kotlin viewer (Phase 2)

Stack: Kotlin + Jetpack Compose + `MediaCodec` hardware H.264 decode.
Reference: `Moonlight-android` (Sunshine-protocol client). No Flutter/RN in the video path.

## Models (mirror `proto/extendo.proto` JSON names)

- `Transport(kind, addr, port, rttMs)` — `kind ∈ {wifi, rndis, localhost}`, `127.0.0.1` always legal.
- `VideoConfig(width, height, fps, bitrate_kbps, codec="h264")` — no B-frames.
- `InputEvent(type, x, y, key_code, down, dx, dy)` → host `SendInput()`.
- `Stats(rtt_ms, fps, drops, bitrate_kbps)` overlay.

## Milestones

1. `POST /pair` + render `/frames` test pattern on canvas (parity with `client-web/`).
2. Swap test pattern for `MediaCodec` H.264 over UDP/RTP (Phase 1 host).
3. Touch/keyboard backchannel, rotate, quality selector, foreground service (Doze-proof).

Verification (needs Android SDK machine): `./gradlew test` — emulator decode lies, test on a physical device.
