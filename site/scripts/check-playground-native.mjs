import assert from 'node:assert/strict';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { runPlaygroundProcess } from './playground-process.mjs';

const testName = 'advertised_site_examples_compile_and_run';

// Deliberately specific to this one repository-only libtest target, not a runner API.
export function assertSiteDiscovery(stdout) {
  assert.equal(
    stdout.trim(),
    `${testName}: test\n\n1 test, 0 benchmarks`,
    `actual-site discovery must select exactly one ignored test named ${testName}`,
  );
}

export function assertSiteExecution(stdout) {
  const lines = stdout.split('\n');
  assert.deepEqual(lines.filter((line) => /^running \d+ tests?$/.test(line)), ['running 1 test'],
    'actual-site execution must run exactly one test');
  assert.deepEqual(lines.filter((line) => /^test .* \.\.\. /.test(line)), [`test ${testName} ... ok`],
    'actual-site execution must pass the selected test exactly once');
  const summaries = lines.filter((line) => line.startsWith('test result:'));
  assert.equal(summaries.length, 1, 'actual-site execution must have exactly one result summary');
  assert.match(summaries[0],
    /^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in \d+(?:\.\d+)?s$/,
    'actual-site execution must pass one test with no ignored or filtered-out tests');
}

async function main() {
  const target = ['test', '-p', 'vela_playground_wasm', '--locked', '--test', 'site_examples', '--'];
  const selection = ['--ignored', '--exact', testName, '--format', 'pretty', '--color', 'never'];
  const options = {
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: {
      ...process.env,
      VELA_SITE_DATA_DIR: fileURLToPath(new URL('../src/data/', import.meta.url)),
    },
  };
  // Listing with the same ignored/exact selection catches a stale name or a lost
  // #[ignore] before execution. A successful listing alone cannot prove execution.
  const discovery = await runPlaygroundProcess('cargo', [...target, ...selection, '--list'], options);
  process.stdout.write(discovery.stdout);
  process.stderr.write(discovery.stderr);
  assertSiteDiscovery(discovery.stdout);

  const result = await runPlaygroundProcess('cargo', [...target, ...selection, '--show-output'], options);
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  assertSiteExecution(result.stdout);
  console.log('Actual-site test discovered and passed exactly once.');
}

// Importing the two assertions in regression tests must not launch Cargo.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    await main();
  } catch (error) {
    process.stdout.write(error.result?.stdout ?? '');
    process.stderr.write(error.result?.stderr ?? '');
    console.error(error.message);
    process.exitCode = 1;
  }
}
