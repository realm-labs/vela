"use strict";

const os = require("os");
const path = require("path");
const vscode = require("vscode");

let client;
let outputChannel;
let traceOutputChannel;

function log(message) {
  outputChannel?.appendLine(`[${new Date().toISOString()}] ${message}`);
}

function config() {
  return vscode.workspace.getConfiguration("vela");
}

function serverEnabled() {
  return config().get("server.enabled", true);
}

function configuredRoots() {
  return config().get("workspace.roots", []);
}

function configuredSchema() {
  return config().get("host.schema", "");
}

function profileEnabled() {
  return config().get("server.profile.enabled", false);
}

function profileSlowMs() {
  const configured = Number(config().get("server.profile.slowMs", 50));
  return Number.isFinite(configured) && configured >= 0 ? Math.floor(configured) : 50;
}

function profilePath(cwd) {
  const configured = config().get("server.profile.path", "");
  if (configured.trim().length > 0) {
    return configured;
  }
  if (cwd) {
    return path.join(cwd, ".vela-lsp-profile.jsonl");
  }
  return path.join(os.tmpdir(), "vela-lsp-profile.jsonl");
}

function traceServer() {
  return config().get("trace.server", "off");
}

function serverTraceEnabled() {
  return traceServer() !== "off";
}

function serverLogPath(cwd) {
  if (cwd) {
    return path.join(cwd, ".vela-lsp-trace.jsonl");
  }
  return path.join(os.tmpdir(), "vela-lsp-trace.jsonl");
}

function watchFilesEnabled() {
  return config().get("server.watchFiles.enabled", true);
}

function serverCommand(context) {
  const configured = config().get("server.path", "");
  if (configured.trim().length > 0) {
    return configured;
  }
  const executable = process.platform === "win32" ? "vela_lsp_server.exe" : "vela_lsp_server";
  return context.asAbsolutePath(path.join("server", executable));
}

function serverArgs(cwd) {
  const args = [...config().get("server.args", ["--stdio"])];
  for (const root of configuredRoots()) {
    args.push("--root", root);
  }
  const schema = configuredSchema();
  if (schema.trim().length > 0) {
    args.push("--schema", schema);
  }
  if (profileEnabled()) {
    args.push("--profile", profilePath(cwd));
    args.push("--profile-slow-ms", String(profileSlowMs()));
  }
  if (serverTraceEnabled()) {
    args.push("--log", serverLogPath(cwd));
  }
  if (!watchFilesEnabled()) {
    args.push("--no-watch-files");
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
  outputChannel = vscode.window.createOutputChannel("Vela");
  context.subscriptions.push(outputChannel);
  context.subscriptions.push(vscode.commands.registerCommand("vela.showOutput", () => {
    outputChannel.show(true);
  }));
  log("Activating Vela extension.");

  if (!serverEnabled()) {
    log("Native language server is disabled by vela.server.enabled=false.");
    return;
  }

  const {
    CloseAction,
    ErrorAction,
    LanguageClient,
    RevealOutputChannelOn
  } = require("vscode-languageclient/node");
  traceOutputChannel = vscode.window.createOutputChannel("Vela LSP Trace");
  context.subscriptions.push(traceOutputChannel);

  const command = serverCommand(context);
  const cwd = workspaceFolderPath();
  const args = serverArgs(cwd);
  log(`Starting native language server: ${command} ${args.join(" ")}`);
  log(`Language server cwd: ${cwd ?? "<none>"}`);
  if (profileEnabled()) {
    log(`Language server profile: ${profilePath(cwd)}`);
  }
  if (serverTraceEnabled()) {
    log(`Language server trace log: ${serverLogPath(cwd)}`);
  }
  if (!watchFilesEnabled()) {
    log("Language server watched-file registration is disabled.");
  }

  const serverOptions = {
    command,
    args,
    options: cwd ? { cwd } : undefined
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "vela" }],
    initializationOptions: initializationOptions(),
    outputChannel,
    traceOutputChannel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    initializationFailedHandler: (error) => {
      log(`Language server initialization failed: ${error?.message ?? String(error)}`);
      return false;
    },
    errorHandler: {
      error: (error, _message, count) => {
        log(`Language server connection error${count ? ` #${count}` : ""}: ${error.message}`);
        return { action: ErrorAction.Shutdown, handled: true };
      },
      closed: () => {
        log("Language server connection closed; not restarting automatically.");
        return { action: CloseAction.DoNotRestart, handled: true };
      }
    },
    connectionOptions: {
      maxRestartCount: 0
    },
    synchronize: {
      configurationSection: "vela"
    }
  };

  client = new LanguageClient("vela", "Vela Language Server", serverOptions, clientOptions);
  context.subscriptions.push({ dispose: () => { void stopClient(); } });
  client.start().then(
    () => log("Language server started."),
    (error) => log(`Language server start failed: ${error?.message ?? String(error)}`)
  );
}

function stopClient() {
  if (!client) {
    return undefined;
  }
  const activeClient = client;
  client = undefined;
  log("Stopping language server.");
  return activeClient.stop().then(
    () => log("Language server stopped."),
    (error) => log(`Language server stop failed: ${error?.message ?? String(error)}`)
  );
}

function deactivate() {
  return stopClient();
}

module.exports = {
  activate,
  deactivate
};
