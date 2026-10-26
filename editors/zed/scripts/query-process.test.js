"use strict";

const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { runQueryProcess } = require('./query-process');

const probe = path.join(__dirname, 'fixtures/process-probe.cjs');
const options = { timeout: 6_000, skip: !['darwin', 'linux'].includes(process.platform) };
const limits = { timeoutMs: 1_000, outputLimit: 4_096 };

function gone(pid) {
  assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' }, `process/group ${pid} must be reaped`);
}

function checkTree(result) {
  const ready = result.stdout.match(/READY (\{[^\n]+\})/);
  assert(ready, `probe did not start both processes: ${result.stdout}`);
  const { parent, descendant } = JSON.parse(ready[1]);
  assert.equal(parent, result.pid);
  assert(Number.isSafeInteger(descendant) && descendant > 0);
  assert.match(result.stdout, /parent ignored SIGTERM/);
  assert.match(result.stdout, /descendant ignored SIGTERM/);
  gone(parent);
  gone(descendant);
  gone(-parent);
}

test('query process captures exact stdout/stderr and rejects nonzero status', options, async () => {
  const result = await runQueryProcess(process.execPath, [probe, 'status', '0'], limits);
  assert.equal(result.stdout, 'out\0🙂\n');
  assert.equal(result.stderr, 'err\n');
  assert.equal(result.code, 0);
  gone(result.pid);
  await assert.rejects(runQueryProcess(process.execPath, [probe, 'status', '7'], limits), (error) => {
    assert.equal(error.result.code, 7);
    assert.equal(error.result.stdout, result.stdout);
    assert.equal(error.result.stderr, result.stderr);
    gone(error.result.pid);
    return true;
  });
});

test('query process spawn failure is bounded', options, async () => {
  await assert.rejects(runQueryProcess(`${probe}.missing-command`, [], limits), { code: 'ENOENT' });
});

test('query process rejects invalid limits before spawning', options, () => {
  for (const value of [0, -1, Infinity, NaN]) {
    assert.throws(() => runQueryProcess(process.execPath, [], { timeoutMs: value }), /positive finite/);
    assert.throws(() => runQueryProcess(process.execPath, [], { outputLimit: value }), /positive finite/);
  }
});

for (const mode of ['deadline', 'stdout', 'stderr']) {
  test(`query ${mode} hard-kills and reaps a SIGTERM-ignoring tree`, options, async () => {
    const started = performance.now();
    await assert.rejects(runQueryProcess(process.execPath, [probe, mode], limits), (error) => {
      assert.match(error.message, mode === 'deadline' ? /deadline exceeded/ : new RegExp(`${mode} exceeded`));
      assert.doesNotMatch(error.message, /cleanup incomplete/);
      assert.equal(error.result.signal, 'SIGKILL');
      assert(Buffer.byteLength(error.result.stdout) <= limits.outputLimit);
      assert(Buffer.byteLength(error.result.stderr) <= limits.outputLimit);
      checkTree(error.result);
      return true;
    });
    assert(performance.now() - started < 4_000, 'deadline + cleanup must beat the 8s probe watchdog');
  });
}

for (const mode of ['exit-parent', 'exit-parent-no-pipes']) {
  test(`query ${mode} cleans descendants even after successful launcher exit`, options, async () => {
    const started = performance.now();
    const result = await runQueryProcess(process.execPath, [probe, mode], limits);
    assert.equal(result.code, 0);
    checkTree(result);
    assert(performance.now() - started < 4_000);
  });
}
