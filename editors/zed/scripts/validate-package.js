"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function hasTomlValue(text, key, value) {
  return text.split(/\r?\n/).some((line) => line.trim() === `${key} = ${value}`);
}

const manifest = read("extension.toml");
assert(hasTomlValue(manifest, "id", '"vela"'), "extension id must be vela");
assert(hasTomlValue(manifest, "schema_version", "1"), "schema_version must be 1");
assert(
  manifest.includes("[language_servers.vela-language-server]"),
  "language server section is missing"
);
assert(manifest.includes("[languages.Vela]"), "Vela language section is missing");

const languageConfigPath = "languages/vela/config.toml";
assert(fs.existsSync(path.join(root, languageConfigPath)), "language config is missing");
const languageConfig = read(languageConfigPath);
assert(hasTomlValue(languageConfig, "name", '"Vela"'), "language name is missing");
assert(hasTomlValue(languageConfig, "path_suffixes", '["vela"]'), "path suffix is missing");
assert(
  languageConfig.includes('language_servers = ["vela-language-server"]') ||
    manifest.includes('language_servers = ["vela-language-server"]'),
  "Vela language must reference the native language server"
);

const extensionRs = read("src/extension.rs");
assert(
  extensionRs.includes("vela_lsp_server") && extensionRs.includes("--stdio"),
  "extension launcher must start vela_lsp_server over stdio"
);
assert(
  !extensionRs.includes("textDocument/") && !extensionRs.includes("workspace/"),
  "editor package must not implement LSP request behavior"
);

console.log("Zed extension package metadata is valid.");
