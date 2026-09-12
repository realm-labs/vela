"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { selectProfile, validateProfiles, inputProfile } = require("./profiles");
const { localContracts } = require("./local-contracts");
const { loadInventory } = require("./inventory");
const { fileUri, canonicalUri, relativeFile } = require("../../editors/vscode/test/input/paths");
const root = path.resolve(__dirname, "../..");
const profiles = [
  { platform: "darwin", arch: "arm64", vscodeVersion: "1.137.0" },
  { platform: "win32", arch: "x64", vscodeVersion: "1.137.0" },
];

test("local profile selection pins platform, architecture and version without changing shared state", () => {
  const before = structuredClone(profiles);
  for (const profile of profiles) {
    assert.deepEqual(selectProfile(profiles, profile), profile);
    const full = inputProfile(root, profile);
    assert.equal(full.platform, profile.platform);
    assert.equal(full.arch, profile.arch);
    assert.equal(full.vscodeVersion, profile.vscodeVersion);
  }
  assert.deepEqual(profiles, before);
  for (const actual of [
    { ...profiles[1], arch: "arm64" }, { ...profiles[0], vscodeVersion: "1.90.0" },
    { platform: "linux", arch: "x64" },
  ]) {
    assert.throws(() => selectProfile(profiles, actual), /local execution profile/);
    assert.equal(selectProfile(profiles, actual, false), undefined);
  }
  assert.throws(() => validateProfiles([...profiles, profiles[0]]), /unique/);
  assert.throws(() => validateProfiles([]), /profiles/);
});

test("Windows input routes keep exact semantic oracles and platform-specific real input", () => {
  const requirements = loadInventory(root).executionRequirements;
  const fixture = require("../../tests/lsp_matrix/fixtures/input-driver.json");
  const mac = localContracts(requirements, fixture, "darwin");
  const windows = localContracts(requirements, fixture, "win32");
  assert.equal(windows.length, mac.length);
  for (let i = 0; i < mac.length; i++) {
    assert.deepEqual(windows[i].requirements, mac[i].requirements);
    const semantic = (c) => !["native-submenu", "visible-candidate"].includes(c.id);
    assert.deepEqual(windows[i].checks.filter(semantic), mac[i].checks.filter(semantic));
    assert.equal(windows[i].actions.length, mac[i].actions.length);
  }
  const actions = windows.flatMap((c) => c.actions);
  assert.deepEqual(windows[0].checks[1].expected, { ...mac[0].checks[1].expected, role: "listitem" });
  assert.equal(actions.find((a) => a.id === "back-key").key, "Alt+ArrowLeft");
  assert.equal(actions.find((a) => a.id === "declaration-open").key, "Control+Shift+P");
  assert.equal(actions.find((a) => a.id === "modifier-down").key, "Control");
  assert.equal(actions.find((a) => a.id === "dirty-home").key, "Control+Home");
  assert.equal(actions.find((a) => a.id === "context-menu").button, "right");
  assert.equal(actions.find((a) => a.id === "peek-definition").device, "pointer");
  assert.deepEqual(mac, localContracts(requirements, fixture, "darwin"));
  assert.throws(() => localContracts(requirements, fixture, "linux"), /unsupported/);
});

test("editor file identity preserves encoded Unicode and fixture paths on the current OS", () => {
  const file = path.join(root, "中文 % workspace", "scripts", "a😀.vela");
  const uri = fileUri(file);
  assert.equal(canonicalUri(uri), uri);
  assert.equal(relativeFile(root, file), "中文 % workspace/scripts/a😀.vela");
  assert.match(uri, /%25/);
  assert.match(uri, /%F0%9F%98%80/);
  assert.throws(() => canonicalUri(`${uri}?unexpected`), /plain file URI/);
  assert.throws(() => canonicalUri(`${uri}#unexpected`), /plain file URI/);
  assert.throws(() => relativeFile(root, path.resolve(root, "..", "outside.vela")), /invalid fixture/);
  if (process.platform === "win32") {
    assert.equal(canonicalUri(uri.replace(/^file:\/\/\/([a-z]):/, (_, drive) => `file:///${drive.toUpperCase()}%3A`)), uri);
  }
});
