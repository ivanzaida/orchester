// Updates a docs tracker (`docs/plan/<module>/`): node tracker.mjs <tracker folder> <json of updates>
// updates: [{id:"PV2-04", status:"done", branch:"…", tokens:"…", row:{status, branch, tokens}}]
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const [dir, json] = process.argv.slice(2);
const updates = JSON.parse(readFileSync(json, 'utf8'));
const readmePath = join(dir, 'README.md');
const lines = readFileSync(readmePath, 'utf8').split('\n');
for (const u of updates) {
  const i = lines.findIndex((l) => l.startsWith(`| [${u.id}](`));
  if (i < 0) throw new Error(`no row for ${u.id}`);
  const cells = lines[i].split(' | ');
  // cells: [ "| [ID](file)", task, kind, depends, status, branch, "tokens |" ]
  if (u.row?.status) cells[4] = u.row.status;
  if (u.row?.branch) cells[5] = u.row.branch;
  if (u.row?.tokens) cells[6] = `${u.row.tokens} |`;
  lines[i] = cells.join(' | ');
  const file = readdirSync(dir).find((f) => f.startsWith(`${u.id}-`));
  let text = readFileSync(join(dir, file), 'utf8');
  if (u.status) text = text.replace(/^- \*\*Status:\*\* .*$/m, `- **Status:** ${u.status}`);
  if (u.branch) text = text.replace(/^- \*\*Branch:\*\* .*$/m, `- **Branch:** ${u.branch}`);
  if (u.tokens) text = text.replace(/^- \*\*Tokens:\*\* .*$/m, `- **Tokens:** ${u.tokens}`);
  writeFileSync(join(dir, file), text);
}
writeFileSync(readmePath, lines.join('\n'));
