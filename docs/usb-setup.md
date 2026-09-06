# USB setup (Android)

Two modes, both plain TCP/IP (no custom USB driver in v1):

## A. USB tethering (easiest, no ADB)

1. Connect phone via a **data cable** (charge-only cables fail silently).
2. Android: Settings → Network → Hotspot & tethering → **USB tethering ON**.
3. On PC run `tools/iface-probe.ps1` — a new `rndis` iface appears (`192.168.42.x` / `192.168.137.x`).
4. In the app connect to that IP. Same sockets as Wi-Fi, lower jitter.

## B. ADB reverse (lowest latency, no router)

1. Enable Developer options → **USB debugging ON**. Use a USB-2 port + data cable first.
2. `powershell -ExecutionPolicy Bypass -File tools/adb-reverse.ps1 -Port 9577`
3. Accept the **RSA fingerprint prompt on the phone** (`unauthorized` → unplug/replug, revoke authorizations if stale).
4. In the app connect to `127.0.0.1:9577`.

## Conflicts (from Spacedesk field reports)

- Uninstall/disable **Smart Connect** and **Samsung USB Driver for Mobile Phones** if USB video stays black.
- Try PTP/Image USB mode instead of File transfer; try another cable/port.
- After changing quality/resolution: force-stop the phone app (clear cache) + restart the host.
