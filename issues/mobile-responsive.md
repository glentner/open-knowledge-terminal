---
status: unshaped
kind: feature
appetite: big
---

# okt on an iPhone and an iPad over Tailscale

## Problem

Geoffrey wants to serve okt from his Mac to his iPhone and iPad over Tailscale and have a solid
experience there (2026-11-11). Today okt is built for one desktop browser on loopback, with a
keyboard and a mouse. Two things stand in the way: reaching the server from another device, and
using the UI once it loads.

### Reaching it

- `okt-server/src/main.rs:66`: the server binds `127.0.0.1` by default; `--host` widens it.
- `main.rs:605-607`: binding beyond loopback turns on `--require-token`. `main.rs:690-698`: the
  `Host` allowlist is `127.0.0.1`, `localhost`, `[::1]` and the bound host, each with `:{port}`.
  `okt-server/src/http.rs:147` (`host_ok`) refuses any other `Host` with a 403 on every request.
  So a MagicDNS name (`https://<machine>.<tailnet>.ts.net/`) or a port-less `Host` behind an HTTPS
  proxy is refused today.
- `http.rs:186-193`: the link token becomes a session cookie (`HttpOnly; SameSite=Strict`, no
  `Max-Age`), and `http.rs:209` checks it on every GET. `main.rs:699-700`: the token is new on
  every start. A device has to be handed a fresh link after each restart or `make install`. A
  home-screen app on iOS keeps its own cookies apart from Safari, and the manifest's `start_url`
  is `/` (`okt-server/src/pwa.rs:177`), with no token. So an installed web app may never get the
  cookie at all. This needs checking on a device.
- `okt-client/src/chrome.rs:583-590`: the service worker registers only in a secure context.
  Plain `http://` to a tailnet address is not one, and neither is the clipboard API behind `y`
  (`okt-client/src/actions.rs:137`). HTTPS (`tailscale serve`, or a Tailscale certificate) is what
  makes okt installable and its copy work on the devices.
- `http.rs:684`: `/api/open` and the other launch POSTs run the editor, open and reveal on the
  Mac (`okt-server/src/launch.rs`). From a phone, EDIT, OPEN and REVEAL would open apps on the
  Mac's screen, not on the device.

`http.rs` and `launch.rs` are high-blast-radius paths (`AGENTS.md` § Invariants), so the access
half of this work is under the human gate.

### Using it

A probe of the default arrangement in Chromium's mobile emulation, with touch, served by a
throwaway okt at `a392a98` (`probe.mjs`, `probe.json` and screenshots in
`.local/mobile-responsive/` in the main checkout), 2026-11-12:

| Viewport | Layout width | Panes | Tap targets under 44 px |
|---|---|---|---|
| iPhone portrait, 393×852 | 441 px: the header pushes the page wider than the screen | stacked, 385 px each | 48 of 48 (median 19 px tall) |
| iPhone landscape, 852×393 | 852 px | stacked, 156 px each | 48 of 48 |
| iPad portrait, 820×1180 | 820 px | stacked, 549 px each | 48 of 48 |
| iPad landscape, 1180×820 | 1180 px | side by side, 637 and 543 px | 48 of 48 |

- The only width rule is `okt-client/static/app.css:701`: under 900 px it hides the brand word, the
  KB name and the crumbs. The first-load arrangement is stacked under 900 px
  (`okt-client/src/arrange.rs:187`). On an iPhone in portrait, the header's GO bar, status and
  icon chips still need 441 px, so the page renders zoomed out and the settings and help buttons
  fall off the right edge. The status bar at the bottom overflows too (`iphone-portrait.png`).
- The GO input's text is 12.5 px. iOS Safari zooms the page when an input under 16 px takes
  focus.
- Every button and tab is under Apple's 44 pt minimum: the median is 19 px tall, the header chips
  are 24 px, and the graph's zoom buttons about 25 px.
- okt is driven by the keyboard. Navigation (`j`/`k`, `h`/`l`, the `w` pane leader, the presets,
  `/` find, pins, `n`/`N`) has no touch route on an iPhone without a keyboard. An iPad with a
  keyboard keeps most of it.
- The device reports `(hover: none)` and `(pointer: coarse)`, but nothing in `app.css` reads
  either. 34 rules use `:hover`; whether any of them is the only way to reveal something is
  unchecked.
- The graph pans and zooms by mouse wheel and drag (`okt-client/src/graph.rs:2184`), with no
  pinch. A drag of a pane tab or a sash uses pointer events (`touch-action: none` on `.ptab` and
  `.sash`, `app.css:161` and `:172`), so it may already work by touch, but nothing has tried it.
- At phone sizes a second pane is rarely worth its space: in landscape each stacked pane is
  156 px tall. The 200 px narrow floor (`arrange.rs:10`) already takes views off screen when a pane
  is narrower.
- The iPad layouts are close to usable as they are (`ipad-portrait.png`). The gaps there are
  touch-target size, the keyboard dependence when no keyboard is attached, and the graph's input.

## Why it was deferred

Not a deferral: a new direction Geoffrey asked for on 2026-11-11, seeded for shaping. It is
**pre-existing** in the sense that every finding above holds on `main` at `a392a98`; no cycle
introduced it. It is big and likely splits into two cycles at shaping, access first, because the
layout can't be tried on a device until the device can reach the server.

## Outcome / vision

From his iPhone or iPad, anywhere on his tailnet, Geoffrey opens okt over HTTPS by its MagicDNS
name, installs it to the home screen once, and it keeps working across server restarts. On the
iPhone it is a single-view reader: one view fills the screen, a thumb-reachable control switches
views, GO is reachable and doesn't zoom the page, the Inspector reads well, and the graph pans
and pinches. On the iPad it keeps the panes, with touch-sized targets, and works with or without a
keyboard. On the Mac, at desktop widths, nothing changes.

## Sketch of the acceptance criteria

Access:
- **R1** WHEN okt is reached through an HTTPS name Geoffrey configures (a MagicDNS name behind
  `tailscale serve`, say), it SHALL serve every route, live updates included, while the default
  stays loopback-only and every other `Host` is still refused.
- **R2** A device that has been let in once SHALL stay let in across a server restart and a
  `make install`, including okt installed to the iOS home screen, without exposing the KB to a
  device that was never let in.
- **R3** WHERE a request comes from another device, the launch actions (EDIT, OPEN, REVEAL) SHALL
  either be hidden there or do what shaping decides, never silently open an app on the Mac.

Layout:
- **R4** At 375–430 px wide (iPhone portrait), no part of the page SHALL extend past the screen's
  width, and the header's controls SHALL all be reachable.
- **R5** On a touch device (`pointer: coarse`), every control SHALL be at least 44×44 CSS px to tap.
- **R6** Focusing GO, or any other text input, SHALL NOT zoom the page on iOS Safari.
- **R7** On a phone, one view SHALL fill the screen below the header, with a touch control that
  switches between every view, Inspector included.
- **R8** Every action the keyboard reaches that shaping marks essential for reading (open, back
  and forward, find, switch view, switch inspector tab, pins) SHALL have a touch route.
- **R9** The graph SHALL pan by one-finger drag and zoom by pinch.
- **R10** At desktop widths with a fine pointer, layout and behaviour SHALL stay as they are
  today: `keys.mjs`, `panes.mjs` and `dock.mjs` unchanged and passing.
- **R11** The browser suites SHALL grade R4–R9 at iPhone and iPad sizes with touch input,
  WebKit included if Playwright's WebKit runs here.

## Notes

- Questions for shaping:
  - Is the iPhone a reader only, or does it edit? EDIT has no meaning there today.
  - Is the tailnet the security boundary, with the token dropped behind `tailscale serve`? Or does
    the token stay, made long-lived and per device?
  - Does okt serve HTTPS itself, with a Tailscale certificate, or stay plain HTTP behind
    `tailscale serve`? The second keeps the server unchanged except for the `Host` allowlist.
  - On iPad, should touch drag-to-dock and sash drags be in scope, or is the iPad layout read-only?
  - What is "solid"? A short list of tasks done on each device would make R8 gradeable.
- The KB holds Purdue work. Which devices and which tailnet see it is Geoffrey's call, and shaping
  should write that call into the GOAL.
- The probe's mobile emulation is Chromium, not Safari. Safari's viewport, input-zoom and
  home-screen behaviour needs a real device or Playwright's WebKit; whether WebKit runs in this
  harness is untested.
- Related: `issues/offbar-placement.md` (the SERVER STOPPED banner wraps to three or four lines at
  phone widths) and `issues/crumbs-kb-name-twice.md` (the header's crumbs).
- Found by: Geoffrey's request in session, 2026-11-11; evidence from the foreman's probe on
  2026-11-12.
