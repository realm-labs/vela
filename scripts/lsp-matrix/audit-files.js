"use strict";

const fs = require("node:fs");
const path = require("node:path");

function createAuditDirectory(reportRoot) {
  fs.mkdirSync(reportRoot, { recursive: true });
  return fs.mkdtempSync(path.join(reportRoot, "run-"));
}

function writeReports(reportRoot, runRoot, report, markdown) {
  fs.writeFileSync(path.join(runRoot, "report.json"), JSON.stringify(report, null, 2) + "\n");
  fs.writeFileSync(path.join(runRoot, "report.md"), markdown);
  for (const file of ["report.json", "report.md"]) {
    fs.copyFileSync(path.join(runRoot, file), path.join(reportRoot, file));
  }
}

module.exports = { createAuditDirectory, writeReports };
