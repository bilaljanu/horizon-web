# Horizon Web documentation

These pages are for developers working on the browser port and the engine it
builds on. For building and installing the web app, start with the
[root README](../README.md) and the [web guide](../web/README.md).

## Build and run

- [Architecture](architecture.md): where each part of the code lives and how
  `Game` is organised.
- [Resource preparation](prepare-performance.md): preparing assets from a local
  Minecraft install for the web build.
- [Build times](build-times.md): measuring cold and incremental builds.
- [Profiling](profiling.md): where the engine spends its time.
- [Controllers](controllers.md): controller input and settings.

## Rendering and content

- [Graphics](graphics.md): graphics settings and rendering behaviour.
- [Shaders](shaders.md) and the [shader-pack clean room](cleanroom/README.md).
- [Resource packs](resource-packs.md) and [reusing pack overlays](reuse-pack-overlays.md).
- [Cosmetics](cosmetics.md), [Extras](extras.md) and [PvP & HUD](pvp.md).
- [Narrator](narrator.md).
- [Design system](design-system.md) and the [HUD kit](ui-kit.md).

## World generation

- [World generation](world-generation.md), [world data](world-data.md),
  [world height](world-height.md) and [heightmaps](heightmaps.md).
- [Noise generation](noise-generation.md), [density functions](density-functions.md)
  and [terrain filling](terrain-filling.md).
- [Biome lookup](biome-lookup.md), [biome temperatures](biome-temperatures.md)
  and [feature placement](feature-placement.md).
- [Carving](carving.md), [structure density](structure-density.md),
  [vertical anchors](vertical-anchors.md) and [scheduled ticks](scheduled-ticks.md).
- [Block-state codecs](block-state-codecs.md), [collision geometry](collision-geometry.md),
  [face visibility](face-visibility.md) and [material surfaces](material-surfaces.md).
- [Material conditions](material-conditions.md),
  [material rule libraries](material-rule-libraries.md) and
  [value providers](value-providers.md).

## Parity and validation

- [Rust rewrite status](rust-parity.md) and the
  [parity workflow](parity-workflow.md).
- [Port ledger](port-ledger.md), [version ports](version-ports.md) and
  [decompilation status](decompilation-status.md).
- [Research lab](research-lab.md).
- Area notes: [block behaviours and ticking](parity/blocks.md),
  [section meshing](parity/meshing.md),
  [redstone machines tick by tick](parity/redstone-diff.md) and
  [the same seed in vanilla and Horizon](parity/world-compare.md).

## Benchmarks

- [Animated sprite mips](animated-sprite-mips-benchmark.md)
- [Generation scan](generation-scan-benchmark.md)
- [Noise aquifer](noise-aquifer-benchmark.md)
- [Portal scan](portal-scan-benchmark.md)
- [Random tick eligibility](random-tick-eligibility-benchmark.md)
- [Shared model library](shared-model-library-benchmark.md)
- [Surface generation](surface-generation-benchmark.md)
- [Visibility](visibility-benchmark.md)
- [Entity culling](media/entity-culling-benchmark.md)
- [Lighting worker results](performance/lighting-worker-results.md)
- [Paletted container access results](performance/paletted-container-access-results.md)
- [Startup stage 3](performance/startup-stage-3.md)

## Design notes

- [Offscreen frame compositor](design/compositor.md)
- [Linear light for HDR and shader packs](design/hdr-linear-light.md)
- [Shared audio output](design/shared-audio.md)

## Other

- [Forking](forking.md): building the engine without Horizon's own extras.
- [Known issues](known-issues.md).
- [Datapack upgrade](datapack-upgrade.md) and
  [native block clone items](native-block-clone-item.md).
- [Pregeneration](pregeneration.md).
