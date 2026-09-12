"use strict";
const fs = require("node:fs");
const path = require("node:path");

function profileKey(profile) {
  return `${profile.platform}-${profile.arch}`;
}

function validateProfiles(profiles) {
  if (!Array.isArray(profiles) || !profiles.length ||
      profiles.some((p) => !["darwin", "win32"].includes(p.platform) ||
        !["arm64", "x64"].includes(p.arch) || !/^\d+\.\d+\.\d+$/.test(p.vscodeVersion)) ||
      new Set(profiles.map(profileKey)).size !== profiles.length) {
    throw Error("checkpoint requires unique exact local profiles");
  }
}

function selectProfile(profiles, actual = process, required = true) {
  validateProfiles(profiles);
  const profile = profiles.find((p) => profileKey(p) === profileKey(actual) &&
    (actual.vscodeVersion === undefined || actual.vscodeVersion === p.vscodeVersion));
  if (!profile && required) throw Error("evidence does not match a registered local execution profile");
  return profile;
}

function inputProfile(root, actual = process) {
  const checkpoint = JSON.parse(fs.readFileSync(path.join(root, "tests/lsp_matrix/checkpoint.json"), "utf8"));
  const selected = selectProfile(checkpoint.profiles, actual);
  const profile = JSON.parse(fs.readFileSync(path.join(root,
    `editors/vscode/test/input/profiles/${profileKey(selected)}.json`), "utf8"));
  if (profile.platform !== selected.platform || profile.arch !== selected.arch ||
      profile.vscodeVersion !== selected.vscodeVersion) throw Error("input profile differs from checkpoint registration");
  return profile;
}

module.exports = { profileKey, validateProfiles, selectProfile, inputProfile };
