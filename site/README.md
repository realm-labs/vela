# Vela Website

This directory contains the Astro Starlight documentation site and the browser playground shell.

Local development:

```bash
cd site
npm ci
npm run dev
```

Build the playground WASM package before testing the playground route:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
wasm-bindgen ../target/wasm32-unknown-unknown/release/vela_playground_wasm.wasm --target web --out-dir public/pkg
npm run build
```
