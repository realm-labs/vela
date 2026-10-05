"use strict";
const assert = require("node:assert/strict");
const special = new Set(["\\", "^", "$", ".", "*", "+", "?", "(", ")", "[", "]", "{", "}", "|"]);
const escaped = text => [...text].map(c => special.has(c) ? "\\" + c : c).join("");

function quickOpenFileLabels(file) {
  assert.equal(typeof file, "string");
  const parts = file.split("/");
  assert(parts.length > 1 && parts.every(p => p && p !== "." && p !== ".." && !p.includes("\\") && !p.includes(":")),
    "installation Quick Open requires an owned relative file path");
  const name = parts.pop();
  return { name: new RegExp("^" + escaped(name) + "$"),
    directory: new RegExp("^" + parts.map(escaped).join("[/\\\\]") + "$") };
}
module.exports = { quickOpenFileLabels };
