import init from './wasm_probe.js';
try {
  const wasm = await init();
  const code = wasm.horizon_java_probe();
  postMessage({passed: code === 0, code, scope: 'Jar lifecycle, Fabric/NeoForge lifecycle, Java registrations to Horizon blocks/items/assets/creative tab'});
} catch (error) {
  let checkpoint = null;
  try { checkpoint = wasm.horizon_java_checkpoint(); } catch (e) {}
  postMessage({passed: false, error: String(error), checkpoint, scope: 'Jar lifecycle and gameplay content pipeline'});
}
