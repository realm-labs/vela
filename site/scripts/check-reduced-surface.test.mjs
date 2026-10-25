import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const docsRoot = path.join(siteRoot, 'src/content/docs');
const removedValues = /\bhost\s+refs?\b|\bhost\s+references?\b|\bpath\s+prox(?:y|ies)\b|HostRef|HostPath|PathProxy/i;
const removedCode = /\basync\s+fn\b|\.await\b|\bextern\s+state\b|\bwith_host_(?:ref|mut)\b|\bhost::(?:release|try_release)\b|\btask::spawn_scoped\b|\bHostAccess\b|\bctx\.emit\s*\(/;
const activeHostClaims = [
  /(?:script\s+or\s+host\s+type|脚本类型或宿主类型)/i,
  /Native\s+functions\s+(?:and|和)\s+host\s+methods/i,
  /注册宿主类型也可以暴露方法|(?:through|通过)\s+HostAccess/i,
  /脚本方法、宿主方法|已注册的\s+host\s+types、native\s+functions、host\s+methods/i,
  /调用参数或\s+adapter\s+修改宿主状态/i,
  /explicit\s+state\s+adapters|显式\s+state\s+adapter/i,
  /`[A-Za-z_]\w*\.[A-Za-z_]\w*`[\s\S]{0,80}(?:rejected|missing|缺失)[\s\S]{0,40}`Host(?:Read|Write)`/i,
  /\+=.*\/\/.*(?:requires\s+host\s+read|需要\s+host\s+read)/i,
  /`Context\.emit`/,
];

async function markdownFiles(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...await markdownFiles(fullPath));
    else if (/\.mdx?$/.test(entry.name)) files.push(fullPath);
  }
  return files;
}

test('key-policy chapters describe owned values, not removed host identity keys', async () => {
  for (const locale of ['', 'zh/']) {
    for (const chapter of ['data/maps.md', 'data/sets.md', 'stdlib/set-methods.md', 'language/type-hints-guards.md']) {
      const name = `${locale}${chapter}`;
      const content = await readFile(path.join(docsRoot, name), 'utf8');
      assert.doesNotMatch(content, removedValues, name);
      assert.match(content, /ValueKey/, name);
      assert.match(content, locale ? /按值/ : /by value/, name);
      assert.match(content, locale ? /按身份/ : /by identity/, name);
      assert.match(content, locale ? /脚本(?:自有|拥有)|由脚本拥有/ : /script-owned/, name);
    }
  }
});

test('removed handle claims stay out of retained chapters and runnable examples', async () => {
  for (const file of await markdownFiles(docsRoot)) {
    const content = await readFile(file, 'utf8');
    const name = path.relative(docsRoot, file);
    // Stable removal-note URLs deliberately name the obsolete surface.
    if (!name.endsWith('host/hostref-hostpath-pathproxy.md')) {
      assert.doesNotMatch(content, removedValues, name);
    }
    for (const block of content.matchAll(/```(?:vela|rust)\s*\n([\s\S]*?)```/g)) {
      assert.doesNotMatch(block[1], removedCode, `${name}: runnable example`);
    }
  }
});

test('host permission examples use owned native effects, not grants on script fields', async () => {
  for (const locale of ['', 'zh/']) {
    const name = `${locale}host/capabilities-execution-budgets.md`;
    const content = await readFile(path.join(docsRoot, name), 'utf8');
    assert.match(content, /register_typed_native_fn::<\(\), _>/, name);
    assert.match(content, /register_typed_native_fn::<\(i64,\), _>/, name);
    assert.match(content, /\.effects\(EffectSet::host_read\(\)\)/, name);
    assert.match(content, /\.effects\(EffectSet::host_write\(\)\)/, name);
    assert.match(content, /\.returns\(TypeHint::i64\(\)\)/, name);
    assert.match(content, /\.returns\(TypeHint::unit\(\)\)/, name);
    assert.match(content, locale
      ? /脚本自有字段[^。]*不需要[^。]*`HostRead`[^。]*`HostWrite`/
      : /Script-owned fields do not require[^.]*`HostRead`[^.]*`HostWrite`/, name);
    assert.match(content, /counter\.value \+= 1/, name);
  }
});

test('retained bilingual chapters do not advertise Rust receiver dispatch or write-through permissions', async () => {
  for (const file of await markdownFiles(docsRoot)) {
    const content = await readFile(file, 'utf8');
    const name = path.relative(docsRoot, file);
    for (const claim of activeHostClaims) assert.doesNotMatch(content, claim, name);
  }
  for (const locale of ['', 'zh/']) {
    for (const chapter of ['host/engine-runtime.md', 'methods/inherent-methods.md',
      'methods/traits-trait-methods.md', 'methods/standard-library-methods.md', 'stdlib/overview.md']) {
      const name = `${locale}${chapter}`;
      const content = await readFile(path.join(docsRoot, name), 'utf8');
      assert.doesNotMatch(content, /HostAccess/, name);
      assert.match(content, locale ? /脚本/ : /script/, name);
    }
  }
});

test('dispatch and permission regressions detect old claims but allow removal notes and native grants', () => {
  for (const stale of [
    'They describe methods a script or host type can implement.',
    'Trait 用来描述脚本类型或宿主类型可以实现的方法。',
    'Native functions 和 host methods 声明 effects',
    '注册宿主类型也可以暴露方法，但这些调用会通过 HostAccess 执行。',
    '它们和脚本方法、宿主方法一样按 receiver 分发',
    'Engine 保存已注册的 host types、native functions、host methods',
    '通过调用参数或 adapter 修改宿主状态',
    '`player.level` can be rejected by missing `HostRead`',
    '`counter.value` can be rejected by missing `HostWrite`',
    '读取 `player.level` 可能被\n缺失 `HostRead` 拒绝',
    'player.level += 1; // requires host read and host write access',
    'player.level += 1; // 需要 host read 和 host write access',
    'Use explicit state adapters',
    '通过显式 state adapter 通信',
    '`Context.emit` requires context schema registration',
  ]) assert.ok(activeHostClaims.some((pattern) => pattern.test(stale)), stale);
  for (const retained of [
    'Host constructors and host methods are removed; HostAccess is not supported.',
    '宿主方法和 HostAccess 已删除；脚本方法继续保留。',
    'Registered native effects require Capability::HostRead and Capability::HostWrite.',
    'Script-owned fields do not require Rust host grants: `HostRead` or `HostWrite`.',
    '脚本自有字段不需要 Rust 宿主授权：`HostRead` 或 `HostWrite`。',
    'Rust mutation is not exposed through call arguments or adapters.',
  ]) assert.ok(activeHostClaims.every((pattern) => !pattern.test(retained)), retained);
});

test('stable host chapter URLs are labeled as removals in both navigation locales', async () => {
  const config = await readFile(path.join(siteRoot, 'astro.config.mjs'), 'utf8');
  for (const slug of ['hostref-hostpath-pathproxy', 'host-object-lifetime', 'hostaccess-write-through']) {
    const entry = config.split('\n').find((line) => line.includes(`link: '/host/${slug}/'`));
    assert.ok(entry, `missing stable URL: ${slug}`);
    assert.match(entry, /label: "Removed /, slug);
    assert.match(entry, /'zh-CN': "已移除的/, slug);
  }
});

test('regression patterns detect the former bilingual claims, not owned identity semantics', () => {
  for (const stale of [
    'script heap objects and host refs compare by identity',
    'script heap objects and host\nrefs compare by identity',
    '脚本堆对象和 host ref 按身份比较',
    '`PathProxy` 等临时值会在修改前被拒绝',
    'Captured host values remain host references or path proxies',
  ]) assert.match(stale, removedValues);
  assert.doesNotMatch('Script-owned records compare by identity; strings compare by value.', removedValues);
  for (const stale of ['async fn run() {}', 'value.await', 'extern state account: Account']) {
    assert.match(stale, removedCode);
  }
});
