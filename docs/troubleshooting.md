# Troubleshooting

Full checklist lives in `AGENTS.md §9`. Short version:

- **Not found?** Same LAN? 5GHz? VPN off? Try manual IP from `tools/iface-probe.ps1`.
- **USB?** Data cable? Tethering ON? `adb devices` = `device` (not `unauthorized`/`offline`)?
- **Black screen?** Smart Connect / Samsung driver conflict; force-stop phone app + clear cache; reboot PC after resolution change.
- **Lag?** 720p30 @ 4–6 Mbps; prefer USB tethering; close router-heavy apps.
- **Report?** Run `tools/diag-collect.ps1`, attach `host-stats.json` + `adapters.txt` + overlay `rtt/fps/drops`.
