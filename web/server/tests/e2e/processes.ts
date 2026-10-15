import { spawn, type ChildProcess } from "node:child_process";

/**
 * The end-to-end run's own processes: each one's output is kept whole, for the log check at the end, and each is
 * stopped by its own pid. Nothing here touches a process the run did not start.
 */
export interface Started {
  readonly name: string;
  readonly child: ChildProcess;
  /** Everything the process wrote to stdout. */
  stdout(): string;
  /** Everything the process wrote to stderr: the log, for the services that log. */
  stderr(): string;
}

export function start(name: string, command: string, args: readonly string[], env: Record<string, string>, cwd?: string): Started {
  const child = spawn(command, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let out = "";
  let err = "";
  child.stdout.on("data", (chunk: Buffer) => (out += chunk.toString()));
  child.stderr.on("data", (chunk: Buffer) => (err += chunk.toString()));
  return { name, child, stdout: () => out, stderr: () => err };
}

/** Runs a command to its end and answers its stdout; a failure throws with its stderr. */
export async function run(command: string, args: readonly string[], env: Record<string, string> = {}): Promise<string> {
  const started = start(command, command, args, env);
  // `close`, not `exit`: the output is all read only when its pipes close.
  const code = await new Promise<number | null>((resolve) => started.child.on("close", resolve));
  if (code !== 0) {
    throw new Error(`${command} ${args.join(" ")} exited with ${String(code)}: ${started.stderr()}`);
  }
  return started.stdout();
}

export async function waitFor<T>(what: string, probe: () => Promise<T | null> | T | null, seconds = 60): Promise<T> {
  const until = Date.now() + seconds * 1000;
  for (;;) {
    const value = await probe();
    if (value !== null) {
      return value;
    }
    if (Date.now() > until) {
      throw new Error(`timed out waiting for ${what}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}

/** Waits until `url` answers at all (any status). */
export function waitForUrl(url: string, seconds = 120): Promise<true> {
  return waitFor(url, async () => {
    try {
      await fetch(url, { redirect: "manual", signal: AbortSignal.timeout(2000) });
      return true;
    } catch {
      return null;
    }
  }, seconds);
}

export async function stop(started: Started): Promise<void> {
  if (started.child.exitCode !== null || started.child.pid === undefined) {
    return;
  }
  const exited = new Promise((resolve) => started.child.on("exit", resolve));
  started.child.kill();
  await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 10_000))]);
}
