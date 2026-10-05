"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const fs = require("node:fs"), path = require("node:path"), os = require("node:os");
const { hostIdentity, hostLogDirectory } = require("../../editors/vscode/test/input/host-identity");
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
