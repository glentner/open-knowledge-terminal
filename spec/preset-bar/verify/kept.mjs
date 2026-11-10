// The existing browser checks stay. Build edits tests/ui/panes.mjs (its preset checks) and runs
// keys.mjs and history.mjs on a page with a new header, so the gate can't trust exit codes alone:
// a deleted or loosened check would still exit 0. Frozen with the plan commit.
//
//   node kept.mjs <base-ref>             static: every check label at <base-ref> is still in the file
//   node kept.mjs <base-ref> <run-log> [suite…]   and also printed as passed in <run-log>, with no
//                                        FAIL line; with suites named, only those (a --part run)
//
// A label is the first argument of each check( call in the suite as it stood at <base-ref> (the
// merge base with main). Its literal part (the longest run between ${…} substitutions, at least 4
// characters) must appear in the current file, and in the log on an "ok" line, never on a "FAIL"
// line. REPLACED names the one label this cycle replaces (GOAL R7: the window bar's mark and click):
// it must be gone from the file and from the log. A suite absent at <base-ref> is skipped and named.
// Footprint: one node process, under 1 s.
import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';

const [ref, logPath, ...only] = process.argv.slice(2);
if (!ref) { console.error('usage: node kept.mjs <base-ref> [run-log]'); process.exit(2); }
const ALL = ['tests/ui/keys.mjs', 'tests/ui/history.mjs', 'tests/ui/panes.mjs'];
const SUITES = only.length ? ALL.filter(f => only.includes(f)) : ALL;
const REPLACED = { 'tests/ui/panes.mjs': ['the window bar marks the current preset and a click applies it'] };
const log = logPath ? readFileSync(logPath, 'utf8').split('\n') : null;
let fails = 0, total = 0;

const unescape = s => s.replace(/\\(.)/g, '$1');
for (const f of SUITES) {
  let old;
  try { old = execFileSync('git', ['show', `${ref}:${f}`], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }); }
  catch { console.log(`skip kept: ${f} is not at ${ref}`); continue; }
  if (!existsSync(f)) { console.log(`FAIL kept: ${f} is gone`); fails++; continue; }
  const now = readFileSync(f, 'utf8');
  const gone = REPLACED[f] || [];
  const labels = [...old.matchAll(/check\(\s*(`(?:[^`\\]|\\.)*`|'(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*")/g)].map(m => m[1].slice(1, -1));
  for (const g of gone) if (!labels.some(l => unescape(l) === g)) { console.log(`FAIL kept: the replaced label "${g}" is not in ${f} at ${ref}`); fails++; }
  for (const label of labels) {
    const seg = label.split(/\$\{[^}]*\}/).map(x => unescape(x).trim()).sort((a, b) => b.length - a.length)[0] || '';
    if (seg.length < 4) continue;
    total++;
    const has = now.includes(seg) || now.includes(seg.replace(/'/g, "\\'"));
    if (gone.includes(unescape(label))) {
      if (has) { console.log(`FAIL kept: ${f} still has the replaced check "${seg}"`); fails++; }
      if (log && log.some(l => l.includes(seg))) { console.log(`FAIL kept: the replaced check "${seg}" still ran`); fails++; }
      continue;
    }
    if (!has) { console.log(`FAIL kept: ${f} lost the check "${seg}"`); fails++; continue; }
    if (log) {
      const okLine = log.some(l => l.startsWith('ok') && l.includes(seg));
      const failLine = log.some(l => l.startsWith('FAIL') && l.includes(seg));
      if (!okLine || failLine) { console.log(`FAIL kept: ${f} "${seg}" ${failLine ? 'failed' : 'was not printed as passed'}`); fails++; }
    }
  }
}
console.log(fails ? `kept: ${fails} of ${total} failed` : `ok   kept: ${total} existing checks ${log ? 'present and passed' : 'present'}, the replaced one gone`);
process.exit(fails ? 1 : 0);
