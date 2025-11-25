// Simple demo script that loads the generated wasm pkg and wires up UI
// When serving the `www/` folder directly, import the pkg from ./pkg
import init, { parse } from './pkg/vers_rs.js';

async function run() {
  await init();

  const rangeEl = document.getElementById('range');
  const out = document.getElementById('out');

  function show(v) {
    out.textContent = typeof v === 'string' ? v : JSON.stringify(v, null, 2);
  }

  document.getElementById('btn-string').addEventListener('click', async () => {
    try {
      const s = await parse(rangeEl.value);
      show(s);
    } catch (e) {
      show('Error: ' + (e && e.message ? e.message : String(e)));
    }
  });

  // structured parse button removed; use the single Parse button above

  document.getElementById('btn-clear').addEventListener('click', () => {
    out.textContent = '(no result yet)';
  });
}

run().catch(e => console.error(e));
