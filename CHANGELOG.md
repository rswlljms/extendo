# Changelog — extendo (curated, user-facing; newest on top)

## [Unreleased]

## [0.5.0] - 2026-09-08
### Added
- Rust capture core `host/core` (WGC → JPEG → MJPEG, AGENTS.md section 4 browser-fallback path):
  pure-Rust SIMD JPEG (no ffmpeg), 720p/1080p ≤60fps, downscale+RGB in one pass (4–7ms encode on test box),
  loopback `host_port+1` with `GET /video.mjpg` (multipart), `/frame.jpg`, `/health`
- Node host spawns/proxies the core: `POST /capture {"backend":"wgc"}` starts it,
  `GET /video.mjpg`/`/frame.jpg` proxy through the host port+token, `/core/health`
  and `GET /info core/capture` expose it; `POST /quality` re-spawns on cap change
- Web viewer prefers MJPEG (real screen) when `GET /info core.ok`, falls back to SSE test pattern; `<img id="mjpeg">` with auto-fallback on error
- 33-test Rust suite (`cargo test`) covering `fit_within`, stride-aware downscale, JPEG SOI/EOI, MJPEG framing, CLI validation and token checks; `cargo clippy -- -D warnings` clean
### Fixed
- Installer: start-menu shortcut finds Node via registry search and points at
  the flat install layout (`[INSTALLDIR]\index.js`); docs shortcut ships
  `README.md`; bundle consumes the versioned MSI via a build-time path, and
  `build.ps1` now emits the bundle exe plus a four-file version-sync check
- Build output ignored: `installer/out/` and `*.msi` are git-ignored (no
  binaries in git); fixed `tsc` invocations to `npx -y -p typescript tsc`
  (bare `npx -y tsc` resolves the wrong package)

## [0.4.0] - 2026-09-06
### Added
- Multi-phone: `GET /frames?monitor=<id>` binds a viewer to one virtual
  monitor (phase-tagged frames, viewer monitor switcher); second simultaneous
  monitor requires Pro (arrival rolled back on free tier)
- Installer: WiX v4 `installer/` project (frozen UpgradeCode, firewall rule,
  shortcuts, version-sync check) + `Bundle.wxs` chaining Node/VDD downloads
- Portable ZIP: `tools/make-portable.ps1` (compiles viewer, stages, zips)
- One-command dev setup: `tools/install-dev.ps1` (Node check, PIN/token,
  `-Firewall`/`-Startup`/`-Vdd`); `docs/install.md` covers ZIP/dev/MSI
### Fixed
- Diagnostics bundle redacts token/PIN from `/info` and config snapshots
- PowerShell scripts are ASCII-only (PS 5.1 misparses UTF-8 without BOM)

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
