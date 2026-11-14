# Horizon APIs

The browser port keeps the engine's public surfaces for content and shader
authors. Each one has an **API** (the contract) and the rules every surface
follows: [stability and versioning](stability.md).

| Surface | For | API today | Stability | Reference |
| --- | --- | --- | --- | --- |
| [Shaders](shaders.md) | shader authors | API 0.1 (sky, vertex, material, weather and post stages, coloured block light, options, hot reload) | experimental | [shaders-reference.md](shaders-reference.md) |
| [Content](content.md) | launchers and pack tools | where each kind of content goes, turning it on and off, the checks before installing; `content` in `horizon-client --launch-api` | experimental | [data formats](data-formats.md) |
| [Data formats](data-formats.md) | tools that read Horizon's files | settings files and prepared-resource formats | stable or preview, per format | on that page |

## How these APIs are designed

- **From the people who use them.** Each API starts from what a mod author, a
  pack author or a tool needs to do, written down as tasks.
- **Our own design.** Horizon's APIs are not modelled on any other project's.
  Vanilla Minecraft 26.3 is the reference for game concepts: ids, commands,
  data packs and resource packs.
- **Safe by default.** A content mod is data and runs no code; nothing here is
  on by default where it would change vanilla's look or behaviour.
- **Free when unused.** An API nobody uses costs nothing per tick or per frame.

## Each surface's page

Every page has the same parts: **Overview** (what you can build), **Guide**
(where to start), **Reference**, **Examples**, **Stability** (the level of each
part, per [stability.md](stability.md)), **Changelog** and **Not there yet**
(what is planned, so nobody builds on a guess).
