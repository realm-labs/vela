import test from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { assertSiteDiscovery, assertSiteExecution } from './check-playground-native.mjs';
import { runPlaygroundProcess } from './playground-process.mjs';

const name = 'advertised_site_examples_compile_and_run';
const discovered = `${name}: test\n\n1 test, 0 benchmarks\n`;
const executed = `\nrunning 1 test\ntest ${name} ... ok\n\nsuccesses:\n\n---- ${name} stdout ----\nLevel reward: 343 bytes, entry main, compile/run checked\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s\n\n`;
const zero = '\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n\n';

test('actual-site guards accept exactly one discovery and named successful execution', () => {
  assertSiteDiscovery(discovered);
  assertSiteExecution(executed);
});

test('actual-site discovery rejects stale, empty and duplicate selections', () => {
  for (const output of [
    '',
    '0 tests, 0 benchmarks\n',
    discovered.replace(name, 'stale_name'),
    discovered + discovered,
    `${name}: test\nanother_test: test\n\n2 tests, 0 benchmarks\n`,
  ]) assert.throws(() => assertSiteDiscovery(output), /discovery must select exactly one ignored test/);
});

test('actual-site execution rejects zero, ignored, wrong-name and repeated results', () => {
  for (const output of [
    '',
    zero,
    executed.replace(`test ${name} ... ok`, `test ${name} ... ignored`),
    executed.replace(`test ${name} ... ok`, 'test stale_name ... ok'),
    executed.replace('1 passed; 0 failed; 0 ignored', '0 passed; 0 failed; 1 ignored'),
    executed.replace('0 filtered out', '1 filtered out'),
    executed.replace('0 failed', '1 failed'),
    executed.replace('1 passed', '2 passed'),
    executed.replace('test result: ok.', 'test result: FAILED.'),
    executed + `test ${name} ... ok\n`,
    executed + executed.slice(executed.indexOf('test result:')),
    executed + executed,
  ]) assert.throws(() => assertSiteExecution(output), /actual-site execution/);
});

const processOptions = {
  cwd: fileURLToPath(new URL('../../', import.meta.url)),
  timeoutMs: 30_000,
};
const liveTestOptions = { timeout: 65_000, skip: !['darwin', 'linux'].includes(process.platform) };

test('real libtest stale exact filter exits zero but both guards reject it', liveTestOptions, async () => {
  const args = ['test', '-p', 'vela_playground_wasm', '--locked', '--test', 'site_examples',
    '--', '--ignored', '--exact', 'sf11_stale_test_name', '--format', 'pretty', '--color', 'never'];
  const listing = await runPlaygroundProcess('cargo', [...args, '--list'], processOptions);
  assert.equal(listing.code, 0);
  assert.equal(listing.stdout.trim(), '0 tests, 0 benchmarks');
  assert.throws(() => assertSiteDiscovery(listing.stdout), /discovery must select exactly one ignored test/);

  const run = await runPlaygroundProcess('cargo', [...args, '--show-output'], processOptions);
  assert.equal(run.code, 0);
  assert.match(run.stdout, /running 0 tests/);
  assert.throws(() => assertSiteExecution(run.stdout), /execution must run exactly one test/);
});

test('real libtest ignored-only discovery excludes a nonignored test', liveTestOptions, async () => {
  // Models loss of #[ignore] without modifying the actual repository-only Rust test.
  const listing = await runPlaygroundProcess('cargo', [
    'test', '-p', 'vela_playground_wasm', '--locked', '--lib', '--', '--ignored',
    '--exact', 'tests::run_script_reports_runtime_error', '--list', '--format', 'pretty', '--color', 'never',
  ], processOptions);
  assert.equal(listing.code, 0);
  assert.equal(listing.stdout.trim(), '0 tests, 0 benchmarks');
  assert.throws(() => assertSiteDiscovery(listing.stdout), /discovery must select exactly one ignored test/);
});
