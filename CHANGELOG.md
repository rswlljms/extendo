# Changelog — extendo (curated, user-facing; newest on top)

## [Unreleased]

## [0.3.0] - 2026-09-06
### Added
- Input backchannel: viewer touch/keyboard → host SendInput bridge
  (persistent `tools/input-runner.ps1`, `touch`/`key` Pro-gated per tiers)
- Token auth on all mutation/video endpoints (`x-extendo-token`/query/body);
  pair brute-force throttle (5 fails → HTTP 429, 30s lockout)
- Adaptive quality loop: viewer `/report` + host 5s tick, drop to 720p30 on
  RTT>80ms or drops>5%, restore on recovery, `{"auto":false}` opt-out
### Changed
- `InputEvent`/`Heartbeat` consumers: `/input` now returns action counts;
  unauthenticated `/frames` returns 401 (pass `?token=`)

## [0.2.0] - 2026-09-06
### Added
- True-extend display control: `POST /displays` creates-or-reuses a virtual
  monitor (`IddCxMonitorArrival` via named pipe), `DELETE /displays/:id`
  departs it; free tier capped at 720p30, Pro unlocks 1080p60/multi-monitor
- Capture source binding (`GET/POST /capture`) — virtual-monitor-only,
  mirror fallback via `monitor_id:-1`
- Interface watcher: host re-probes best transport on USB replug/Wi-Fi change
- Web viewer: transport switcher (Wi-Fi ↔ USB without re-pairing) with RTT,
  auto-reconnect stream on drop
- `tools/vdd-control.ps1` for pipe control; `Display*/Monitor` proto messages

## [0.1.0] - 2026-09-06
### Added
- Phase 0 scaffold: host transport picker (wifi/rndis/localhost), PIN/token
  pairing, `/frames` test-pattern stream, web fallback viewer with RTT overlay
- `tools/iface-probe.ps1`, `tools/adb-reverse.ps1`, `tools/diag-collect.ps1`
- Signed-VDD reuse plan, USB tethering + ADB-reverse docs
