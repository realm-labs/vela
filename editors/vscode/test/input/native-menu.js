"use strict";
const path = require("node:path");
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");

function windowsDesktop(operation, pid = 0, layout = "00000409", window = "0") {
  return JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-File",
    path.join(__dirname, "windows-desktop.ps1"), "-Operation", operation,
    "-TargetProcess", String(pid), "-Layout", layout, "-Window", window],
  { encoding: "utf8", timeout: 15000, windowsHide: true }));
}

function buildNativeMenu(root, platform) {
  if (platform === "win32") { windowsDesktop("preflight"); return; }
  if (platform !== "darwin") throw Error(`unsupported input platform ${platform}`);
  execFileSync("swiftc", [path.join(__dirname, "native-menu.swift"), "-o", path.join(root, "native-menu")],
    { stdio: "inherit", timeout: 120000 });
}

function nativeMenu({ root, platform, page, pid }) {
  const invoke = (operation, target) => JSON.parse(execFileSync(path.join(root, "native-menu"),
    [String(pid), operation, target], { encoding: "utf8", timeout: 5000 }));
  // Windows renders VS Code context menus inside the workbench. CDP pointer
  // actions use the real menu; no command/provider call substitutes for input.
  const item = (title) => page.locator('.monaco-menu [role="menuitem"]')
    .filter({ has: page.locator('.action-label').filter({ hasText: new RegExp(`^${title}$`) }) });
  return {
    async activate() {
      if (platform === "darwin") invoke("activate", "setup");
      else await page.bringToFront();
    },
    async contextClick(pointer) {
      if (platform === "darwin") return invoke("context-click", JSON.stringify({ ...pointer,
        ...await page.evaluate(() => ({ width: innerWidth, height: innerHeight })) }));
      await page.mouse.click(pointer.x, pointer.y, { button: "right" });
      return { ...pointer, backend: "Electron workbench menu" };
    },
    async inspect(title) {
      if (platform === "darwin") return invoke("inspect", title);
      const target = item(title);
      await target.waitFor({ state: "visible" });
      assert.equal(await target.count(), 1, "one visible menu item must own the action");
      return { title: await target.locator('.action-label').innerText(),
        role: await target.getAttribute("role"), enabled: await target.getAttribute("aria-disabled") !== "true" };
    },
    async operate(title, operation) {
      if (platform === "darwin") invoke(operation, title);
      else if (operation === "hover") await item(title).hover();
      else await item(title).click();
    },
    async screenshot(file, visible) {
      if (platform === "darwin") {
        const b = visible.menuBounds;
        execFileSync("screencapture", ["-x", "-R",
          [Math.floor(b.x), Math.floor(b.y), Math.ceil(b.width), Math.ceil(b.height)].join(","), file], { timeout: 5000 });
      } else await page.screenshot({ path: file });
    },
  };
}
module.exports = { buildNativeMenu, nativeMenu, windowsDesktop };
