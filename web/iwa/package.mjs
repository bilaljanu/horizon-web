// Package a complete web/package.py output, including private local resources.
import { readFileSync, readdirSync, lstatSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, relative, extname, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createPrivateKey } from 'node:crypto';
import { BundleBuilder } from 'wbn';
import { SignedWebBundle, NodeCryptoSigningStrategy, WebBundleId } from 'wbn-sign';

const here = dirname(fileURLToPath(import.meta.url));
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm',
    '.json': 'application/json', '.webmanifest': 'application/manifest+json',
    '.svg': 'image/svg+xml', '.png': 'image/png', '.txt': 'text/plain', '.gz': 'application/gzip' };
export const csp = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; " +
    "style-src 'self' 'unsafe-inline'; img-src 'self' https: blob: data:; " +
    "connect-src 'self' https: wss: blob: data:; worker-src 'self'; " +
    "object-src 'none'; base-uri 'none'; require-trusted-types-for 'script'; trusted-types horizon-worker";

export function buildBundle(site, origin, version) {
    if (!/^[1-9]\d*(?:\.(?:0|[1-9]\d*)){0,3}$/.test(version)) {
        throw new Error('Version must be numeric, for example 1.0.0');
    }
    for (const name of ['index.html', 'main.js', 'worker.js', 'audio.js',
        'pkg/horizon-client.js', 'pkg/horizon-client_bg.wasm', 'fs/boot.tar.gz', 'fs/manifest.txt']) {
        if (!lstatSync(resolve(site, name)).isFile()) throw new Error(`Missing file: ${name}`);
    }
    const entries = new Map();
    function walk(dir) {
        for (const name of readdirSync(dir).sort()) {
            const path = resolve(dir, name), stat = lstatSync(path);
            if (stat.isSymbolicLink()) throw new Error(`Package a materialized folder, not symlinks: ${path}`);
            if (name.startsWith('.') || /\.(pem|key|swbn)$/i.test(name)) throw new Error(`Unexpected private/build file: ${path}`);
            if (stat.isDirectory()) walk(path);
            else if (stat.isFile()) entries.set(relative(site, path).replaceAll('\\', '/'), readFileSync(path));
        }
    }
    walk(site);
    entries.delete('sw.js');
    // Packed resources also use cache-busting queries in the regular web build.
    const manifest = entries.get('fs/manifest.txt').toString().split('\n').map(line => {
        const fields = line.split('\t');
        if (fields[2]) fields[2] = fields[2].split('?')[0];
        if (fields[0]) {
            const resource = new URL(fields[2] || fields[0], origin + 'fs/res/');
            if (!resource.href.startsWith(origin + 'fs/') || !entries.has(decodeURIComponent(resource.pathname.slice(1)))) {
                throw new Error(`Missing or unsafe manifest resource: ${line}`);
            }
        }
        return fields.join('\t');
    }).join('\n');
    entries.set('fs/manifest.txt', Buffer.from(manifest));
    entries.set('index.html', Buffer.from(entries.get('index.html').toString().replace('</head>',
        '<link rel="manifest" href="/.well-known/manifest.webmanifest">\n</head>')));
    entries.set('.well-known/manifest.webmanifest', Buffer.from(JSON.stringify({
        id: '/', name: 'CMB', short_name: 'CMB', version,
        start_url: '/index.html', scope: '/', display: 'standalone',
        update_manifest_url: "https://github.com/bilaljanu/horizon-web/releases/latest/download/update.json", background_color: '#000000', theme_color: '#182c38',
        icons: [{ src: '/icon.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'any' }],
    })));
    entries.set('icon.svg', readFileSync(resolve(here, 'icon.svg')));
    const builder = new BundleBuilder('b2');
    for (const [name, body] of entries) {
        const headers = { 'content-type': types[extname(name)] || 'application/octet-stream',
            'content-security-policy': csp, 'cross-origin-opener-policy': 'same-origin',
            'cross-origin-embedder-policy': 'require-corp', 'cross-origin-resource-policy': 'same-origin' };
        builder.addExchange(new URL(name, origin).href, 200, headers, body);
        if (name === 'index.html') builder.addExchange(origin, 200, headers, body);
    }
    return builder.createBundle();
}

async function main() {
    const [folder, keyFile, version = '1.0.0', output = resolve(here, '../out/horizon.swbn')] = process.argv.slice(2);
    if (!folder || !keyFile) throw new Error('Usage: npm run package -- SITE_FOLDER KEY.pem [VERSION] [OUTPUT.swbn]');
    const site = resolve(folder);
    const key = createPrivateKey({ key: readFileSync(keyFile),
        passphrase: process.env.WEB_BUNDLE_SIGNING_PASSPHRASE });
    const origin = new WebBundleId(key).serializeWithIsolatedWebAppOrigin();
    const bundle = buildBundle(site, origin, version);
    const signed = await SignedWebBundle.fromWebBundle(bundle, [new NodeCryptoSigningStrategy(key)]);
    mkdirSync(dirname(resolve(output)), { recursive: true });
    writeFileSync(output, signed.getSignedWebBundleBytes());
    console.log(`Created ${resolve(output)}\nApp origin: ${origin}\nVersion: ${version}`);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
