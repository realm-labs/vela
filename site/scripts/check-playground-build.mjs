import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';

const decode = (file) => new TextDecoder('utf-8', { fatal: true, ignoreBOM: true })
  .decode(readFileSync(file));
const data = new URL('../src/data/', import.meta.url);
const examples = JSON.parse(decode(new URL('playground-examples.json', data)));
assert.equal(examples.length, 4);
assert.deepEqual(
  readdirSync(new URL('playground/', data)).filter((name) => name.endsWith('.vela')).sort(),
  examples.map((example) => example.sourceFile).sort(),
);
const expected = examples.map(({ title, entry, sourceFile }) => ({
  title, entry, source: decode(new URL(`playground/${sourceFile}`, data)),
}));

// Inspect the actual component payload after Astro's raw imports and HTML escaping.
// This does not execute browser JS, WASM glue, or Compile/Run buttons.
for (const page of ['playground/index.html', 'zh/playground/index.html']) {
  const html = decode(new URL(`../dist/${page}`, import.meta.url));
  const payloads = [...html.matchAll(/data-examples="([^"]*)"/g)];
  assert.equal(payloads.length, 1, `${page}: exactly one playground payload`);
  const entities = { amp: '&', quot: '"', apos: "'", lt: '<', gt: '>', '#39': "'" };
  const payload = payloads[0][1].replace(/&(amp|quot|apos|lt|gt|#39);/g, (_, key) => entities[key]);
  assert.deepEqual(JSON.parse(payload), expected, `${page}: exact source bytes and entries`);
  console.log(`${page}: all four emitted sources/entries match their authored origins`);
}
