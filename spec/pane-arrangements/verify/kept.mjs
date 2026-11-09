// The existing browser checks stay (GOAL R9: tests/ui/keys.mjs keeps its key assertions and their
// expected outcomes; the same holds for tests/ui/history.mjs, which a predecessor cycle landed).
// Build ports both suites to the new DOM, so the gate can't trust their exit codes alone: a deleted
// or loosened check would still exit 0.
//
//   node kept.mjs <base-ref>             static: every check label at <base-ref> is still in the file
//   node kept.mjs <base-ref> <run-log>   and also printed as passed in <run-log>, with no FAIL line
//
// A label is the first argument of each check( call in the suite as it stood at <base-ref> (the
// merge base with main: the suites as the predecessors landed them). Its literal part (the longest
// run between ${…} substitutions, at least 4 characters) must appear in the current file, and in
// the log on an "ok" line, never on a "FAIL" line. A suite absent at <base-ref> is skipped and named.
// Footprint: one node process, under 1 s.
import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';

const [ref, logPath] = process.argv.slice(2);
if (!ref) { console.error('usage: node kept.mjs <base-ref> [run-log]'); process.exit(2); }
const SUITES = ['tests/ui/keys.mjs', 'tests/ui/history.mjs'];
const log = logPath ? readFileSync(logPath, 'utf8').split('\n') : null;
let fails = 0, total = 0;

const unescape = s => s.replace(/\\(.)/g, '$1');
for (const f of SUITES) {
  let old;
  try { old = execFileSync('git', ['show', `${ref}:${f}`], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }); }
  catch { console.log(`skip kept: ${f} is not at ${ref}`); continue; }
  if (!existsSync(f)) { console.log(`FAIL kept: ${f} is gone`); fails++; continue; }
  const now = readFileSync(f, 'utf8');
  const labels = [...old.matchAll(/check\(\s*(`(?:[^`\\]|\\.)*`|'(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*")/g)].map(m => m[1].slice(1, -1));
  for (const label of labels) {
    const seg = label.split(/\$\{[^}]*\}/).map(x => unescape(x).trim()).sort((a, b) => b.length - a.length)[0] || '';
    if (seg.length < 4) continue;
    total++;
    if (!now.includes(seg) && !now.includes(seg.replace(/'/g, "\\'"))) { console.log(`FAIL kept: ${f} lost the check "${seg}"`); fails++; continue; }
    if (log) {
      const okLine = log.some(l => l.startsWith('ok') && l.includes(seg));
      const failLine = log.some(l => l.startsWith('FAIL') && l.includes(seg));
      if (!okLine || failLine) { console.log(`FAIL kept: ${f} "${seg}" ${failLine ? 'failed' : 'was not printed as passed'}`); fails++; }
    }
  }
}
console.log(fails ? `kept: ${fails} of ${total} failed` : `ok   kept: ${total} existing checks ${log ? 'present and passed' : 'present'}`);
process.exit(fails ? 1 : 0);
