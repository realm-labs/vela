"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const fs = require("node:fs"), path = require("node:path"), os = require("node:os");
const { hostIdentity, hostLogDirectory, observeHostLogs } = require("../../editors/vscode/test/input/host-identity");
const old = hostIdentity(4172, "11111111111111111111111111111111");
const current = hostIdentity(4172, "22222222222222222222222222222222");
function corpus(action) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-host-identity-"));
  const write = (folder, identity, text) => {
    const directory = path.join(root, folder); fs.mkdirSync(directory, { recursive: true });
    const file = path.join(directory, `3-${identity.name}.log`); fs.writeFileSync(file, text); return directory;
  };
  try { action(root, write); }
  finally {
    assert(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    assert(path.basename(root).startsWith("vela-host-identity-")); fs.rmSync(root, { recursive: true });
  }
}
test("reused Windows observer PID cannot select a historical reload log before the new invocation flushes", () => corpus((root, write) => {
  const previous = write("old", old, old.marker + "\n");
  assert.equal(hostLogDirectory(root, current), null, "old PID alone is not ready");
  const actual = write("new", current, current.marker + "\r\n");
  assert.equal(hostLogDirectory(root, current), actual); assert.equal(hostLogDirectory(root, old), previous);
  assert.deepEqual(current, { name: "Vela Test Host 4172 22222222222222222222222222222222", marker: "VELA_INPUT_HOST 4172 22222222222222222222222222222222" });
}));
test("observer identity rejects incomplete and wrong-run markers and duplicate current channels", () => corpus((root, write) => {
  write("new", current, old.marker + "\n"); assert.equal(hostLogDirectory(root, current), null);
  write("new", current, current.marker); assert.equal(hostLogDirectory(root, current), null);
  write("new", current, current.marker + " extra\n"); assert.equal(hostLogDirectory(root, current), null);
  write("new", current, current.marker + "\n"); assert(hostLogDirectory(root, current));
  write("duplicate", current, current.marker + "\n"); assert.throws(() => hostLogDirectory(root, current), /unique current invocation/);
}));
test("observer log identity validates PID and full independent invocation nonce", () => {
  for (const pid of [0, -1, 1.5, "4172", NaN]) assert.throws(() => hostIdentity(pid, "1".repeat(32)), /PID/);
  for (const nonce of [undefined, "", "1".repeat(31), "1".repeat(33), "G".repeat(32)]) assert.throws(() => hostIdentity(4172, nonce), /nonce/);
  assert.notDeepEqual(current, old);
});

async function observerCorpus(action) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-observer-lifecycle-"));
  try { await action(root); }
  finally {
    assert(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    assert(path.basename(root).startsWith("vela-observer-lifecycle-"));
    fs.rmSync(root, { recursive: true });
  }
}
test("installed observer owns its ready identity channel until explicit finish disposal", () => observerCorpus(async root => {
  let disposed = 0, channelName;
  const vscode = { window: { createOutputChannel(name) {
    channelName = name;
    return { appendLine(marker) { fs.writeFileSync(path.join(root, `3-${name}.log`), marker + "\n"); }, dispose() { disposed++; } };
  } } };
  const observed = await observeHostLogs(vscode, { pid: 4172, logDirectory: root });
  assert.match(channelName, /^Vela Test Host 4172 [0-9a-f]{32}$/);
  assert.equal(observed.logDirectory, root); assert.equal(disposed, 0);
  observed.dispose(); assert.equal(disposed, 1);
}));
test("failed observer identity readiness disposes its own channel and retains the original failure", () => observerCorpus(async root => {
  let disposed = 0;
  const vscode = { window: { createOutputChannel(name) {
    return { appendLine(marker) {
      for (const child of ["first", "duplicate"]) {
        fs.mkdirSync(path.join(root, child)); fs.writeFileSync(path.join(root, child, `3-${name}.log`), marker + "\n");
      }
    }, dispose() { disposed++; } };
  } } };
  await assert.rejects(observeHostLogs(vscode, { pid: 4172, logDirectory: root }), /unique current invocation identity log/);
  assert.equal(disposed, 1);
}));
