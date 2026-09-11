"use strict";
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { safeFile } = require("./fixtures");
const { hash, artifactPath } = require("./local-evidence");

function manifest(bytes) {
  const value = JSON.parse(bytes.toString());
  delete value.__metadata; // VS Code adds install location/version metadata.
  return value;
}

function comparePackageEntries(entries, current, installed, serverSha256) {
  const seen = new Set();
  for (const { name, bytes } of entries) {
    safeFile(name);
    if (seen.has(name)) throw new Error(`duplicate VSIX entry ${name}`);
    seen.add(name);
    if (name === "package.json") {
      assert.deepEqual(
        manifest(bytes),
        manifest(current(name)),
        "VSIX manifest differs from candidate",
      );
      assert.deepEqual(
        manifest(bytes),
        manifest(installed(name)),
        "installed manifest differs from VSIX",
      );
    } else {
      assert.equal(
        hash(installed(name)),
        hash(bytes),
        `installed file differs from VSIX: ${name}`,
      );
      if (
        ["extension.js", "language-configuration.json"].includes(name) ||
        name.startsWith("syntaxes/")
      ) {
        assert.equal(
          hash(current(name)),
          hash(bytes),
          `VSIX payload differs from candidate: ${name}`,
        );
      }
      if (name.startsWith("server/"))
        assert.equal(
          hash(bytes),
          serverSha256,
          "VSIX server differs from candidate",
        );
    }
  }
  for (const required of [
    "package.json",
    "extension.js",
    "language-configuration.json",
    "syntaxes/vela.tmLanguage.json",
  ]) {
    if (!seen.has(required)) throw new Error(`VSIX is missing ${required}`);
  }
  if (![...seen].some((name) => name.startsWith("server/")))
    throw new Error("VSIX is missing its server");
}

async function verifyInstalledPackage(
  vsix,
  installedRoot,
  extensionRoot,
  serverSha256,
) {
  const yauzl = require(path.join(extensionRoot, "node_modules/yauzl"));
  const entries = await new Promise((resolve, reject) => {
    yauzl.open(vsix, { lazyEntries: true }, (error, zip) => {
      if (error) return reject(error);
      const entries = [];
      let total = 0;
      const fail = (error) => {
        zip.close();
        reject(error);
      };
      zip.on("error", fail);
      zip.on("end", () => resolve(entries));
      zip.on("entry", (entry) => {
        if (
          entry.fileName.endsWith("/") ||
          !entry.fileName.startsWith("extension/")
        ) {
          zip.readEntry();
          return;
        }
        if (
          ((entry.externalFileAttributes >>> 16) & 0o170000) === 0o120000 ||
          entry.uncompressedSize > 128 * 1024 * 1024 ||
          entries.length >= 4096
        )
          return fail(new Error("unsupported or oversized VSIX entry"));
        const name = entry.fileName.slice("extension/".length);
        try {
          safeFile(name);
        } catch (error) {
          fail(error);
          return;
        }
        zip.openReadStream(entry, (error, stream) => {
          if (error) return fail(error);
          const chunks = [];
          stream.on("error", fail);
          stream.on("data", (chunk) => {
            total += chunk.length;
            if (total > 256 * 1024 * 1024) {
              stream.destroy();
              fail(new Error("VSIX exceeds evidence byte budget"));
              return;
            }
            chunks.push(chunk);
          });
          stream.on("end", () => {
            entries.push({ name, bytes: Buffer.concat(chunks) });
            zip.readEntry();
          });
        });
      });
      zip.readEntry();
    });
  });
  comparePackageEntries(
    entries,
    (name) => fs.readFileSync(artifactPath(extensionRoot, name)),
    (name) => fs.readFileSync(artifactPath(installedRoot, name)),
    serverSha256,
  );
}
module.exports = { comparePackageEntries, verifyInstalledPackage };
