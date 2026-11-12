# Chromebook app

Build the game on Linux, then install the signed `.swbn` in Chrome on your
Chromebook. You need a WebGPU-capable Chromebook and permission to enable IWA
development mode. A school-managed device may restrict this; its administrator
controls installation. This is Chrome's **IWA development flag**, not ChromeOS
Developer Mode or a requirement to disable OS verification.

## Package the app

First follow the [root build steps](../../README.md#build-the-game). Install
Node.js 22.13 or newer, then run these commands from the repository root:

```sh
npm ci --prefix web/iwa
python3 web/package.py --no-build
mkdir -p "$HOME/.config/horizon-iwa"
# Run key generation ONCE. Reuse this key for later app updates.
(umask 077; openssl genpkey -algorithm Ed25519 -out "$HOME/.config/horizon-iwa/signing.pem")
node web/iwa/package.mjs web/out/horizon-web "$HOME/.config/horizon-iwa/signing.pem" 1.0.0
```

Output: **`web/out/horizon.swbn`**. Keep the signing key private and backed up;
a different key creates a different app with separate saves. Never commit the
key, `web/dist/`, `web/out/` or prepared Minecraft resources. The resulting game
bundle contains your prepared resources: keep it for your own installation.
For an encrypted PEM, set `WEB_BUNDLE_SIGNING_PASSPHRASE` when packaging.

The packager includes all lazy resource packs, removes cache-busting queries,
adds the manifest and icon, sets isolation/security headers and signs the bundle.
It rejects missing manifest resources and symlinks. Use `web/package.py` first
to materialize the build's resource symlink into a complete folder.

## Install in Chrome

1. Open `chrome://flags/#enable-isolated-web-app-dev-mode`, enable it and restart Chrome.
2. Open `chrome://web-app-internals`.
3. Under **Install IWA from Signed Web Bundle**, select `horizon.swbn`.
4. Launch Horizon from the Chromebook launcher.

If the flag or install option is unavailable, check Chrome/device policy support.
See [Chrome's IWA documentation](https://developer.chrome.com/docs/iwa/introduction).

For an update, rebuild and package with the **same key** and a higher version,
for example `1.0.1`, then use the IWA development update controls. Automatic
hosted updates are not configured. Do not uninstall to update if you want to
keep saves. Browser-site saves do not automatically transfer to the IWA's origin.

## Multiplayer and saves

Add your own compatible server in Multiplayer using its `wss://` address.
Plain `ws://` is blocked by IWA policy. An ordinary Java TCP server still needs
a compatible WebSocket bridge. Microsoft online-mode authentication remains an
upstream web limitation. Shared-world relay endpoints are retained.

The former promoted server is no longer added or pinned. An entry already saved
by the old build remains an ordinary editable/deletable entry, so this update
does not silently delete saved server data.

Worlds use the app's browser storage. The upstream web port does not yet provide
world ZIP import/export, and abrupt closing can lose pending writes.

## Development and validation

```sh
npm test --prefix web/iwa
```

GitHub Actions runs these packaging tests. Manually dispatch **Chromebook web
build** with `compile_wasm` enabled to compile the engine without game resources;
the artifact is not a playable app. Local packaging needs your prepared resources.

Before calling a build ready, test on the target Chromebook:

- Title screen opens without CSP or worker errors in DevTools.
- Create a world, move, place/break blocks and confirm audio/pointer lock.
- Save and quit, close the app, reopen it and confirm the world persists.
- Join your WSS server and check shared-world connections.
- Start single player with Wi-Fi off; confirm lazy resources still load.

Current validation covers bundle structure, signing and worker URL restrictions
using synthetic fixtures. Full ChromeOS gameplay and native title-screen checks
require the prepared game resources and have not been completed for this change.
