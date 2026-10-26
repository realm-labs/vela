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

`src/data/playground-examples.json` owns the advertised titles, entry names and
source selection. Its four `src/data/playground/*.vela` files are the only authored
site sources (including their missing final newlines); Astro imports them raw.
`npm run build` also compares the emitted English/Chinese playground payloads
with those exact files and entries.

From a repository checkout, run `npm run test:playground` for the bounded native
compile/run lane through `vela_playground_wasm` (Cargo must be installed). It checks
independent results 40, 37, 50 and the runtime division-by-zero diagnostic. This
explicitly runs the ignored `site_examples` integration test with
`VELA_SITE_DATA_DIR` pointing at this directory's `src/data`; no external
`include_str!`, copied source, or production dependency on repository test data
is involved. Ordinary Cargo tests/builds do not require the site tree. A standalone
Cargo source package cannot run this repository lane without the site inputs;
its ordinary native fixtures remain package-local. These checks and the wasm32
build are not a real-browser smoke test.

The native launcher currently requires macOS/Linux POSIX process groups (tested
on macOS arm64; Linux unverified). Windows is rejected before spawning; there is
no tested `taskkill` implementation. It buffers at most 4 MiB per output stream,
uses a 180-second execution deadline, and allows two seconds for hard-kill/reap
cleanup. Ordinary descendants in the command's process group are cleaned even
if the launcher exits first; deliberately detached/`setsid` daemons are not
contained. Cleanup failure is reported, never treated as successful execution.
Run its finite regression probes with
`node --test site/scripts/playground-process.test.mjs` from the repository root.

Build the playground WASM package before testing the playground route:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
wasm-bindgen ../target/wasm32-unknown-unknown/release/vela_playground_wasm.wasm --target web --out-dir public/pkg
npm run build
```
