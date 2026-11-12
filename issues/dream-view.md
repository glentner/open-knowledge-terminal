---
status: unshaped
kind: feature
appetite: big
---

# A Dream view: step through the KB's memory proposals by key

> **Candidate, not a contract.** `code okt feature issues/dream-view.md` promotes it;
> appetite, non-goals and R-IDs get negotiated there.

## Problem

The KB's `dream` run (`~/KB/skill/dream.md`) drains short-term memory into proposals, then asks
Geoffrey to approve, drop or defer each one. Today that review is a chat digest: a long message
answered with one structured reply covering every item. Geoffrey, 2026-11-12: "I need better UI/UX
for stepping through each thing instead of a wall of text that needs my responses as another
monolithic structured response."

The cost is real. The Dream v2 run of 2026-11-09 wrote 42 proposals; none has been answered three
days later, while the intake grew by 252 lines. A review that must be finished in one sitting
doesn't get started.

okt is Geoffrey's interface to the whole KB, and pane arrangements (0.4.0) and drag-to-dock (0.4.2)
exist so new views can join it. It has no view for memory or for the message boards.

## Outcome / vision

A `Dream` view, opened by a `DREAM` command in the GO bar, that docks in a pane like any other view.
It shows one proposal at a time: its group and kind, its targets, one line on what changes in agent
behavior, any flags, and its body with the old and new text. A single key answers it and steps to
the next. The target card, or the inbox line a proposal came from, opens in the Inspector beside it,
so Geoffrey reads the card he is about to change without leaving the walk.

Every answer is saved the moment it is given. Closing okt mid-walk loses nothing; reopening the
view starts at the first item without an answer.

## The contract

The KB owns the payloads; okt reads one and writes the other. Neither lives in a git repository.
v1 is fixed in `~/KB/skill/dream.md` → Walk payload, which governs if they differ:

- **Location:** `${XDG_STATE_HOME:-~/.local/state}/kb/dream/`, or `$KB_DREAM_DIR` when set.
- **Inbound, `walk.json`,** rebuilt at will by the KB's `bin/dream-walk` from `memory/proposals/`:

  ```json
  {
    "version": 1,
    "generated": "2026-11-12T14:05:00-05:00",
    "kb_head": "2e8c390",
    "groups": [
      {"id": "G1", "title": "Lane migration", "kind": "harness", "batch": false, "items": 5},
      {"id": "Record", "title": "Record", "kind": "record", "batch": true, "items": 38}
    ],
    "items": [{
      "id": "P2026-11-09-01",
      "path": "memory/proposals/P2026-11-09-01.md",
      "sha": "sha256 of the proposal file",
      "group": "G1",
      "kind": "kernel",
      "targets": ["AGENTS.md"],
      "created": "2026-11-09",
      "seen": 1,
      "deferred": null,
      "summary": "One line: what changes in agent behavior, or why",
      "lands": "the inbox lines or source it lands",
      "flags": ["gate-loosening"],
      "body": "the proposal's Markdown after its frontmatter: quoted old and new text, or a new file"
    }],
    "deferred_left_out": ["P2026-09-23-05"]
  }
  ```

  Items arrive in walk order. `kind` is `record`, `harness` or `kernel`; `batch: true` marks a
  group that may be answered whole. `path` is KB-relative, so the Inspector can open the proposal
  itself, and every `targets` entry is a KB path too. Proposal bodies are Markdown, with old and
  new text in indented `-`/`+` blocks.
- **Outbound, `answers.json`,** written by okt, one entry per answer, last answer per `id` wins:

  ```json
  {
    "version": 1,
    "answers": [{
      "id": "P2026-11-09-01",
      "sha": "sha256 copied from the walk item",
      "verdict": "approve",
      "note": "optional free text: an amendment or a reason",
      "at": "2026-11-12T14:07:31-05:00"
    }]
  }
  ```

  `verdict` is one of `approve`, `drop`, `defer`. Answers are matched by `id` and `sha`, never by
  position, so the walk can be rebuilt between sessions without losing them. An answer whose `sha`
  no longer matches its item is stale: the view shows that item as unanswered again. When the KB
  lands a set of answers it renames `answers.json` aside, and the view starts from an empty one.

The KB's `bin/dream-walk reply` turns `answers.json` into the Markdown reply `skill/dream.md`
already accepts. That reply is Geoffrey's official response, and the KB lands approvals from it.

## Sketch of the acceptance criteria

- `DREAM` in the GO bar opens the view; it docks, drags and persists like the other main views.
  With no `walk.json`, it says so and names the command that writes one.
- One item at a time, with progress in the view's header (group, position in group, position
  overall, unanswered count).
- Keys while the Dream view is the key view, final letters decided at plan time against
  `keys.rs`: approve, drop, defer, respond (a note field; a note with approve is an amendment),
  next and previous, undo the last answer, show unanswered only, open the target or source in the
  Inspector, and answer a `batch` group whole after a confirm.
- Each answer reaches `answers.json` before the view steps on. A failed save is shown, and the
  view doesn't advance.
- The view reloads when `walk.json` changes, through the existing event stream, and keeps the
  answers whose `sha` still matches.
- The proposal body renders as the Inspector renders KB Markdown, its `-`/`+` blocks highlighted.
- A browser suite walks a fixture `walk.json`: answer, undo, stale-answer reset, reload mid-walk,
  batch answer, save failure.

## Open questions

- **Q1.** How okt finds the payload directory: a CLI flag, a setting, or a fixed path under the
  KB's name.
- **Q2.** The write route. A `POST` in `okt-server/src/http.rs` is a human-gate path (AGENTS.md →
  Invariants) and needs the link-token check `/api/open` has. Is there a design that keeps okt
  read-only here, and is it worth what it costs (answers held in the browser until exported)?
- **Q3.** Key letters. Most single letters are already global (`keys.rs`); the plan says which
  ones the Dream view may shadow while it is the key view.

## Notes

- Out of scope: generating the walk, landing approvals, and anything else that writes the KB.
  okt reads `walk.json` and writes `answers.json`, nothing more.
- Related: `issues/mobile-responsive.md` makes okt on a device a reader with launch actions
  refused. The answer route should be refused there too.
- A later candidate, not this cycle: the same items-and-verdict-keys shape for the message boards
  (`~/KB/message/`), which also have no view.
