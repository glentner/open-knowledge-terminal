---
status: declined
kind: fix
appetite: small
---

# A code fence in a tight list item renders with an extra newline

**Declined, 2026-10-15.** This was a defect only against the reference (`bin/terminal.py`), whose
`/api/file` output was the parity contract. The reference retired from the KB on 2026-10-14 and
okt is now its own specification. The extra `\n` sits between inline text and a block `<pre>`,
where it doesn't render. Reopen if a consumer of `/api/file` turns out to be whitespace-sensitive.
The record below is kept as written.

> **Candidate, not a contract.** `code okt feature issues/tight-list-fence-newline.md` promotes it;
> appetite, non-goals and R-IDs get negotiated there.

## Problem

When a fenced code block follows the text of a **tight** list item, okt's HTML has a `\n` between
the item's text and `<pre>`; the reference has none. Loose items (a blank line before the fence)
match. `/api/file` is part of the parity contract (`AGENTS.md` § Invariants 1), so `api_diff.py`
fails on any KB file with this shape.

## Evidence

Found 2026-10-09 by `tests/parity/api_diff.py` inside `tests/parity/env.sh`: 537/538 files
identical. The 538th was `workspace/code-harness/draft/skill/code-plan.md` (step 9: a fence inside
an ordered-list item).

Minimal repro, a KB holding one file:

````markdown
1. Tight item text:
   ```
   code
   ```
2. Next
````

- reference: `<li data-line="6">Tight item text:<pre><code data-line="7">code\n</code></pre>\n</li>`
- okt: `<li data-line="6">Tight item text:\n<pre><code data-line="7">code\n</code></pre>\n</li>`

The same holds for bullet lists. Where to start: `okt-server/src/render/markdown.rs`, the
tight/loose list-item handling near L227 (as of `64605fb`).

## Possible acceptance (to negotiate)

- The repro above renders byte-identical to the reference, for ordered and bullet lists.
- `api_diff.py` over the KB reports no `/api/file` differences from this shape.
- Loose list items, and fences outside lists, are unchanged.

## Non-goals (to negotiate)

- Other markdown rendering differences not shown by the repro.
