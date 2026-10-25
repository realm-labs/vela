# Vela Website

This directory contains the Astro Starlight documentation site and the browser playground shell.

Local development:

```bash
cd site
npm ci
npm run dev
```

Documentation checks:

```bash
npm run test:syntax
npm run test:docs
```

`test:docs` checks placeholders and the reduced-surface contract: bilingual key
semantics, removal-note navigation, and no removed syntax in runnable examples.
It does not claim runtime or installed-editor acceptance.

Build the playground WASM package before testing the playground route:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
wasm-bindgen ../target/wasm32-unknown-unknown/release/vela_playground_wasm.wasm --target web --out-dir public/pkg
npm run build
```
