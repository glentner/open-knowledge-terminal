// Page mutants for page.mjs's self-test (gate.sh --self-test mutants). Each plants one way the
// page could break a requirement while the rest of the page stays good; page.mjs must go red on
// it, with a FAIL line holding `expect`, running only `groups`. They attack the gate's own
// decisions: the 0.5 px edge, the hit rows, the band, what counts as a preset button, the
// crumbs' slot and clip, the width invariance and its counterfactual, the 900 px rule, the
// single row, the pane shapes and the help rows. Frozen with the plan commit.
const add = html => `document.querySelector('#app > header').insertAdjacentHTML('beforeend', ${JSON.stringify(html)});`;
const chip = html => `document.querySelector('#app > header .chips').insertAdjacentHTML('beforeend', ${JSON.stringify(html)});`;
export const MUTANTS = {
  bar: {
    what: 'the 28 px row back under the header, holding a preset button',
    css: '#app { grid-template-rows: 38px 28px minmax(0, 1fr) var(--ticker-h) 22px !important; } #app.noticker { grid-template-rows: 38px 28px minmax(0, 1fr) 0 22px !important; } .mbar { display: flex; background: #222; }',
    js: `document.querySelector('#app > header').insertAdjacentHTML('afterend', '<nav class="mbar"><button>CLASSIC<kbd>;</kbd></button></nav>');`,
    groups: 'stack', expect: 'the panes meet the header with nothing between',
  },
  overlay: {
    what: 'a 20 px strip fixed over the top of the panes',
    css: '.mstrip { position: fixed; left: 0; right: 0; top: 38px; height: 20px; background: #333; z-index: 40; }',
    js: `document.body.insertAdjacentHTML('beforeend', '<div class="mstrip"></div>');`,
    groups: 'stack', expect: 'the panes meet the header with nothing between',
  },
  thin: {
    what: 'a 2 px strip fixed over the top of the panes, under the right half only',
    css: '.mthin { position: fixed; left: 50%; right: 0; top: 38px; height: 2px; background: #333; z-index: 40; }',
    js: `document.body.insertAdjacentHTML('beforeend', '<div class="mthin"></div>');`,
    groups: 'stack', expect: 'the panes meet the header with nothing between',
  },
  gap: {
    what: 'the panes 1 px below the header',
    css: '#panes { margin-top: 1px !important; }',
    groups: 'stack', expect: 'the panes meet the header with nothing between',
  },
  tall: {
    what: 'the header 46 px tall, over the grid row',
    css: '#app > header { height: 46px !important; }',
    groups: 'stack', expect: 'the panes meet the header with nothing between',
  },
  chipbtn: {
    what: 'a THREE button among the chips',
    js: chip('<button class="chip">THREE</button>'),
    groups: 'stack', expect: 'no preset button in any arrangement',
  },
  kbdonly: {
    what: 'a chip holding only the backtick key',
    js: chip('<button class="chip"><kbd>`</kbd></button>'),
    groups: 'stack', expect: 'no preset button in any arrangement',
  },
  pointer: {
    what: 'a span with a pointer cursor naming a preset, in the brand',
    js: `document.querySelector('#app > header .brand').insertAdjacentHTML('beforeend', '<span style="cursor:pointer">stacked</span>');`,
    groups: 'stack', expect: 'no preset button in any arrangement',
  },
  deadkey: {
    what: 'the stacked key swallowed before okt sees it',
    js: `window.addEventListener('keydown', e => { if (e.key === '\\\\') e.stopImmediatePropagation(); }, true);`,
    groups: 'stack', expect: 'each preset key gives its preset\'s panes',
  },
  grow: {
    what: 'the crumbs sized by their content, taking width from GO',
    css: '.crumbs { flex: 0 0 auto !important; margin-left: 0 !important; }',
    groups: 'sweep', expect: 'GO and the chips keep their widths and places whatever the crumbs hold',
  },
  gap10: {
    what: 'the crumbs keeping the gap before them, 10 px off GO where it is under its cap',
    css: '.crumbs { margin-left: 0 !important; }',
    groups: 'sweep', expect: 'GO and the chips keep their widths with the crumbs out of layout',
  },
  after: {
    what: 'the crumbs after the chips',
    css: '.crumbs { order: 99 !important; }',
    groups: 'sweep', expect: 'from 1600 to 901 px the crumbs sit in the header between GO and the chips',
  },
  gone: {
    what: 'the crumbs never shown',
    css: '.crumbs { display: none !important; }',
    groups: 'sweep', expect: 'from 1600 to 901 px the crumbs sit in the header between GO and the chips',
  },
  spill: {
    what: 'the crumbs unclipped, their trail painting over the chips',
    css: '.crumbs { overflow: visible !important; }',
    groups: 'sweep', expect: 'from 1600 to 901 px the crumbs sit in the header between GO and the chips',
  },
  wrap: {
    what: 'the header wrapping the crumbs onto a second row',
    css: '#app > header { flex-wrap: wrap !important; } .crumbs { flex: 1 0 100% !important; margin-left: 0 !important; }',
    groups: 'sweep', expect: 'the header stays one 38 px row at every width',
  },
  narrow: {
    what: 'the crumbs shown at 900 px and below',
    css: '@media (max-width: 900px) { .crumbs { display: flex !important; flex: 0 0 120px !important; } }',
    groups: 'sweep', expect: 'at 900 px and below the crumbs',
  },
  ghost: {
    what: 'an invisible strip (opacity 0) over the panes\' title bars, taking their clicks',
    css: '.mghost { position: fixed; left: 0; right: 0; top: 40px; height: 6px; opacity: 0; z-index: 40; }',
    js: `document.body.insertAdjacentHTML('beforeend', '<div class="mghost"></div>');`,
    groups: 'stack', expect: 'the panes meet the header with nothing between',
  },
  pseudo: {
    what: 'a 28 px bar hung under the header by ::after, ignoring the pointer',
    css: '#app > header { position: relative; } #app > header::after { content: ""; position: absolute; left: 0; right: 0; top: 100%; height: 28px; background: #333; pointer-events: none; z-index: 40; }',
    groups: 'stack', expect: 'the panes meet the header with nothing between',
  },
  iconchip: {
    what: 'an icon-only chip titled with a preset key, no preset name',
    js: chip('<button class="chip icon" title="Views left, Inspector right (;)">▥</button>'),
    groups: 'stack', expect: 'no preset button in any arrangement',
  },
  datapreset: {
    what: 'a data-preset button in the Inspector\'s head, a KB-data region',
    js: `new MutationObserver(() => { const h = document.getElementById('ihead'); if (h && !h.querySelector('.mdp')) h.insertAdjacentHTML('beforeend', '<button class="mdp" data-preset="three">x</button>'); }).observe(document.documentElement, { childList: true, subtree: true });`,
    groups: 'stack', expect: 'no preset button in any arrangement',
  },
  shift: {
    what: 'the crumbs taking the deficit below GO\'s cap and their content above it, moving the chips',
    css: '.crumbs { flex: 0 10000 auto !important; }',
    groups: 'sweep', expect: 'GO and the chips keep their widths and places whatever the crumbs hold',
  },
  nogrow: {
    what: 'the crumbs never growing into the room GO leaves',
    css: '.crumbs { flex-grow: 0 !important; }',
    groups: 'sweep', expect: 'a selected card\'s crumbs fill the room GO leaves',
  },
  helprow: {
    what: 'the help without the w x row',
    js: `new MutationObserver(() => { for (const dt of document.querySelectorAll('#help dt')) if (dt.textContent.trim() === 'w x') { dt.nextElementSibling?.remove(); dt.remove(); } }).observe(document.documentElement, { childList: true, subtree: true });`,
    groups: 'help', expect: 'the help lists every preset key',
  },
};
