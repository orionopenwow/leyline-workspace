// node tools/den-ui/render.cjs <scenes dir> <out dir> <WxH pixels> [prefix]
// Draws the Den editor's scenes (tools/den-ui/scenes.lua's JSON: every visible region's rect in UI units) as the
// game would at that screen size, and saves a PNG per scene with Playwright's Chromium. The game's own frames
// (minimap, chat, bars, player frame) are drawn as labelled outlines so overlaps show. No game data: an icon is drawn
// as a dark square with its name (Interface\Icons\<name>), a 3D model tile as a turntable glyph with its file name.
const fs = require('fs');
const path = require('path');

const [, , inDir, outDir, px = '1920x1080', prefix = ''] = process.argv;
const [PW, PH] = px.split('x').map(Number);

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const rgba = (c, a = 1) => (c ? `rgba(${Math.round(c[0] * 255)},${Math.round(c[1] * 255)},${Math.round(c[2] * 255)},${(c[3] ?? 1) * a})` : 'transparent');
// the game's colour codes in a text: |cAARRGGBB ... |r
function richText(t) {
  let out = '';
  let open = false;
  const parts = String(t).split(/(\|c[0-9a-fA-F]{8}|\|r|\|T[^|]*\|t|\n)/);
  for (const p of parts) {
    if (/^\|c/.test(p)) { if (open) out += '</span>'; out += `<span style="color:#${p.slice(4)}">`; open = true; }
    else if (p === '|r') { if (open) out += '</span>'; open = false; }
    else if (/^\|T/.test(p)) out += '<span class="inl"></span>';
    else if (p === '\n') out += '<br>';
    else out += esc(p.replace(/\|\|/g, '|'));
  }
  if (open) out += '</span>';
  return out;
}

// our own icons (tools/den-ui/icons/make_icons.py): the addon's TGA files, drawn tinted on a canvas
const ICON_DIR = path.join(__dirname, '..', '..', 'stream', 'fui-web', 'Interface', 'AddOns', 'WrathUnleashedUI', 'Media', 'Icons');
function tgaRGBA(file) {
  const b = fs.readFileSync(file);
  const w = b.readUInt16LE(12), h = b.readUInt16LE(14), top = (b[17] & 0x20) !== 0;
  const out = Buffer.alloc(w * h * 4);
  for (let y = 0; y < h; y++) {
    const sy = top ? y : h - 1 - y;
    for (let x = 0; x < w; x++) {
      const s = 18 + (sy * w + x) * 4, d = (y * w + x) * 4;
      out[d] = b[s + 2]; out[d + 1] = b[s + 1]; out[d + 2] = b[s]; out[d + 3] = b[s + 3];
    }
  }
  return out.toString('base64');
}
function page(doc) {
  const k = PH / doc.screen.h; // pixels per UI unit
  const H = doc.screen.h;
  const els = [];
  const icons = new Set();
  for (const r of doc.regions) {
    const left = r.x * k;
    const top = (H - r.y - r.h) * k;
    const w = r.w * k;
    const h = r.h * k;
    const box = `left:${left.toFixed(1)}px;top:${top.toFixed(1)}px;width:${w.toFixed(1)}px;height:${h.toFixed(1)}px;opacity:${r.alpha}`;
    if (r.bd || r.bdedge) {
      // a backdrop: the rounded tooltip edge over a fill (Style.lua S.ROUND)
      const rad = Math.max(3, (r.edge || 12) * 0.5 * k);
      const bw = Math.max(1, (r.edge || 12) / 9 * k);
      els.push(`<div style="${box};border-radius:${rad.toFixed(1)}px;${r.bd ? `background:${rgba(r.bd)};` : ''}${r.bdedge ? `border:${bw.toFixed(1)}px solid ${rgba(r.bdedge)};` : ''}"></div>`);
    }
    if (r.stock) {
      els.push(`<div class="stock" style="${box}"><span>${esc(r.stock)}</span></div>`);
    } else if (r.kind === 'Texture') {
      if (r.file) {
        const name = String(r.file).split(/[\\/]/).pop();
        const isIcon = /Interface\\Icons\\/i.test(r.file);
        const tint = r.color ? `background:${rgba(r.color)};` : '';
        const own = /Media\\Icons\\(\w+)$/i.exec(r.file);
        if (own) { icons.add(own[1]); els.push(`<canvas class="ic" width="64" height="64" data-n="${own[1]}" data-c="${(r.color || [1, 1, 1, 1]).join(',')}" style="${box}"></canvas>`); continue; }
        if (/white8x8|WHITE8X8/i.test(r.file)) els.push(`<div style="${box};background:${rgba(r.color || [1, 1, 1, 1])}"></div>`);
        else if (/leyline-mark/i.test(r.file)) els.push(`<div class="mark" style="${box}"></div>`);
        else els.push(`<div class="icon ${isIcon ? '' : 'tex'} ${r.desat ? 'desat' : ''}" style="${box};${tint}" title="${esc(r.file)}"><span style="font-size:${Math.max(7, Math.min(11, h / 4.2))}px">${esc(name)}</span></div>`);
      } else if (r.color) {
        els.push(`<div style="${box};background:${rgba(r.color)}"></div>`);
      }
    } else if (r.kind === 'FontString' || (r.kind === 'EditBox' && r.text)) {
      if (r.text == null || r.text === '') continue;
      const size = (r.font || 12) * k;
      const just = r.justify === 'CENTER' ? 'center' : r.justify === 'RIGHT' ? 'right' : 'left';
      const col = r.tcolor ? rgba(r.tcolor) : (r.kind === 'EditBox' ? '#eee' : '#ffd100');
      const pad = r.kind === 'EditBox' ? 'padding-left:5px;' : '';
      // one line when the region is one line high (the game cuts it with "..."), else wrapped at its width
      const one = r.kind === 'EditBox' || r.h < (r.font || 12) * 1.9;
      const flow = one ? `white-space:nowrap;overflow:hidden;text-overflow:ellipsis;line-height:${h.toFixed(1)}px;` : `white-space:normal;line-height:${(size * 1.25).toFixed(1)}px;`;
      const ww = r.autoW ? 'width:auto;' : '';
      els.push(`<div class="txt" style="${box};${ww}font-size:${size.toFixed(1)}px;${flow}text-align:${just};color:${col};${pad}">${richText(r.text)}</div>`);
    } else if (r.kind === 'Model' && r.model) {
      const name = String(r.model).split(/[\\/]/).pop();
      els.push(`<div class="model" style="${box}"><div class="spin"></div><span>${esc(name)}</span></div>`);
    }
  }
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;padding:0}
  body{width:${PW}px;height:${PH}px;position:relative;overflow:hidden;font-family:"DejaVu Sans","Liberation Sans",Arial,sans-serif;
    background:radial-gradient(ellipse at 50% 60%,#e9ecef 0%,#cfd5db 55%,#9aa3ab 100%)}
  .floor{position:absolute;left:0;right:0;bottom:0;height:45%;background:linear-gradient(#dfe4e8,#c3cad1)}
  div,canvas{position:absolute;box-sizing:border-box}
  .stock{border:1px dashed rgba(40,60,90,.55);background:rgba(40,60,90,.10)}
  .stock span{position:absolute;left:4px;top:2px;font:11px sans-serif;color:rgba(30,45,70,.8)}
  .txt{text-shadow:0 1px 1px rgba(0,0,0,.9)}
  .icon{background:linear-gradient(135deg,#3a3f4a,#22252c);border:1px solid #555;overflow:hidden}
  .icon.desat{filter:grayscale(1) brightness(.7)}
  .icon span{position:absolute;left:1px;right:1px;bottom:1px;color:#c9cdd4;line-height:1.05;word-break:break-all;text-shadow:0 1px 0 #000}
  .icon.tex{background:#2b2f37}
  .mark{background:radial-gradient(circle,#e0a43a 0 35%,transparent 36%),#111;border-radius:3px}
  .model{background:radial-gradient(ellipse at 50% 70%,rgba(224,164,58,.25),transparent 60%)}
  .model .spin{left:28%;right:28%;top:18%;bottom:30%;border:2px solid rgba(224,164,58,.85);border-radius:50% 50% 12% 12%;
    box-shadow:inset 0 -8px 0 rgba(224,164,58,.25)}
  .model span{left:2px;right:2px;bottom:2px;font:9px sans-serif;color:#d7b46a;text-align:center;white-space:nowrap;overflow:hidden;position:absolute}
  .inl{display:inline-block;width:1em;height:1em;background:#444;vertical-align:middle}
  .cap{position:absolute;right:10px;bottom:8px;font:12px sans-serif;color:rgba(20,30,40,.75);background:rgba(255,255,255,.6);padding:2px 6px;border-radius:3px}
  </style></head><body><div class="floor"></div>${els.join('\n')}
  <div class="cap">${esc(doc.title)} · ${PW}x${PH} · UI ${doc.screen.w.toFixed(0)}x${doc.screen.h.toFixed(0)} units</div>
  <script>(function(){var ICONS={${[...icons].map((n) => { const f = path.join(ICON_DIR, n + '.tga'); return fs.existsSync(f) ? `${n}:"${tgaRGBA(f)}"` : ''; }).filter(Boolean).join(',')}};
  for(const c of document.querySelectorAll('canvas.ic')){const d=ICONS[c.dataset.n];if(!d)continue;const t=c.dataset.c.split(',').map(Number);
  const raw=atob(d);const im=c.getContext('2d').createImageData(64,64);for(let i=0;i<raw.length;i+=4){im.data[i]=raw.charCodeAt(i)*t[0];
  im.data[i+1]=raw.charCodeAt(i+1)*t[1];im.data[i+2]=raw.charCodeAt(i+2)*t[2];im.data[i+3]=raw.charCodeAt(i+3)*(t[3]??1);}c.getContext('2d').putImageData(im,0,0);}})();
  </script></body></html>`;
}

(async () => {
  const { chromium } = require('playwright');
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: PW, height: PH } });
  const p = await ctx.newPage();
  fs.mkdirSync(outDir, { recursive: true });
  const files = fs.readdirSync(inDir).filter((f) => f.endsWith('.json')).sort();
  for (const f of files) {
    const doc = JSON.parse(fs.readFileSync(path.join(inDir, f), 'utf8'));
    const html = page(doc);
    if (process.env.DEN_UI_HTML) fs.writeFileSync(path.join(outDir, f.replace(/\.json$/, '.html')), html);
    await p.setContent(html);
    const out = path.join(outDir, `${prefix}${f.replace(/\.json$/, '')}.png`);
    await p.screenshot({ path: out });
    console.log(out);
  }
  await browser.close();
})().catch((e) => { console.error(e); process.exit(1); });
