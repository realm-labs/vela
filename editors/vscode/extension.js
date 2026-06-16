"use strict";

const path = require("path");
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

function config() {
  return vscode.workspace.getConfiguration("vela");
}

function configuredRoots() {
  return config().get("workspace.roots", []);
}

function configuredSchema() {
  return config().get("host.schema", "");
}

function serverCommand(context) {
  const configured = config().get("server.path", "");
  if (configured.trim().length > 0) {
    return configured;
  }
  const executable = process.platform === "win32" ? "vela_lsp_server.exe" : "vela_lsp_server";
  return context.asAbsolutePath(path.join("server", executable));
}

function serverArgs() {
  const args = [...config().get("server.args", ["--stdio"])];
  for (const root of configuredRoots()) {
    args.push("--root", root);
  }
  const schema = configuredSchema();
  if (schema.trim().length > 0) {
    args.push("--schema", schema);
  }
  return args;
}

function initializationOptions() {
  const roots = configuredRoots();
  const schema = configuredSchema();
  return {
    workspace: {
      roots
    },
    host: {
      schema
    }
  };
}

function workspaceFolderPath() {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

function activate(context) {
  const serverOptions = {
    command: serverCommand(context),
    args: serverArgs(),
    options: {
      cwd: workspaceFolderPath()
    }
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "vela" }],
    initializationOptions: initializationOptions(),
    synchronize: {
      configurationSection: "vela",
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.vela"),
        vscode.workspace.createFileSystemWatcher("**/vela.toml")
      ]
    }
  };

  client = new LanguageClient("vela", "Vela Language Server", serverOptions, clientOptions);
  context.subscriptions.push(client.start());
}

function deactivate() {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

module.exports = {
  activate,
  deactivate
};
