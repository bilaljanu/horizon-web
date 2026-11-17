# Experimental Java mod execution

Horizon runs a **narrow vertical slice** of real Fabric/NeoForge `.jar` mods in
gameplay. `horizon-java`, a shared Rust bytecode interpreter and jar inspector,
executes a jar's entrypoint, maps its block/item registry calls into Horizon's
existing content system, and dispatches its server-tick callbacks. The same Rust
code runs natively and compiled to WebAssembly; there is no JNI and no second
engine.

This is not general Fabric/NeoForge compatibility. Only the small Minecraft API
surface listed below is implemented, and metadata inspection always leaves
compatibility **unverified**. A jar that uses any wider API fails explicitly with
class, descriptor and bytecode context; it does not silently no-op.

## What is verified

- Fabric/NeoForge metadata, version predicates, dependency ordering diagnostics,
  class-file parsing, access-widener/transformer parsing, and archive path checks.
- `javac --release 21` bytecode: integer loops, dense/sparse switches, arrays,
  reference comparison, inherited/interface dispatch, caught arithmetic/null
  exceptions, static initialization, captured lambdas, string concatenation,
  category-two stack operations, instruction-budget failure and recovery.
- A real Fabric-shaped jar runs its `ModInitializer`, registers a block item
  (`<mod>:ruby_block`), an item (`<mod>:ruby`), a `ServerTickEvents.END_SERVER_TICK`
  callback, and two ticks produce `tick=1` and `tick=2`.
- A NeoForge-shaped jar runs its matching `@Mod` zero-argument constructor and
  performs the same registrations.
- The Java registrations become Horizon `Block`/`Item` content declarations, a
  `<mod>:items` creative tab, and overlay files read from the jar: the block
  texture PNG, block/item models, and the block loot table. The game's real PNG
  decoder reads the texture at 16×16 from the resource overlay.
- **In-game gameplay** (native, headless integration test): the creative tab
  lists both items; survival placement consumes one stack of the block item;
  breaking returns the drop; the placed block survives a save, a fresh load of
  the on-disk region and a re-read; custom item NBT round-trips.
- **Browser/WASM**: both jars execute unchanged in a module Web Worker. The probe
  reports success only after matching the host logs, the recorded block/item
  registrations, the jar texture/model/loot files, the declared content and the
  creative tab. Verified result: `{"passed":true,"code":0}`.
- Local `.jar` import in the Mods interface: the browser opens a file chooser
  (`pick_file`)/per-frame `take_picked_named`, the native build installs a `.jar`
  dropped on the Mods page. Both validate the archive, reduce the supplied name
  to one path component, store it under `horizon-mods/`, enable it and refresh
  the list. Browser storage persists `horizon-mods/*.jar` because it is outside
  the scratch `/tmp` and `/.local` roots.
- `cargo test -p horizon-java`: 29 tests pass (21 unit, 5 execution, 3 loader).
- `local_jar_import_validates_stores_and_enables`,
  `mods_page_import_button_asks_the_game_to_pick_a_jar`, and the headless
  gameplay test `java_jars_place_break_and_save_in_real_game` pass.

The fixtures (`crates/horizon-java/testdata/`) compile against small,
source-visible signature stubs that are **not packaged in the jars**. They are
built with a local JDK 21 and bundle original checkerboard textures, not
proprietary Minecraft assets. They are not validated against official
Fabric/NeoForge distributions or Minecraft 26.3 artifacts.

## Reproduce

```sh
# Fixtures (needs a JDK 21)
JAVA_HOME=/path/to/jdk-21 sh crates/horizon-java/testdata/build.sh
cargo test -p horizon-java

# The real-game slice: the game's resources must be present.
HORIZON_TEST_RESOURCES=/path/to/.local/horizon-client/26.3 \
  cargo test -p horizon-client --lib --no-default-features \
    java_jars_place_break_and_save_in_real_game -- --ignored --nocapture

cargo test -p horizon-client --lib --no-default-features \
  local_jar_import_validates_stores_and_enables
cargo test -p horizon-client-render --lib --no-default-features mods_page
cargo check -p horizon-client --lib --no-default-features
```

Browser runtime probe (needs the pinned nightly, `rust-src`, a matching
`wasm-bindgen` CLI and the threaded flags `web/build.sh` uses):

```sh
cargo +nightly-2026-10-14 -Z build-std=std,panic_abort build \
  --target wasm32-unknown-unknown -p horizon-java --example wasm_probe
wasm-bindgen --target web --out-dir /tmp/opencode/java-browser \
  target/wasm32-unknown-unknown/debug/examples/wasm_probe.wasm
cp crates/horizon-java/testdata/probe{.html,-worker.js} /tmp/opencode/java-browser/
python3 -m http.server 8765 --directory /tmp/opencode/java-browser
```

Open `http://localhost:8765/probe.html` in a fresh tab (cache-bust the URL when
replacing a previous build). Expected: `passed: true`, `code: 0`, running inside
a module worker. This exercises jar lifecycle, both loader shapes, Java
registrations, content declarations, assets and the creative tab. It does not
start the renderer or WebGPU and is not an IWA packaging test.

## Deliberate limits

- **Tiny registry ABI only.** The implemented Minecraft surface is
  `ResourceLocation`, `Registry.register`, `BuiltInRegistries.BLOCK/ITEM`,
  `Block`/`BlockItem`/`Item`/`Properties`, Fabric's tick event and the
  NeoForge-style `@Mod` constructor. Any other class, method or descriptor fails
  explicitly. Third-party mods that use Minecraft's wider API, resource reloads,
  data generation or client code are not supported.
- **No official loader.** Metadata inspection never proves execution. Dependency
  ordering is a diagnostic; multiple jars are not booted in dependency order.
  Multi-mod jars, nested jars and cross-mod class loading are not implemented.
- **Mixins, access wideners and access transformers** are parsed/diagnosed but
  not applied. The development loader refuses jars that require them. There is
  no Java instruction stream for the Rust engine to transform.
- **No invented clock or entropy.** `Math.random()` and unsupported time/entropy
  natives are not exposed as fake deterministic implementations.
- **Incomplete interpreter.** No bytecode verifier, garbage collector, full JDK,
  thread implementation or untrusted-code security boundary. Instruction/depth/
  object limits are safeguards, not a sandbox guarantee.
- **Metadata stays `Partial`/unverified.** `Support::Runnable` and `verified`
  require observed behavior, so jars are listed as experimental with a clear
  "compatibility unverified" note.
- **Not verified here:** a full browser client startup with rendering of the mod
  texture on a block (the probe checks content and assets, not the GPU), a real
  Chromebook IWA device run, and performance. The default Linux client build is
  blocked in this environment by missing `pkg-config`/ALSA; the IWA packaging
  tests need Node.js, which is not installed here. No CSP, IWA policy,
  authentication or worker configuration was changed for this work.
- `every_horizon_key_the_code_names_is_in_en_us` has a pre-existing failure
  (`horizon.profile.title` is absent from `en_us.json`), unrelated to this work.
