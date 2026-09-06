# Pairing (Wi-Fi LAN)

1. Start the host: `node host/src/index.js` — note the PIN + port (default 9577).
2. Same network: phone and PC on the same LAN (5GHz preferred). VPN / guest-AP isolation blocks discovery.
3. On the phone/web viewer open `GET /info` (e.g. `http://192.168.1.10:9577/info`) to list transports + QR payloads.
4. `POST /pair` with the token or PIN: `{ "device_name": "pixel", "token": "<token|PIN>" }`.
5. Open the stream. Overlay shows `rtt · fps · drops`. If RTT > 80ms or drops > 5%, drop to 720p30.

See `proto/PROTOCOL.md` for endpoint shapes.
