"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path");
function assertObserverHealthy(root) {
  assert(!fs.existsSync(path.join(root, "bridge-failure.log")), "installed observer failed; inspect retained bridge-failure.log");
}
module.exports = { assertObserverHealthy };
