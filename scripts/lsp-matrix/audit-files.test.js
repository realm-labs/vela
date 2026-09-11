"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { createAuditDirectory, writeReports } = require("./audit-files");

test("later audits preserve earlier failure reports and logs", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-audit-artifacts-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const first = createAuditDirectory(root);
  fs.writeFileSync(path.join(first, "protocol-run.log"), "failed assertion");
  writeReports(root, first, { failed: true }, "failed");
  const second = createAuditDirectory(root);
  writeReports(root, second, { failed: false }, "passed");
  assert.notEqual(first, second);
  assert.equal(fs.readFileSync(path.join(first, "protocol-run.log"), "utf8"), "failed assertion");
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(first, "report.json"))), { failed: true });
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(root, "report.json"))), { failed: false });
});
