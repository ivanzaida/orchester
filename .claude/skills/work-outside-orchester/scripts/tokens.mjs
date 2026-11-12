// Sums the token usage of the Claude Code sessions whose first message names a marker (a brief's file name), so a
// task's cost is the sum over the children that worked on it.
// Usage: node tokens.mjs [--skip <session id>] <marker> [<marker> …]
//   --skip: the orchestrator's own session, whose transcript names every brief.
// Sessions are read from every `~/.claude*/projects/*/` folder (a config folder other than `.claude` counts too).
// Prints one line per marker: sessions, input, cache write, cache read, output, total; and the sum of all markers.
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const skip = [];
const markers = [];
for (let i = 0; i < args.length; i += 1) {
  if (args[i] === '--skip') skip.push(args[(i += 1)]);
  else markers.push(args[i]);
}

const files = [];
for (const config of readdirSync(homedir()).filter((name) => name.startsWith('.cla'))) {
  const projects = join(homedir(), config, 'projects');
  if (!existsSync(projects) || !statSync(projects).isDirectory()) continue;
  for (const project of readdirSync(projects)) {
    const folder = join(projects, project);
    if (!statSync(folder).isDirectory()) continue;
    for (const name of readdirSync(folder).filter((file) => file.endsWith('.jsonl'))) {
      if (!skip.some((id) => name.includes(id))) files.push(join(folder, name));
    }
  }
}

const all = { sessions: 0, input: 0, cacheWrite: 0, cacheRead: 0, output: 0 };
for (const marker of markers) {
  const sum = { sessions: 0, input: 0, cacheWrite: 0, cacheRead: 0, output: 0 };
  for (const file of files) {
    if (statSync(file).size === 0) continue;
    const text = readFileSync(file, 'utf8');
    const firstUser = text.split('\n').find((line) => line.includes('"type":"user"'));
    if (!firstUser || !firstUser.includes(marker)) continue;
    sum.sessions += 1;
    const seen = new Set();
    for (const line of text.split('\n')) {
      if (!line.includes('"usage"')) continue;
      let row;
      try {
        row = JSON.parse(line);
      } catch {
        continue;
      }
      const message = row.message;
      if (!message?.usage || seen.has(message.id)) continue;
      seen.add(message.id);
      const usage = message.usage;
      sum.input += usage.input_tokens ?? 0;
      sum.cacheWrite += usage.cache_creation_input_tokens ?? 0;
      sum.cacheRead += usage.cache_read_input_tokens ?? 0;
      sum.output += usage.output_tokens ?? 0;
    }
  }
  for (const key of Object.keys(all)) all[key] += sum[key];
  const total = sum.input + sum.cacheWrite + sum.cacheRead + sum.output;
  console.log(`${marker}\tsessions=${sum.sessions}\tinput=${sum.input}\tcacheWrite=${sum.cacheWrite}\tcacheRead=${sum.cacheRead}\toutput=${sum.output}\ttotal=${total}`);
}
const total = all.input + all.cacheWrite + all.cacheRead + all.output;
console.log(`ALL\tsessions=${all.sessions}\ttotal=${(total / 1e6).toFixed(1)}M\toutput=${Math.round(all.output / 1e3)}k`);
