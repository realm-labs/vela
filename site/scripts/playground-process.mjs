import { spawn } from 'node:child_process';

// Private to the native playground check and its process regression probes.
// A process group contains ordinary compiler/test descendants, not setsid daemons.
export function runPlaygroundProcess(command, args, {
  cwd,
  env = process.env,
  timeoutMs = 180_000,
  outputLimit = 4 * 1024 * 1024,
} = {}) {
  if (!['darwin', 'linux'].includes(process.platform)) {
    throw new Error('Playground process cleanup requires POSIX groups (macOS/Linux); Windows taskkill is not implemented');
  }
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0 ||
      !Number.isSafeInteger(outputLimit) || outputLimit <= 0) {
    throw new Error('Expected positive finite process deadline and output limit');
  }
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd, env, detached: true, shell: false, stdio: ['ignore', 'pipe', 'pipe'],
    });
    const chunks = { stdout: [], stderr: [] };
    const sizes = { stdout: 0, stderr: 0 };
    let failure;
    let closed = false;
    let cleaning = false;
    let finished = false;
    let code = null;
    let signal = null;
    let poll;
    let cleanupDeadline;
    const deadline = setTimeout(() => {
      failure ??= new Error(`${command}: deadline exceeded (${timeoutMs}ms)`);
      cleanup();
    }, timeoutMs);

    function finish() {
      if (finished) return;
      finished = true;
      clearTimeout(deadline);
      clearTimeout(cleanupDeadline);
      clearInterval(poll);
      const result = {
        pid: child.pid, code, signal,
        stdout: Buffer.concat(chunks.stdout).toString('utf8'),
        stderr: Buffer.concat(chunks.stderr).toString('utf8'),
      };
      if (failure) reject(Object.assign(failure, { result }));
      else resolve(result);
    }

    function groupExists() {
      if (!child.pid) return false;
      try {
        process.kill(-child.pid, 0);
        return true;
      } catch (error) {
        if (error.code === 'ESRCH') return false;
        // macOS can report EPERM while an orphaned group is being reaped.
        // It is still pending, never success; require ESRCH before the deadline.
        if (error.code !== 'EPERM') failure ??= error;
        return true;
      }
    }

    function checkCleanup() {
      if (closed && !groupExists()) finish();
    }

    function cleanup() {
      if (cleaning) return;
      cleaning = true;
      clearTimeout(deadline);
      // Also clean descendants when the launcher exits normally, before waiting
      // for close: inherited pipes can outlive the direct child indefinitely.
      if (child.pid) {
        try {
          process.kill(-child.pid, 'SIGKILL');
        } catch (error) {
          if (error.code !== 'ESRCH') failure ??= error;
        }
      }
      poll = setInterval(checkCleanup, 20);
      cleanupDeadline = setTimeout(() => {
        failure = new Error(`${failure?.message ?? command}: cleanup incomplete after 2000ms (child/pipe closure or process group still pending)`);
        child.stdout.destroy();
        child.stderr.destroy();
        child.unref();
        finish();
      }, 2_000);
      checkCleanup();
    }

    for (const stream of ['stdout', 'stderr']) {
      child[stream].on('data', (chunk) => {
        const remaining = outputLimit - sizes[stream];
        const kept = chunk.subarray(0, remaining);
        if (kept.length) chunks[stream].push(Buffer.from(kept));
        sizes[stream] += kept.length;
        if (chunk.length > remaining) {
          failure ??= new Error(`${command}: ${stream} exceeded ${outputLimit} bytes`);
          cleanup();
        }
      });
      child[stream].on('error', (error) => {
        failure ??= error;
        cleanup();
      });
    }
    child.once('error', (error) => {
      failure ??= error;
      cleanup();
    });
    child.once('exit', (exitCode, exitSignal) => {
      code = exitCode;
      signal = exitSignal;
      if (code !== 0) failure ??= new Error(`${command}: exited with code ${code}, signal ${signal}`);
      cleanup();
    });
    child.once('close', () => {
      closed = true;
      checkCleanup();
    });
  });
}
