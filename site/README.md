# Vela GitHub Pages

This directory is the static GitHub Pages source for Vela documentation and the browser playground.

The Pages workflow builds `vela_playground_wasm` for `wasm32-unknown-unknown`, runs `wasm-bindgen`, writes the browser package to `site/pkg`, and deploys this directory as the Pages artifact.

Local build:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/vela_playground_wasm.wasm --target web --out-dir site/pkg
python3 -m http.server 8080 --directory site
```
