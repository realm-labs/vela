"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const {
  artifact,
  fileHash,
  jsonHash,
  validateBundle,
  inputHash,
  proofStatus,
} = require("./local-evidence");
const { localContracts } = require("./local-contracts");
const fixture = require("../../tests/lsp_matrix/fixtures/input-driver.json");

function setup(action) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-local-proof-"));
  try {
    const requirements = [
      { id: "batch/B01/input-render-driver", contractHash: "c".repeat(64) },
    ];
    const contracts = localContracts(requirements, fixture),
      contract = contracts[0];
    const files = [
      ...contract.artifacts,
      "vela.vsix",
      "extensions/vela/server/vela_lsp_server",
    ];
    for (const file of files) {
      fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
      fs.writeFileSync(path.join(root, file), file);
    }
    const inputs = {
      source: { revision: "a".repeat(40), treeSha256: "b".repeat(64) },
      driverSha256: "d".repeat(64),
      fixturesSha256: "e".repeat(64),
      packagingSha256: "f".repeat(64),
      profileSha256: "1".repeat(64),
      serverSha256: fileHash(path.join(root, files.at(-1))),
    };
    const profile = {
      platform: "darwin",
      arch: "arm64",
      vscodeVersion: "1.137.0",
      locale: "en",
      keyboardLayout: "ABC",
      theme: "Dark Modern",
      zoomLevel: 0,
      display: { width: 1440, height: 900, deviceScaleFactor: 2 },
    };
    const bundle = {
      version: 2,
      observedDisplay: profile.display,
      status: "passed",
      exit: { code: 0, signal: null },
      inputs,
      profile,
      vsix: "vela.vsix",
      installedServer: files.at(-1),
      vsixSha256: fileHash(path.join(root, "vela.vsix")),
      artifacts: files.map((file) => artifact(root, file)),
      proofs: [
        {
          id: contract.id,
          fixture: contract.fixture,
          contractHash: jsonHash(contract),
          status: "passed",
          durationMs: 50,
          startedAt: "2026-09-12T00:00:00.000Z",
          finishedAt: "2026-09-12T00:00:00.050Z",
          actions: structuredClone(contract.actions),
          checks: contract.checks.map((check) => ({
            ...check,
            status: "passed",
            observed: structuredClone(check.expected),
          })),
        },
      ],
    };
    const trace = [
      ...contract.actions.map((action) => ({
        kind: "input",
        proof: contract.id,
        at: new Date().toISOString(),
        ...action,
      })),
      ...contract.checks.map((check) => ({
        kind: "assertion",
        proof: contract.id,
        id: check.id,
        observed: check.expected,
      })),
    ];
    fs.writeFileSync(path.join(root, "trace.json"), JSON.stringify(trace));
    bundle.artifacts = bundle.artifacts.map((item) =>
      artifact(root, item.path),
    );
    action({
      root,
      bundle,
      contracts,
      expected: structuredClone({ inputs, profile }),
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test("local input proof validates exact actions rendered outcome final document and artifact bytes", () =>
  setup(({ root, bundle, contracts, expected }) => {
    assert.deepEqual(validateBundle(bundle, expected, contracts, root), [
      {
        id: "batch/B01/input-render-driver",
        contractHash: "c".repeat(64),
        status: "verified",
      },
    ]);
  }));
test("local input proof rejects stale source driver fixture packaging server and exact profile", () =>
  setup(({ root, bundle, contracts, expected }) => {
    for (const key of Object.keys(bundle.inputs)) {
      const changed = structuredClone(bundle);
      changed.inputs[key] = null;
      assert.throws(
        () => validateBundle(changed, expected, contracts, root),
        /stale/,
      );
    }
    for (const key of Object.keys(bundle.profile)) {
      const changed = structuredClone(bundle);
      changed.profile[key] = "changed";
      assert.throws(
        () => validateBundle(changed, expected, contracts, root),
        /exact profile/,
      );
    }
  }));
test("local input proof rejects skipped actions wrong devices missing or downgraded checks", () =>
  setup(({ root, bundle, contracts, expected }) => {
    for (const mutate of [
      (b) => b.proofs[0].actions.pop(),
      (b) => (b.proofs[0].actions[0].device = "command"),
      (b) => b.proofs[0].checks.pop(),
      (b) => (b.proofs[0].checks[0].level = "Provider"),
      (b) => (b.proofs[0].checks[0].status = "skipped"),
      (b) => (b.proofs[0].checks[2].observed.text = "incorrect edit"),
      (b) => (b.proofs[0].checks[1].expected.label = "invented"),
      (b) => (b.proofs[0].durationMs = 45001),
      (b) => (b.proofs[0].status = "skipped"),
    ]) {
      const changed = structuredClone(bundle);
      mutate(changed);
      assert.throws(() => validateBundle(changed, expected, contracts, root));
    }
  }));
test("local input proof rejects duplicate unknown stale routes and dirty process exits", () =>
  setup(({ root, bundle, contracts, expected }) => {
    for (const mutate of [
      (b) => b.proofs.push(b.proofs[0]),
      (b) => (b.proofs[0].id = "UX99"),
      (b) => (b.proofs[0].fixture = "wrong-fixture"),
      (b) => (b.proofs[0].finishedAt = "missing"),
      (b) => (b.proofs[0].contractHash = "outdated"),
      (b) => b.proofs[0].checks.push(b.proofs[0].checks[0]),
      (b) => (b.exit.signal = "SIGKILL"),
      (b) => (b.exit.code = 1),
      (b) => (b.status = "failed"),
      (b) => (b.proofs = []),
    ]) {
      const changed = structuredClone(bundle);
      mutate(changed);
      assert.throws(() => validateBundle(changed, expected, contracts, root));
    }
  }));
test("local input proof rejects missing changed duplicate escaping and mismatched VSIX artifacts", () =>
  setup(({ root, bundle, contracts, expected }) => {
    for (const mutate of [
      (b) => b.artifacts.shift(),
      (b) => b.artifacts.push(b.artifacts[0]),
      (b) => (b.artifacts[0].path = "../trace.json"),
      (b) => (b.vsixSha256 = "0".repeat(64)),
      (b) => (b.installedServer = "vela.vsix"),
    ]) {
      const changed = structuredClone(bundle);
      mutate(changed);
      assert.throws(() => validateBundle(changed, expected, contracts, root));
    }
    fs.writeFileSync(path.join(root, "suggestions.png"), "changed screenshot");
    assert.throws(
      () => validateBundle(bundle, expected, contracts, root),
      /artifact changed/,
    );
    fs.symlinkSync(
      path.join(root, ".."),
      path.join(root, "escape"),
      "junction",
    );
    assert.throws(() => artifact(root, "escape"), /escapes/);
  }));
test("local input identity hashes include driver fixture and packaging file changes", () =>
  setup(({ root }) => {
    const before = inputHash(root, ["extensions"]);
    fs.writeFileSync(path.join(root, "extensions/manifest.json"), "candidate");
    assert.notEqual(inputHash(root, ["extensions"]), before);
    fs.symlinkSync(root, path.join(root, "extensions/link"), "junction");
    assert.throws(() => inputHash(root, ["extensions"]), /symlink/);
  }));

test("driver self-tests alone cannot certify actual input or replace missing interaction routes", () => {
  const driver = {
    id: "batch/B01/input-render-driver",
    contractHash: "current",
    layer: "gate",
  };
  const tests = [{ ...driver, status: "verified" }];
  assert.equal(proofStatus(driver, tests, []), "unreviewed");
  assert.equal(
    proofStatus(driver, tests, [
      { ...driver, status: "verified", contractHash: "old" },
    ]),
    "unreviewed",
  );
  const actual = [{ ...driver, status: "verified" }];
  assert.equal(proofStatus(driver, tests, actual), "verified");
  assert.equal(proofStatus(driver, [], actual), "unreviewed");
  assert.equal(
    proofStatus(driver, [{ ...driver, status: "failed" }], actual),
    "failed",
  );
  const route = {
    id: "vscode/UX02/keyboard-definition/input/local",
    contractHash: "route",
    layer: "interaction",
  };
  assert.equal(proofStatus(route, [], actual), "unreviewed");
  assert.equal(
    proofStatus(route, [], [{ ...route, status: "verified" }]),
    "verified",
  );
});

test("local proof rejects trace omissions even when artifact hashes are recomputed", () =>
  setup(({ root, bundle, contracts, expected }) => {
    const trace = JSON.parse(
      fs.readFileSync(path.join(root, "trace.json"), "utf8"),
    );
    trace.splice(0, 1);
    fs.writeFileSync(path.join(root, "trace.json"), JSON.stringify(trace));
    bundle.artifacts = bundle.artifacts.map((item) =>
      artifact(root, item.path),
    );
    assert.throws(
      () => validateBundle(bundle, expected, contracts, root),
      /trace does not match/,
    );
  }));

test("local proof rejects command receipts relabeled as physical keyboard input", () =>
  setup(({ root, bundle, contracts, expected }) => {
    const trace = JSON.parse(fs.readFileSync(path.join(root, "trace.json"), "utf8"));
    trace[0].kind = "command";
    fs.writeFileSync(path.join(root, "trace.json"), JSON.stringify(trace));
    bundle.artifacts = bundle.artifacts.map((item) => artifact(root, item.path));
    assert.throws(() => validateBundle(bundle, expected, contracts, root), /cannot be interchanged/);
  }));
