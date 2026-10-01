"use strict";
const model = require("./model");
const suites = [
  { layer: "service", log: "service", crate: "vela_language_service", target: ["--lib"] },
  { layer: "protocol", log: "protocol", crate: "vela_lsp_server", target: ["--lib"] },
  // Integration tests build a test-profile executable. Keep that executable
  // away from the dev-profile binary fingerprinted by the installed VSIX.
  { layer: "protocol", log: "stdio", crate: "vela_lsp_server", target: ["--test", "stdio_transport", "--target-dir", "target/lsp-matrix-stdio"] }
];
function appendSuite(suite, listed, executed, available, results) {
  const names = model.testNames(listed);
  if (!names.length) throw new Error(`${suite.log}: no tests discovered`);
  const previous = available[suite.layer] || [];
  if (new Set([...previous, ...names]).size !== previous.length + names.length) throw new Error(`duplicate ${suite.layer} test identity across suites`);
  available[suite.layer] = [...previous, ...names];
  if (executed !== undefined) {
    const current = model.testResults(executed);
    if ([...current.keys()].some((name) => !names.includes(name))) throw new Error(`${suite.log}: undiscovered result`);
    results[suite.layer] = new Map([...(results[suite.layer] || []), ...current]);
  }
}
module.exports = { suites, appendSuite };
