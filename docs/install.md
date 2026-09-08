# Install extendo

Three ways, simplest first. All LAN-only; no account needed.

## A. Portable ZIP (test today, no admin)

1. Build it: `powershell -ExecutionPolicy Bypass -File tools/make-portable.ps1`
2. Unzip `dist/extendo-portable-<ver>.zip` anywhere (even a USB stick).
3. `node host/src/index.js` (needs Node 18+), note the PIN.
4. `node client-web/serve.js 8080`, open the viewer, pair with the PIN.
5. Optional, same LAN only: allow inbound TCP 9577 when Windows asks,
   or run `tools/install-dev.ps1 -Firewall` once as admin.

## B. Dev setup (from source)

`powershell -ExecutionPolicy Bypass -File tools/install-dev.ps1 [-Firewall] [-Startup] [-Vdd]`

## C. MSI installer (maintainers, Phase 3)

`installer/build.ps1` needs the .NET SDK (`dotnet tool restore` pulls WiX 4).
It checks version sync across `host/package.json`, `client-web/package.json`,
`Package.wxs`, and `Bundle.wxs`, compiles the viewer, and emits
`installer/out/extendo-<ver>.msi` plus the `extendo-setup-<ver>.exe` bundle
(which chains Node LTS + signed VDD downloads at install time).
Sign the MSI with your EV cert before release (`signtool`), and fill the
`Bundle.wxs` download URLs (Node LTS, signed VDD) with verified SHA256 at
release time. UpgradeCode is frozen — never change it.

## True extended monitor (all methods)

The ZIP/dev/MSI all drive the stub registry until the signed VDD is present:
`tools/install-dev.ps1 -Vdd` opens the release page. Install as admin, reboot
if asked, then `POST /displays` lights up a real panel in Display Settings.

## Uninstall

- Portable: delete the folder. Config lives in `%APPDATA%\extendo` (delete too if wanted).
- MSI: Apps > Installed apps > extendo > Uninstall.
- Logon task (if `-Startup` was used): `schtasks /delete /tn "extendo host" /f`.
- Firewall rule: `Remove-NetFirewallRule -DisplayName "extendo host"`.
