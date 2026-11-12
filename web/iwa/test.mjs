import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { generateKeyPairSync } from 'node:crypto';
import { Bundle } from 'wbn';
import { WebBundleId, SignedWebBundle, NodeCryptoSigningStrategy } from 'wbn-sign';
import { buildBundle } from './package.mjs';
import vm from 'node:vm';

test('bundle resolves startup and lazy packs without query strings and signs', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'horizon-iwa-'));
    try {
        mkdirSync(join(dir, 'pkg')); mkdirSync(join(dir, 'fs'));
        for (const f of ['main.js', 'worker.js', 'audio.js', 'pkg/horizon-client.js',
            'pkg/horizon-client_bg.wasm', 'fs/boot.tar.gz', 'fs/worldgen.tar.gz']) writeFileSync(join(dir, f), 'fixture');
        writeFileSync(join(dir, 'index.html'), '<html><head></head><body></body></html>');
        writeFileSync(join(dir, 'fs/manifest.txt'), 'worldgen/test.json\t2\t../worldgen.tar.gz?v=abc\n');
        const { privateKey } = generateKeyPairSync('ed25519');
        const origin = new WebBundleId(privateKey).serializeWithIsolatedWebAppOrigin();
        const bytes = buildBundle(dir, origin, '1.0.0');
        const bundle = new Bundle(bytes);
        const get = p => bundle.getResponse(origin + p);
        assert.equal(get('pkg/horizon-client_bg.wasm').headers['content-type'], 'application/wasm');
        assert.equal(new TextDecoder().decode(get('fs/manifest.txt').body), 'worldgen/test.json\t2\t../worldgen.tar.gz\n');
        const manifest = JSON.parse(new TextDecoder().decode(get('manifest.webmanifest').body));
        assert.equal(manifest.version, '1.0.0');
        assert.equal(get(manifest.start_url.slice(1)).status, 200);
        assert.equal(get('icon.svg').status, 200);
        assert.equal(get('').status, 200);
        const signed = await SignedWebBundle.fromWebBundle(bytes, [new NodeCryptoSigningStrategy(privateKey)]);
        assert.equal(SignedWebBundle.fromBytes(signed.getSignedWebBundleBytes()).getWebBundleId(), new WebBundleId(privateKey).serialize());
        assert.throws(() => buildBundle(dir, origin, 'invalid'), /Version/);
        rmSync(join(dir, 'fs/worldgen.tar.gz'));
        assert.throws(() => buildBundle(dir, origin, '1.0.0'), /Missing or unsafe/);
    } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('worker policy allows only the bundled worker and hands it shared memory', () => {
    const source = readFileSync(new URL('../../crates/horizon-os/src/workers.rs', import.meta.url), 'utf8')
        .split('inline_js = r#"')[1].split('"#)]')[0].replaceAll('export function', 'function');
    let policy, started, message;
    const context = vm.createContext({ URL, location: { href: 'isolated-app://test/index.html' }, window: {},
        trustedTypes: { createPolicy: (_, rules) => { policy = rules; return rules; } },
        Worker: class { constructor(url) { started = url; } postMessage(value) { message = value; } }, console });
    vm.runInContext(source + '\nhorizon_spawn_worker("module", "memory", 42, "test", 4096);', context);
    assert.equal(started, 'isolated-app://test/worker.js');
    assert.equal(message.memory, 'memory');
    assert.throws(() => policy.createScriptURL('https://example.com/evil.js'), /Unexpected/);
});

test('automatic multiplayer transport uses WSS inside an IWA, preserving local HTTP', () => {
    const source = readFileSync(new URL('../../crates/horizon-os/src/net.rs', import.meta.url), 'utf8')
        .split('inline_js = r#"')[1].split('"#)]')[0].replaceAll('export function', 'function');
    for (const [protocol, expected] of [['isolated-app:', 'wss'], ['https:', 'wss'], ['http:', 'ws']]) {
        let address;
        const context = vm.createContext({ location: { protocol },
            WebSocket: class { constructor(url) { address = url; } }, console });
        vm.runInContext(source + '\nopen(1, null, "example.com", 25565, null, []);', context);
        assert.equal(address, `${expected}://example.com:25565/`);
    }
});
