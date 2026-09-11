"use strict";

const { spawn } = require("node:child_process");

// Probe the actual binary: capability drift must not be hidden by a source-text scan.
function capabilities(binary) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, ["--stdio"], { stdio: "pipe", windowsHide: true });
    let buffer = Buffer.alloc(0);
    let result;
    let stderr = "";
    let failure;
    const fail = (error) => { failure = error; child.kill(); };
    const timer = setTimeout(() => fail(new Error("LSP capability probe timed out after 15 seconds")), 15000);
    const send = (message) => {
      const payload = Buffer.from(JSON.stringify({ jsonrpc: "2.0", ...message }));
      child.stdin.write(Buffer.concat([Buffer.from(`Content-Length: ${payload.length}\r\n\r\n`), payload]));
    };
    child.stderr.on("data", (chunk) => { stderr = (stderr + chunk).slice(-8000); });
    child.stdin.on("error", fail);
    child.on("error", (error) => { clearTimeout(timer); reject(error); });
    child.stdout.on("data", (chunk) => {
      try {
        buffer = Buffer.concat([buffer, chunk]);
        if (buffer.length > 1024 * 1024) throw new Error("oversized LSP probe response");
        while (buffer.length) {
          const end = buffer.indexOf("\r\n\r\n");
          if (end < 0) break;
          const header = buffer.subarray(0, end).toString("ascii");
          const length = Number(/^Content-Length: (\d+)$/mi.exec(header)?.[1]);
          if (!Number.isSafeInteger(length) || length > 1024 * 1024) throw new Error("invalid Content-Length");
          if (buffer.length < end + 4 + length) break;
          const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString("utf8"));
          buffer = buffer.subarray(end + 4 + length);
          if (message.error) throw new Error(`LSP probe failed: ${JSON.stringify(message.error)}`);
          if (message.id === 1) {
            result = message.result.capabilities;
            send({ method: "initialized", params: {} });
            send({ id: 2, method: "shutdown", params: null });
          } else if (message.id === 2) {
            send({ method: "exit" });
            child.stdin.end();
          }
        }
      } catch (error) { fail(error); }
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      if (failure) reject(failure);
      else if (code !== 0 || !result) reject(new Error(`LSP probe exited ${code}: ${stderr}`));
      else resolve(result);
    });
    send({ id: 1, method: "initialize", params: { processId: process.pid, capabilities: {} } });
  });
}

module.exports = { capabilities };
