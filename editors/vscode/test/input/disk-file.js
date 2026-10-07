"use strict";
const fs = require("node:fs");

function readFileIfPresent(file, read = fs.readFileSync) {
  try { return read(file, "utf8"); }
  catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

module.exports = { readFileIfPresent };
