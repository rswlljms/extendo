# driver/ — virtual display (IddCx)

v1 reuses the prebuilt **signed** fork, no custom driver code yet:

- Upstream: `https://github.com/VirtualDrivers/Virtual-Display-Driver` (IddCx 1.10, SDR/HDR, custom EDID, ARM64)
- Sample: `https://github.com/Microsoft/Windows-driver-samples/tree/main/video/IndirectDisplay`

## Install (manual, Phase 1)

1. Download the signed release matching your Windows build (10 vs 11 22H2+ HDR).
2. Run installer as admin, reboot if asked, trust the cert prompt.
3. Verify: Settings > System > Display shows the virtual monitor; `option.txt` controls default modes.

## IPC contract (host service ↔ driver helper, Phase 1)

JSON over named pipe `\\.\pipe\extendo-vdd`:

- `{"cmd":"add","width":1280,"height":720,"fps":60}` → `{"ok":true,"id":1}`
- `{"cmd":"remove","id":1}` → `{"ok":true}`
- Arrival/departure maps to `IddCxMonitorArrival` / `IddCxMonitorDeparture`.

## Rules (AGENTS.md §7.2)

- NEVER commit driver `.sys`/`.cat`/binaries to git. Link releases only.
- NEVER require Test Mode / disable driver signing.
- EDID/`option.txt` changes need review (resolution caps: 1080p60 SDR in v1).
