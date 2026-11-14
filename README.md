# Horizon Web for Chromebook

An experimental Minecraft Java Edition 26.3 client in the browser, built from
[Horizon Web](https://github.com/nobuildersnotools/horizon-web) and
[Horizon](https://github.com/horizon-rust/horizon). Rust → WebAssembly → WebGPU.

This fork focuses on a simple Chromebook setup and an installable **Isolated Web
App (IWA)**. There are no default promoted multiplayer servers.

## Start here

| I want to… | Go here |
| --- | --- |
| Build and install the Chromebook app | [Chromebook guide](web/iwa/README.md) |
| Build a normal browser version | [Web build guide](web/README.md#build-and-run) |
| Understand the browser port's limits | [Known web limitations](web/README.md#whats-missing) |
| Work on the engine | [Architecture](docs/architecture.md) |

**Status:** IWA packaging and compatibility changes are experimental. Packaging
checks use fixtures; a complete game launch inside ChromeOS still needs testing.
No playable download is included. Building needs resources prepared from your
own Minecraft installation; do not commit or publicly upload those resources.

The browser port supports single player, local saves, audio, controllers and
compatible WebSocket multiplayer. It requires WebGPU and shared-memory support.
Java Fabric/NeoForge mods do not run in this Rust client. Packaging as an IWA does
not add Java mod support, Microsoft sign-in or direct TCP networking.

## Build the game

On a Linux build machine (or Chromebook Linux environment), install Rust,
Python 3.10+, JDK 25 and the dependencies in the [web guide](web/README.md).
Prepare resources from your own Minecraft Java Edition 26.3 installation:

```sh
cargo run --release -p horizon-prepare -- --prepare --server-jar /path/to/server.jar
cargo run --release -p horizon-prepare -- sounds
web/build.sh
```

The preparer looks in `~/.minecraft`; use `--minecraft-dir` or
`HORIZON_MINECRAFT_DIR` for another installation. The result is `web/dist/`.
Continue with the [IWA packaging steps](web/iwa/README.md#package-the-app).

## Repository map

- `html/` — the game page, loader, audio and worker bootstrap.
- `web/` — browser build scripts, resource packaging and development server.
- `web/iwa/` — Chromebook packaging, tests and installation guide.
- `crates/` — Rust game engine and browser platform support.
- `docs/`, `tests/`, `tools/` — engine documentation and developer checks.

The engine layout is retained so upstream fixes remain easier to merge.
GitHub Actions → **Chromebook web build** checks packaging and can compile a
resource-free Wasm artifact on manual runs. It does not publish Minecraft assets
or create a playable signed app without your local resources.

## Credits and licence

Original Horizon by Ludovic Marie and contributors; browser port by
[@bedrock3359](https://github.com/bedrock3359). Fork maintained by
[@bilaljanu](https://github.com/bilaljanu).
[MIT licence](LICENSE) · [Third-party notices](THIRD_PARTY_NOTICES.md).

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
