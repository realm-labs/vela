"use strict";

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const extensionRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(extensionRoot, "..", "..");
const manifest = JSON.parse(fs.readFileSync(path.join(extensionRoot, "package.json"), "utf8"));
const isWindows = process.platform === "win32";

function usage() {
  console.log(`Usage: npm run package -- [options]

Options:
  --release             Build and bundle target/release/vela_lsp_server.
  --debug               Build and bundle target/debug/vela_lsp_server. This is the default.
  --skip-build          Reuse an existing vela_lsp_server binary.
  --no-bundle-server    Do not bundle a server binary. Users must set vela.server.path.
  --out <path>          VSIX output path. Defaults to dist/<name>-<version>-<platform>-<arch>.vsix.
  --help                Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    profile: "debug",
    skipBuild: false,
    bundleServer: true,
    out: undefined
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--release":
        options.profile = "release";
        break;
      case "--debug":
        options.profile = "debug";
        break;
      case "--skip-build":
        options.skipBuild = true;
        break;
      case "--no-bundle-server":
        options.bundleServer = false;
        break;
      case "--out":
        index += 1;
        if (index >= argv.length) {
          throw new Error("--out requires a path");
        }
        options.out = path.resolve(process.cwd(), argv[index]);
        break;
      case "--help":
      case "-h":
        usage();
        process.exit(0);
        break;
      default:
        throw new Error(`unknown option: ${arg}`);
    }
  }

  return options;
}

function run(command, args, cwd) {
  console.log(`$ ${[command, ...args].join(" ")}`);
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    shell: isWindows
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function cargoTargetDir() {
  const configured = process.env.CARGO_TARGET_DIR;
  if (!configured || configured.trim().length === 0) {
    return path.join(repoRoot, "target");
  }
  return path.isAbsolute(configured) ? configured : path.resolve(repoRoot, configured);
}

function serverBinaryPath(profile) {
  const executable = isWindows ? "vela_lsp_server.exe" : "vela_lsp_server";
  return path.join(cargoTargetDir(), profile, executable);
}

function ensureNodeDependencies() {
  const dependency = path.join(extensionRoot, "node_modules", "vscode-languageclient");
  if (fs.existsSync(dependency)) {
    return;
  }
  run("npm", ["install", "--no-package-lock"], extensionRoot);
}

function buildServer(profile) {
  const args = ["build", "-p", "vela_lsp_server"];
  if (profile === "release") {
    args.push("--release");
  }
  run("cargo", args, repoRoot);
}

function bundleServer(profile) {
  const source = serverBinaryPath(profile);
  if (!fs.existsSync(source)) {
    throw new Error(`server binary is missing: ${source}`);
  }

  const serverDir = path.join(extensionRoot, "server");
  const executable = isWindows ? "vela_lsp_server.exe" : "vela_lsp_server";
  const destination = path.join(serverDir, executable);

  fs.mkdirSync(serverDir, { recursive: true });
  fs.copyFileSync(source, destination);
  if (!isWindows) {
    fs.chmodSync(destination, 0o755);
  }

  console.log(`Bundled ${path.relative(repoRoot, source)} -> ${path.relative(repoRoot, destination)}`);
}

function removeBundledServer() {
  fs.rmSync(path.join(extensionRoot, "server"), { recursive: true, force: true });
}

function syncLicense() {
  const source = path.join(repoRoot, "LICENSE");
  if (!fs.existsSync(source)) {
    return;
  }
  fs.copyFileSync(source, path.join(extensionRoot, "LICENSE"));
}

function defaultOutputPath(options) {
  if (options.out) {
    return options.out;
  }

  const suffix = options.bundleServer
    ? `${process.platform}-${process.arch}-${options.profile}`
    : "no-server";
  return path.join(extensionRoot, "dist", `${manifest.name}-${manifest.version}-${suffix}.vsix`);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const out = defaultOutputPath(options);

  ensureNodeDependencies();

  if (options.bundleServer) {
    if (!options.skipBuild) {
      buildServer(options.profile);
    }
    bundleServer(options.profile);
  } else {
    removeBundledServer();
  }

  run("npm", ["run", "validate"], extensionRoot);
  syncLicense();

  fs.mkdirSync(path.dirname(out), { recursive: true });
  run(
    "npx",
    ["--yes", "@vscode/vsce", "package", "--allow-missing-repository", "--out", out],
    extensionRoot
  );

  console.log(`VSIX written to ${out}`);
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
