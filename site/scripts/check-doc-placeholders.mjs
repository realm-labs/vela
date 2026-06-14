import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const docsRoot = path.resolve(scriptDir, '..', 'src', 'content', 'docs');

async function collectMarkdownFiles(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...await collectMarkdownFiles(fullPath));
    } else if (entry.name.endsWith('.md') || entry.name.endsWith('.mdx')) {
      files.push(fullPath);
    }
  }

  return files;
}

const placeholders = [
  /\bTODO\b/i,
  /Detailed chapters will be filled in/i,
  /This chapter belongs to \*\*/i,
  /document the semantics, examples, host boundary behavior, and common errors/i,
  /add runnable Vela or Rust embedding examples/i,
];

const failures = [];

for (const file of await collectMarkdownFiles(docsRoot)) {
  const content = await readFile(file, 'utf8');

  for (const placeholder of placeholders) {
    if (placeholder.test(content)) {
      failures.push(`${path.relative(docsRoot, file)} matches ${placeholder}`);
    }
  }
}

assert.equal(
  failures.length,
  0,
  `documentation placeholders remain:\n${failures.join('\n')}`
);

console.log('Documentation placeholder check passed');
