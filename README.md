# PGlove — A Browser With A New Generation

The PGlove desktop browser: a real, standalone browser app built with **Tauri 2 (Rust + WebView2)**.
First theme: **Pokémon (v1.0.4)**. Made for Windows, made in India 🇮🇳.

Every tab is its own embedded WebView2 webview inside one window, so tabs keep
real per-tab history, cookies and state — just like a real browser.

## Features (v1.0)

- Real tabbed browsing — open, close, switch, middle-click to close
- Address bar: type a URL or a search (DuckDuckGo fallback), `Ctrl+L` to focus
- Back / Forward (Alt+← / Alt+→), Reload / Stop (Ctrl+R / Esc)
- New tabs (Ctrl+T), close tabs (Ctrl+W)
- Popups / `target=_blank` links open as new tabs
- Pokémon-themed chrome + a Poké-style start/new-tab page
- DSL-free frontend: plain HTML/CSS/JS bundled with esbuild

## Requirements (Windows)

- Rust toolchain (MSVC): `winget install Rustlang.Rustup` + **Visual Studio Build Tools**
  with the "Desktop development with C++" workload
- WebView2 Runtime (preinstalled on Windows 10/11)
- Node.js 18+ (for the frontend build + Tauri CLI)

## Run it

```bash
npm install
npm run dev          # builds the frontend, then launches the app in dev mode
```

Or build a release installer:

```bash
npm run tauri build  # produces an NSIS installer in src-tauri/target/release/bundle
```

## Project layout

```
src/              browser chrome UI (HTML/CSS/JS)  -> bundled into dist/
src-tauri/        Rust backend
  src/browser.rs   tab/webview manager + IPC commands
  tauri.conf.json  window + bundle config
scripts/          frontend build + icon generation
```

## Roadmap

- v1.0.5 — Minecraft theme
- v1.0.6 — GTA 6 theme
- v1.0.7 — Vice City theme

Web: https://pglove.jo3.org · GitHub: https://github.com/TechAdityaBRO/PGlove