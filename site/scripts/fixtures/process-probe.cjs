"use strict";

const { spawn } = require("node:child_process");
const mode = process.argv[2];

if (mode === "status") {
  process.stdout.write("out\0🙂\n");
  process.stderr.write("err\n");
  process.exitCode = Number(process.argv[3]);
} else {
  // Even a broken supervisor cannot leave these probes running indefinitely.
  setTimeout(() => process.exit(90), 8_000);
  if (mode === "leaf") {
    process.on("SIGTERM", () => {
      if (process.connected) process.send("ignored");
    });
    process.send({ ready: process.pid });
  } else {
    let parentIgnored = false;
    let descendantIgnored = false;
    let acted = false;
    function proceed() {
      if (!parentIgnored || !descendantIgnored || acted) return;
      acted = true;
      if (mode === "stdout" || mode === "stderr") {
        process[mode].write(Buffer.alloc(128 * 1024, "x"));
      } else if (mode.startsWith("exit-parent")) {
        process.exit(0);
      }
    }
    process.on("SIGTERM", () => {
      parentIgnored = true;
      process.stdout.write("parent ignored SIGTERM\n");
      proceed();
    });
    const noPipes = mode === "exit-parent-no-pipes";
    const child = spawn(process.execPath, [__filename, "leaf"], {
      stdio: ["ignore", noPipes ? "ignore" : "inherit", noPipes ? "ignore" : "inherit", "ipc"],
    });
    child.on("message", (message) => {
      if (message.ready) {
        process.stdout.write(`READY ${JSON.stringify({ parent: process.pid, descendant: message.ready })}\n`);
        process.kill(process.pid, "SIGTERM");
        child.kill("SIGTERM");
      } else if (message === "ignored") {
        descendantIgnored = true;
        process.stdout.write("descendant ignored SIGTERM\n");
        proceed();
      }
    });
  }
}
