import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createHighlighter } from 'shiki';

import velaGrammar from '../src/syntax/vela.tmLanguage.json' with { type: 'json' };

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const siteRoot = path.resolve(scriptDir, '..');
const fixtureDir = path.join(siteRoot, 'src', 'syntax', 'fixtures');
const expectedPath = path.join(fixtureDir, 'complete.expected-scopes.json');

const expected = JSON.parse(await readFile(expectedPath, 'utf8'));
const source = await readFile(path.join(fixtureDir, expected.fixture), 'utf8');
const highlighter = await createHighlighter({
  themes: ['github-dark'],
  langs: [velaGrammar],
});

assert(
  highlighter.getLoadedLanguages().includes(expected.language),
  `expected Shiki to load language '${expected.language}'`
);

const highlighted = highlighter.codeToTokens(source, {
  lang: expected.language,
  theme: 'github-dark',
  includeExplanation: true,
});

const tokens = [];
const seenScopes = new Set();

for (const [lineIndex, line] of highlighted.tokens.entries()) {
  for (const token of line) {
    const tokenScopes = new Set();

    for (const explanation of token.explanation ?? []) {
      const explanationScopes = explanation.scopes.map((scope) => scope.scopeName);
      explanationScopes.forEach((scope) => {
        seenScopes.add(scope);
        tokenScopes.add(scope);
      });

      tokens.push({
        line: lineIndex + 1,
        content: explanation.content,
        scopes: explanationScopes,
      });
    }

    tokens.push({
      line: lineIndex + 1,
      content: token.content,
      scopes: [...tokenScopes],
    });
  }
}

for (const scope of expected.requiredScopes) {
  assert(seenScopes.has(scope), `missing required Vela scope: ${scope}`);
}

for (const required of expected.requiredTokens) {
  const found = tokens.some((token) => {
    return token.content === required.content && token.scopes.includes(required.scope);
  });

  assert(
    found,
    `missing token '${required.content}' with scope '${required.scope}'`
  );
}

console.log(
  `Vela highlighting contract passed: ${expected.requiredScopes.length} scopes, ${expected.requiredTokens.length} token checks`
);
