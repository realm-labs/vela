"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { sourceIdentity } = require("./source-identity");
const { safeFile } = require("./fixtures");

const hash = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const fileHash = (file) => hash(fs.readFileSync(file));
const jsonHash = (value) => hash(JSON.stringify(value));

function inputHash(root, inputs) {
  const files = [];
  function collect(file) {
    const stat = fs.lstatSync(path.join(root, file));
    if (stat.isSymbolicLink())
      throw new Error(`input symlink is not supported: ${file}`);
    if (stat.isDirectory()) {
      for (const entry of fs.readdirSync(path.join(root, file)).sort())
        collect(`${file}/${entry}`);
    } else if (stat.isFile()) files.push(file);
  }
  for (const input of inputs) collect(input);
  const digest = crypto.createHash("sha256");
  for (const file of files.sort())
    digest.update(file + "\0").update(fs.readFileSync(path.join(root, file)));
  return digest.digest("hex");
}

function currentInputs(root, binary, profile) {
  return {
    source: sourceIdentity(root),
    profileSha256: jsonHash(profile),
    driverSha256: inputHash(root, [
      "editors/vscode/test/input",
      "scripts/lsp-matrix",
    ]),
    fixturesSha256: inputHash(root, [
      "tests/lsp_matrix/fixtures",
      "scripts/lsp-matrix/fixtures.js",
    ]),
    packagingSha256: inputHash(root, [
      "editors/vscode/scripts",
      "editors/vscode/extension.js",
      "editors/vscode/package.json",
      "editors/vscode/package-lock.json",
      "editors/vscode/language-configuration.json",
      "editors/vscode/syntaxes",
      "editors/vscode/.vscodeignore",
    ]),
    serverSha256: fileHash(binary),
  };
}

function artifactPath(root, relative) {
  safeFile(relative);
  const file = path.join(root, relative);
  const physical = fs.realpathSync(file);
  const prefix = fs.realpathSync(root) + path.sep;
  if (!physical.startsWith(prefix) || !fs.statSync(physical).isFile())
    throw new Error(`artifact escapes run directory: ${relative}`);
  return file;
}

function artifact(root, relative) {
  return { path: relative, sha256: fileHash(artifactPath(root, relative)) };
}

function validateBundle(bundle, expected, contracts, root) {
  if (
    bundle.version !== 2 ||
    bundle.status !== "passed" ||
    bundle.exit?.code !== 0 ||
    bundle.exit?.signal !== null
  ) {
    throw new Error(
      "local evidence requires a passing run and clean process exit",
    );
  }
  assert.deepEqual(
    bundle.inputs,
    expected.inputs,
    "local evidence is stale: source/driver/fixture/package/server/profile changed",
  );
  assert.deepEqual(
    bundle.profile,
    expected.profile,
    "local evidence has a different exact profile",
  );
  assert.deepEqual(
    bundle.observedDisplay,
    expected.profile.display,
    "local display observation differs from the pinned profile",
  );
  if (
    !Array.isArray(bundle.proofs) ||
    !bundle.proofs.length ||
    !Array.isArray(bundle.artifacts)
  )
    throw new Error("missing local proofs or artifacts");
  const artifacts = new Map();
  for (const item of bundle.artifacts) {
    if (artifacts.has(item.path) || !/^[a-f0-9]{64}$/.test(item.sha256))
      throw new Error("duplicate or invalid artifact hash");
    if (fileHash(artifactPath(root, item.path)) !== item.sha256)
      throw new Error(`artifact changed: ${item.path}`);
    artifacts.set(item.path, item);
  }
  if (
    !artifacts.has(bundle.vsix) ||
    !artifacts.has(bundle.installedServer) ||
    artifacts.get(bundle.installedServer).sha256 !==
      expected.inputs.serverSha256
  ) {
    throw new Error("missing VSIX or different installed server");
  }
  // The installer records the actual archive bytes; replacing that artifact
  // cannot preserve its recorded identity.
  if (bundle.vsixSha256 !== artifacts.get(bundle.vsix).sha256)
    throw new Error("VSIX identity mismatch");
  const seen = new Set(),
    verified = new Map();
  const trace = JSON.parse(
    fs.readFileSync(artifactPath(root, "trace.json"), "utf8"),
  );
  if (!Array.isArray(trace) || trace.some((item) => item.kind === "failure"))
    throw new Error("invalid or failed action trace");
  for (const proof of bundle.proofs) {
    const contract = contracts.find((item) => item.id === proof.id);
    if (
      !contract ||
      seen.has(proof.id) ||
      proof.fixture !== contract.fixture ||
      proof.contractHash !== jsonHash(contract)
    )
      throw new Error("unknown duplicate or stale local proof");
    seen.add(proof.id);
    if (
      proof.status !== "passed" ||
      !Number.isFinite(proof.durationMs) ||
      proof.durationMs < 0 ||
      proof.durationMs > contract.deadlineMs ||
      !Number.isFinite(Date.parse(proof.startedAt)) ||
      Date.parse(proof.finishedAt) - Date.parse(proof.startedAt) !==
        proof.durationMs
    ) {
      throw new Error(`${proof.id}: failed skipped or unbounded proof`);
    }
    assert.deepEqual(
      proof.actions,
      contract.actions,
      `${proof.id}: required input actions were changed or skipped`,
    );
    assert.deepEqual(
      trace
        .filter((item) => item.kind === "input" && item.proof === proof.id)
        .map(({ kind, at, proof: owner, ...action }) => action),
      proof.actions,
      "action trace does not match proof receipts",
    );
    const checks = new Map();
    for (const check of proof.checks ?? []) {
      if (checks.has(check.id)) throw new Error("duplicate local assertion");
      checks.set(check.id, check);
    }
    assert.deepEqual(
      [...checks.keys()].sort(),
      contract.checks.map((item) => item.id).sort(),
      "missing or unknown local assertion",
    );
    for (const required of contract.checks) {
      const check = checks.get(required.id);
      if (check.status !== "passed" || check.level !== required.level)
        throw new Error("failed skipped or downgraded local assertion");
      assert.deepEqual(
        check.expected,
        required.expected,
        "local expectation differs from independent contract",
      );
      assert.deepEqual(
        check.observed,
        required.expected,
        "local observation differs from independent contract",
      );
      const observed = trace.filter(
        (item) =>
          item.kind === "assertion" &&
          item.proof === proof.id &&
          item.id === check.id,
      );
      if (observed.length !== 1)
        throw new Error("assertion trace is missing or duplicated");
      assert.deepEqual(
        observed[0].observed,
        check.observed,
        "assertion trace differs from the submitted observation",
      );
    }
    for (const required of contract.artifacts)
      if (!artifacts.has(required))
        throw new Error(`missing required local artifact: ${required}`);
    for (const requirement of contract.requirements) {
      if (verified.has(requirement.id))
        throw new Error("duplicate local requirement proof");
      verified.set(requirement.id, { ...requirement, status: "verified" });
    }
  }
  return [...verified.values()];
}

function proofStatus(requirement, assessed, localProofs) {
  const base =
    assessed.find((item) => item.id === requirement.id)?.status ?? "unreviewed";
  if (
    requirement.id !== "batch/B01/input-render-driver" &&
    requirement.layer !== "interaction"
  )
    return base;
  const live = localProofs.find((item) => item.id === requirement.id);
  if (base === "failed") return base;
  if (
    live?.status !== "verified" ||
    live.contractHash !== requirement.contractHash
  )
    return "unreviewed";
  return requirement.layer === "interaction" || base === "verified"
    ? "verified"
    : base;
}

module.exports = {
  hash,
  fileHash,
  jsonHash,
  inputHash,
  currentInputs,
  artifactPath,
  artifact,
  validateBundle,
  proofStatus,
};
