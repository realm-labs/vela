"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { assess } = require("./model");

function nodeResults(output) {
  const tests = new Map();
  for (const match of output.matchAll(/^(ok|not ok) \d+ - (.*?)(?: # (SKIP|TODO)\b[^\r\n]*)?\r?$/gm)) {
    const [, kind, name, skipped] = match;
    if (tests.has(name)) throw new Error(`duplicate infrastructure test identity ${name}`);
    tests.set(name, skipped ? "ignored" : kind === "ok" ? "ok" : "FAILED");
  }
  if (!tests.size) throw new Error("no infrastructure tests discovered");
  return tests;
}

function runInfrastructure(root, output, execute) {
  const directory = path.join(root, "scripts/lsp-matrix");
  const files = fs.readdirSync(directory).filter((file) => file.endsWith(".test.js")).sort();
  const args = ["--test", "--test-reporter=tap", ...(!execute ? ["--test-name-pattern=^$"] : []),
    ...files.map((file) => path.join(directory, file))];
  const run = spawnSync(process.execPath, args, { cwd: root, encoding: "utf8", timeout: 120000, maxBuffer: 16 * 1024 * 1024 });
  fs.writeFileSync(path.join(output, "infrastructure.log"), (run.stdout || "") + (run.stderr || ""));
  if (run.error) throw run.error;
  const results = nodeResults(run.stdout || "");
  return { available: [...results.keys()], results: execute ? results : new Map(), failed: run.status !== 0 };
}

function assessInfrastructure(requirements, evidence, run, other = { available: {}, results: {} }) {
  const gates = requirements.filter((item) => item.layer === "gate");
  const available = { ...other.available, gate: run.available };
  const keys = new Set();
  for (const proof of evidence) {
    if (keys.has(proof.requirement) || !gates.some((item) => item.id === proof.requirement) ||
        !proof.assertion?.trim() || !proof.tests?.length || proof.tests.some((test) => !Object.hasOwn(available, test.layer) || !test.name)) {
      throw new Error(`invalid infrastructure evidence ${proof.requirement}`);
    }
    keys.add(proof.requirement);
  }
  return assess(gates, evidence, available, { ...other.results, gate: run.results });
}

module.exports = { nodeResults, runInfrastructure, assessInfrastructure };
