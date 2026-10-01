"use strict";
const fs = require("node:fs"), path = require("node:path");
function writeProfileSettings(workspace, userData, profile) {
  // VS Code declares window.dialogStyle as APPLICATION scope. Workspace
  // settings are ignored for it; use only this run's private user profile.
  const { "window.dialogStyle": dialogStyle, ...workspaceSettings } = profile.settings;
  fs.mkdirSync(path.join(workspace, ".vscode"), { recursive: true });
  fs.writeFileSync(path.join(workspace, ".vscode/settings.json"), JSON.stringify(workspaceSettings, null, 2));
  fs.mkdirSync(path.join(userData, "User"), { recursive: true });
  fs.writeFileSync(path.join(userData, "User/settings.json"), JSON.stringify({ "window.dialogStyle": dialogStyle }, null, 2));
}
module.exports = { writeProfileSettings };
