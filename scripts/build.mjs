// Builds the browser chrome frontend: bundles JS with esbuild and copies
// static assets (HTML/CSS) into dist/. `dist` is tauri.conf.json's frontendDist.
import { build } from 'esbuild';
import { cpSync, mkdirSync, rmSync } from 'node:fs';

rmSync('dist', { recursive: true, force: true });
mkdirSync('dist', { recursive: true });

await build({
  entryPoints: ['src/main.js'],
  bundle: true,
  format: 'esm',
  outfile: 'dist/main.js',
  target: 'chrome120',
  sourcemap: true,
  logLevel: 'info',
});

for (const f of ['index.html', 'start.html', 'styles.css']) {
  cpSync(`src/${f}`, `dist/${f}`);
  console.log(`copied src/${f} -> dist/${f}`);
}