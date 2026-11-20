'use strict';
/* Battle maps : sol et décor peints case par case sur la carte (rendu canvas), éditeur du MJ. */

const SOLS = [
  ['herbe', 'Herbe'], ['foret', 'Sous-bois'], ['terre', 'Terre'], ['sable', 'Sable'], ['neige', 'Neige'],
  ['paves', 'Pavés'], ['dalles', 'Dalles'], ['plancher', 'Plancher'], ['tapis', 'Tapis'],
  ['eau', 'Eau'], ['eau-prof', 'Eau profonde'], ['marais', 'Marais'], ['glace', 'Glace'], ['lave', 'Lave'],
  ['gouffre', 'Gouffre'], ['mur', 'Mur'], ['roche', 'Roche'],
];
const DECOS = [
  ['🌲', 'Sapin'], ['🌳', 'Arbre'], ['🌴', 'Palmier'], ['🌿', 'Buisson'], ['🍄', 'Champignons'], ['🌾', 'Hautes herbes'],
  ['🪨', 'Rocher'], ['🪵', 'Bûches'], ['🔥', 'Feu'], ['⛺', 'Tente'], ['🛢️', 'Tonneau'], ['📦', 'Caisse'],
  ['💰', 'Trésor'], ['🪑', 'Chaise'], ['🛏️', 'Lit'], ['🚪', 'Porte'], ['🗝️', 'Clé'], ['🗿', 'Statue'],
  ['⚰️', 'Cercueil'], ['🕯️', 'Bougie'], ['🕸️', 'Toile'], ['🦴', 'Ossements'], ['💀', 'Crâne'], ['⛲', 'Fontaine'],
  ['🏛️', 'Colonnes'], ['⚱️', 'Urne'], ['📜', 'Parchemin'], ['⚗️', 'Alchimie'], ['❌', 'Marque'], ['⚠️', 'Piège'],
];
const MURS = new Set(['mur', 'roche']);
const BORDES = new Set(['mur', 'roche', 'gouffre', 'eau', 'eau-prof', 'lave', 'tapis']);
const EMOJI_FONT = '"Segoe UI Emoji","Apple Color Emoji","Noto Color Emoji",sans-serif';

// ------------------------------------------------------------------ textures procédurales
function mulberry(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6D2B79F5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const cellHash = (c, r) => ((c * 73856093) ^ (r * 19349663)) >>> 0;

function speckle(ctx, s, R, n, colors, min, max) {
  for (let i = 0; i < n; i++) {
    ctx.fillStyle = colors[Math.floor(R() * colors.length)];
    const z = (min + R() * (max - min)) * s;
    ctx.beginPath();
    ctx.ellipse(R() * s, R() * s, z, z * (0.6 + R() * 0.5), R() * Math.PI, 0, Math.PI * 2);
    ctx.fill();
  }
}
function vary(hex, amount, R) {
  const n = parseInt(hex.slice(1), 16);
  const d = Math.round((R() * 2 - 1) * amount);
  const ch = (v) => Math.max(0, Math.min(255, v + d));
  return `rgb(${ch(n >> 16)},${ch((n >> 8) & 255)},${ch(n & 255)})`;
}
function roundRect(ctx, x, y, w, h, rad) {
  ctx.beginPath();
  ctx.moveTo(x + rad, y);
  ctx.arcTo(x + w, y, x + w, y + h, rad);
  ctx.arcTo(x + w, y + h, x, y + h, rad);
  ctx.arcTo(x, y + h, x, y, rad);
  ctx.arcTo(x, y, x + w, y, rad);
  ctx.closePath();
}

function paintTerrain(ctx, key, s, R) {
  const fill = (c) => { ctx.fillStyle = c; ctx.fillRect(0, 0, s, s); };
  const lw = Math.max(1, s / 40);
  switch (key) {
    case 'herbe':
      fill('#5d8a38');
      speckle(ctx, s, R, 14, ['#6a9a42', '#527c30', '#64923d'], 0.06, 0.16);
      ctx.strokeStyle = '#80b052';
      ctx.lineWidth = lw;
      for (let i = 0; i < 10; i++) {
        const x = R() * s; const y = R() * s; const h = s * (0.05 + R() * 0.06);
        ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x + (R() - 0.5) * h * 0.8, y - h); ctx.stroke();
      }
      break;
    case 'foret':
      fill('#3b5a27');
      speckle(ctx, s, R, 10, ['#2f4b1e', '#46682d'], 0.1, 0.22);
      speckle(ctx, s, R, 12, ['#5b7f34', '#6d5a2c', '#7a5a30'], 0.03, 0.06);
      break;
    case 'terre':
      fill('#87673f');
      speckle(ctx, s, R, 16, ['#7a5b37', '#957450', '#6f5331'], 0.05, 0.14);
      speckle(ctx, s, R, 5, ['#9a9187', '#6e6a64'], 0.02, 0.035);
      break;
    case 'sable':
      fill('#dcc38c');
      speckle(ctx, s, R, 18, ['#e7d2a2', '#ccb175', '#d4b981'], 0.04, 0.1);
      ctx.strokeStyle = '#c6aa6e88';
      ctx.lineWidth = lw;
      for (let i = 0; i < 2; i++) {
        const y = s * (0.25 + i * 0.45 + R() * 0.1);
        ctx.beginPath(); ctx.moveTo(0, y); ctx.quadraticCurveTo(s / 2, y + (R() - 0.5) * s * 0.2, s, y); ctx.stroke();
      }
      break;
    case 'neige':
      fill('#edf2f6');
      speckle(ctx, s, R, 12, ['#dde7ef', '#ffffff', '#d3dfea'], 0.06, 0.16);
      break;
    case 'paves': {
      fill('#4d4943');
      const n = 3; const z = s / n;
      for (let y = 0; y < n; y++) {
        for (let x = 0; x < n; x++) {
          ctx.fillStyle = vary('#86817a', 18, R);
          const j = z * 0.08;
          roundRect(ctx, x * z + j + (R() - 0.5) * j, y * z + j + (R() - 0.5) * j, z - 2 * j, z - 2 * j, z * 0.25);
          ctx.fill();
        }
      }
      break;
    }
    case 'dalles': {
      fill('#2e2c31');
      const z = s / 2;
      for (let y = 0; y < 2; y++) {
        for (let x = 0; x < 2; x++) {
          ctx.fillStyle = vary('#615e66', 12, R);
          ctx.fillRect(x * z + lw, y * z + lw, z - 2 * lw, z - 2 * lw);
        }
      }
      speckle(ctx, s, R, 6, ['#55525a', '#6d6a72'], 0.03, 0.08);
      if (R() < 0.5) {
        ctx.strokeStyle = '#2a282d'; ctx.lineWidth = lw;
        ctx.beginPath(); let x = R() * s; let y = R() * s; ctx.moveTo(x, y);
        for (let i = 0; i < 3; i++) { x += (R() - 0.5) * s * 0.3; y += (R() - 0.5) * s * 0.3; ctx.lineTo(x, y); }
        ctx.stroke();
      }
      break;
    }
    case 'plancher': {
      const n = 4; const z = s / n;
      for (let i = 0; i < n; i++) {
        ctx.fillStyle = vary('#8a5d33', 14, R);
        ctx.fillRect(0, i * z, s, z);
        ctx.strokeStyle = '#00000022'; ctx.lineWidth = lw;
        ctx.beginPath(); ctx.moveTo(0, i * z + z * (0.3 + R() * 0.4)); ctx.lineTo(s, i * z + z * (0.3 + R() * 0.4)); ctx.stroke();
        ctx.fillStyle = '#3b2412';
        ctx.fillRect(0, i * z, s, lw);
        const cut = R() * s;
        ctx.fillRect(cut, i * z, lw, z);
      }
      break;
    }
    case 'tapis':
      fill('#7d1a20');
      speckle(ctx, s, R, 6, ['#8f2329', '#6e161b'], 0.06, 0.14);
      ctx.fillStyle = '#c9a24a';
      ctx.beginPath(); ctx.moveTo(s / 2, s * 0.3); ctx.lineTo(s * 0.7, s / 2); ctx.lineTo(s / 2, s * 0.7); ctx.lineTo(s * 0.3, s / 2); ctx.closePath(); ctx.fill();
      ctx.fillStyle = '#7d1a20';
      ctx.beginPath(); ctx.arc(s / 2, s / 2, s * 0.07, 0, Math.PI * 2); ctx.fill();
      break;
    case 'eau':
    case 'eau-prof': {
      const deep = key === 'eau-prof';
      const g = ctx.createLinearGradient(0, 0, s, s);
      g.addColorStop(0, deep ? '#1b4366' : '#2f6e99');
      g.addColorStop(1, deep ? '#153656' : '#3a82ad');
      fill(g);
      ctx.strokeStyle = deep ? '#3a6c9666' : '#8cc6e699';
      ctx.lineWidth = lw * 1.3;
      for (let i = 0; i < (deep ? 2 : 3); i++) {
        const x = R() * s * 0.7; const y = s * (0.2 + R() * 0.65); const w = s * (0.15 + R() * 0.15);
        ctx.beginPath(); ctx.moveTo(x, y); ctx.quadraticCurveTo(x + w / 2, y - w * 0.35, x + w, y); ctx.stroke();
      }
      break;
    }
    case 'marais':
      fill('#4b5a32');
      speckle(ctx, s, R, 4, ['#36483a', '#3d4f40'], 0.12, 0.22);
      ctx.strokeStyle = '#7d8c3e'; ctx.lineWidth = lw;
      for (let i = 0; i < 6; i++) {
        const x = R() * s; const y = R() * s;
        ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x + (R() - 0.5) * s * 0.06, y - s * 0.12); ctx.stroke();
      }
      break;
    case 'glace': {
      const g = ctx.createLinearGradient(0, 0, s, s);
      g.addColorStop(0, '#d6eef8'); g.addColorStop(1, '#a9d6ea');
      fill(g);
      ctx.strokeStyle = '#ffffffcc'; ctx.lineWidth = lw;
      ctx.beginPath(); let x = R() * s; let y = 0; ctx.moveTo(x, y);
      for (let i = 0; i < 4; i++) { x += (R() - 0.5) * s * 0.4; y += s * 0.27; ctx.lineTo(x, y); }
      ctx.stroke();
      break;
    }
    case 'lave': {
      fill('#3a1307');
      for (let i = 0; i < 5; i++) {
        const x = R() * s; const y = R() * s; const z = s * (0.12 + R() * 0.2);
        const g = ctx.createRadialGradient(x, y, 0, x, y, z);
        g.addColorStop(0, '#ffd23a'); g.addColorStop(0.4, '#ff7a1a'); g.addColorStop(1, '#ff3d0000');
        ctx.fillStyle = g; ctx.fillRect(0, 0, s, s);
      }
      break;
    }
    case 'gouffre': {
      const g = ctx.createRadialGradient(s / 2, s / 2, 0, s / 2, s / 2, s * 0.75);
      g.addColorStop(0, '#000'); g.addColorStop(1, '#17110d');
      fill(g);
      break;
    }
    case 'mur': {
      fill('#2f2b27');
      const rows = 3; const h = s / rows;
      for (let y = 0; y < rows; y++) {
        const off = y % 2 ? -s / 4 : 0;
        for (let x = off; x < s; x += s / 2) {
          ctx.fillStyle = vary('#77716a', 14, R);
          ctx.fillRect(x + lw, y * h + lw, s / 2 - 2 * lw, h - 2 * lw);
          ctx.fillStyle = '#ffffff1a';
          ctx.fillRect(x + lw, y * h + lw, s / 2 - 2 * lw, lw * 1.5);
        }
      }
      break;
    }
    case 'roche':
      fill('#463c34');
      speckle(ctx, s, R, 7, ['#56493f', '#3a312a', '#5e5147'], 0.12, 0.24);
      speckle(ctx, s, R, 6, ['#6b5d52'], 0.03, 0.06);
      break;
    default:
      fill('#888');
  }
}

const texCache = new Map();
function terrainTexture(key, px, v) {
  const id = `${key}:${px}:${v}`;
  let t = texCache.get(id);
  if (!t) {
    t = document.createElement('canvas');
    t.width = t.height = px;
    paintTerrain(t.getContext('2d'), key, px, mulberry(v * 7919 + key.charCodeAt(0) * 131 + key.length));
    texCache.set(id, t);
    if (texCache.size > 500) texCache.delete(texCache.keys().next().value);
  }
  return t;
}

// ------------------------------------------------------------------ rendu de la carte
let tilesDrawn = null; // ce qui est actuellement dessiné : { key, px, cols, rows, sol, deco }

function mapGrid() {
  const sc = S.scene;
  const cell = sc.cellule;
  return sc.battle
    ? { cols: sc.cols, rows: sc.rows, cell }
    : { cols: Math.ceil(imgSize.w / cell), rows: Math.ceil(imgSize.h / cell), cell };
}

function solAt(c, r) {
  const g = tilesDrawn;
  if (c < 0 || r < 0 || c >= g.cols || r >= g.rows) return null;
  return S.scene.sol[c + ',' + r] || S.scene.fond || null;
}

function drawCell(ctx, c, r) {
  const s = tilesDrawn.px;
  const x = c * s;
  const y = r * s;
  ctx.clearRect(x, y, s, s);
  const key = solAt(c, r);
  if (key) {
    ctx.drawImage(terrainTexture(key, s, cellHash(c, r) % 4), x, y);
    const n = [[0, -1], [1, 0], [0, 1], [-1, 0]].map(([dc, dr]) => solAt(c + dc, r + dr));
    const e = Math.max(1, s * 0.07);
    if (BORDES.has(key)) {
      // bord sombre là où le terrain change (rives, murs, gouffres)
      ctx.fillStyle = MURS.has(key) ? '#00000080' : '#0000004d';
      if (n[0] && n[0] !== key) ctx.fillRect(x, y, s, e);
      if (n[1] && n[1] !== key) ctx.fillRect(x + s - e, y, e, s);
      if (n[2] && n[2] !== key) ctx.fillRect(x, y + s - e, s, e);
      if (n[3] && n[3] !== key) ctx.fillRect(x, y, e, s);
    }
    if (!MURS.has(key)) {
      // ombre portée des murs voisins
      const d = s * 0.22;
      const shade = (x0, y0, x1, y1, rx, ry, rw, rh) => {
        const g = ctx.createLinearGradient(x0, y0, x1, y1);
        g.addColorStop(0, '#00000066'); g.addColorStop(1, '#00000000');
        ctx.fillStyle = g; ctx.fillRect(rx, ry, rw, rh);
      };
      if (MURS.has(n[0])) shade(x, y, x, y + d, x, y, s, d);
      if (MURS.has(n[1])) shade(x + s, y, x + s - d, y, x + s - d, y, d, s);
      if (MURS.has(n[2])) shade(x, y + s, x, y + s - d, x, y + s - d, s, d);
      if (MURS.has(n[3])) shade(x, y, x + d, y, x, y, d, s);
    }
  }
  const deco = S.scene.deco[c + ',' + r];
  if (deco) {
    ctx.save();
    ctx.font = `${Math.round(s * 0.74)}px ${EMOJI_FONT}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.shadowColor = '#000000a0';
    ctx.shadowBlur = s * 0.08;
    ctx.shadowOffsetY = s * 0.04;
    ctx.fillText(deco, x + s / 2, y + s / 2 + s * 0.04);
    ctx.restore();
  }
}

/** Redessine la carte : en entier si la scène a changé, sinon seulement les cases modifiées. */
function renderTiles() {
  const cv = $('#map-tiles');
  const sc = S && S.scene;
  if (!sc || !sc.id) { cv.hidden = true; tilesDrawn = null; return; }
  const { cols, rows, cell } = mapGrid();
  const px = Math.max(8, Math.min(cell, Math.floor(3072 / Math.max(cols, rows))));
  const key = [sc.id, cols, rows, px, sc.fond || ''].join('|');
  cv.hidden = false;
  cv.style.width = (cols * cell / imgSize.w) * 100 + '%';
  cv.style.height = (rows * cell / imgSize.h) * 100 + '%';
  const ctx = cv.getContext('2d');
  if (!tilesDrawn || tilesDrawn.key !== key) {
    cv.width = cols * px;
    cv.height = rows * px;
    tilesDrawn = { key, px, cols, rows, sol: {}, deco: {} };
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) drawCell(ctx, c, r);
  } else {
    const dirty = new Set();
    const diff = (now, before, neighbours) => {
      for (const k of new Set([...Object.keys(now), ...Object.keys(before)])) {
        if (now[k] === before[k]) continue;
        dirty.add(k);
        if (neighbours) {
          const [c, r] = k.split(',').map(Number);
          dirty.add(`${c},${r - 1}`).add(`${c + 1},${r}`).add(`${c},${r + 1}`).add(`${c - 1},${r}`);
        }
      }
    };
    diff(sc.sol, tilesDrawn.sol, true);
    diff(sc.deco, tilesDrawn.deco, false);
    for (const k of dirty) {
      const [c, r] = k.split(',').map(Number);
      if (c >= 0 && r >= 0 && c < cols && r < rows) drawCell(ctx, c, r);
    }
  }
  tilesDrawn.sol = { ...sc.sol };
  tilesDrawn.deco = { ...sc.deco };
}

socket.on('map:patch', ({ sceneId, calque, cells } = {}) => {
  if (!S || S.scene.id !== sceneId || !Array.isArray(cells)) return;
  const layer = S.scene[calque];
  if (!layer) return;
  for (const [c, r, v] of cells) { if (v == null) delete layer[c + ',' + r]; else layer[c + ',' + r] = v; }
  renderTiles();
});

// ------------------------------------------------------------------ éditeur (MJ)
const edit = { on: false, calque: 'sol', val: 'dalles', outil: 'pinceau', taille: 1 };
let paintQueue = [];
let paintTimer = null;

function flushPaint() {
  clearTimeout(paintTimer);
  paintTimer = null;
  const byLayer = {};
  for (const [calque, c, r, v] of paintQueue) (byLayer[calque] = byLayer[calque] || []).push([c, r, v]);
  paintQueue = [];
  for (const [calque, cells] of Object.entries(byLayer)) socket.emit('map:paint', { calque, cells });
}

function applyCells(cells, val) {
  const layer = S.scene[edit.calque];
  const { cols, rows } = mapGrid();
  let changed = false;
  for (const [c, r] of cells) {
    if (c < 0 || r < 0 || c >= cols || r >= rows) continue;
    const k = c + ',' + r;
    if ((layer[k] || null) === val) continue;
    if (val == null) delete layer[k]; else layer[k] = val;
    paintQueue.push([edit.calque, c, r, val]);
    changed = true;
  }
  if (!changed) return;
  renderTiles();
  if (paintQueue.length > 3000) flushPaint();
  else if (!paintTimer) paintTimer = setTimeout(flushPaint, 70);
}

function brushCells(c, r) {
  const n = edit.taille;
  const o = Math.floor((n - 1) / 2);
  const out = [];
  for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) out.push([c - o + x, r - o + y]);
  return out;
}
function rectCells(a, b) {
  const out = [];
  for (let r = Math.min(a[1], b[1]); r <= Math.max(a[1], b[1]); r++) {
    for (let c = Math.min(a[0], b[0]); c <= Math.max(a[0], b[0]); c++) out.push([c, r]);
  }
  return out;
}
function fillCells(c0, r0) {
  const { cols, rows } = mapGrid();
  const get = (c, r) => (edit.calque === 'sol' ? solAt(c, r) : S.scene.deco[c + ',' + r] || null);
  const target = get(c0, r0);
  const seen = new Set([c0 + ',' + r0]);
  const stack = [[c0, r0]];
  const out = [];
  while (stack.length && out.length < 10000) {
    const [c, r] = stack.pop();
    out.push([c, r]);
    for (const [dc, dr] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const nc = c + dc;
      const nr = r + dr;
      const k = nc + ',' + nr;
      if (nc < 0 || nr < 0 || nc >= cols || nr >= rows || seen.has(k) || get(nc, nr) !== target) continue;
      seen.add(k);
      stack.push([nc, nr]);
    }
  }
  return out;
}
function lineCells(a, b) {
  const out = [];
  const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]), 1);
  for (let i = 0; i <= n; i++) out.push([Math.round(a[0] + (b[0] - a[0]) * i / n), Math.round(a[1] + (b[1] - a[1]) * i / n)]);
  return out;
}

function cellFromEvent(ev) {
  const r = $('#map').getBoundingClientRect();
  const cell = S.scene.cellule;
  return [
    Math.floor(((ev.clientX - r.left) / r.width) * imgSize.w / cell),
    Math.floor(((ev.clientY - r.top) / r.height) * imgSize.h / cell),
  ];
}
function showCursor(a, b = a) {
  const cur = $('#map-cursor');
  const cell = S.scene.cellule;
  let c0 = Math.min(a[0], b[0]);
  let r0 = Math.min(a[1], b[1]);
  let w = Math.abs(b[0] - a[0]) + 1;
  let h = Math.abs(b[1] - a[1]) + 1;
  if (edit.outil === 'pinceau' && a === b) { const o = Math.floor((edit.taille - 1) / 2); c0 -= o; r0 -= o; w = h = edit.taille; }
  if (edit.outil === 'remplir' && a === b) w = h = 1;
  Object.assign(cur.style, {
    left: (c0 * cell / imgSize.w) * 100 + '%', top: (r0 * cell / imgSize.h) * 100 + '%',
    width: (w * cell / imgSize.w) * 100 + '%', height: (h * cell / imgSize.h) * 100 + '%',
  });
  cur.hidden = false;
}

const mapEl = $('#map');
mapEl.addEventListener('contextmenu', (ev) => { if (edit.on) ev.preventDefault(); });
mapEl.addEventListener('pointerleave', () => { $('#map-cursor').hidden = true; });
mapEl.addEventListener('pointermove', (ev) => { if (edit.on && S.scene.id && !mapEl.hasPointerCapture(ev.pointerId)) showCursor(cellFromEvent(ev)); });
mapEl.addEventListener('pointerdown', (ev) => {
  if (!edit.on || !S.scene.id || ev.button > 2) return;
  ev.preventDefault();
  const val = ev.button === 2 ? null : edit.val; // clic droit = gomme
  const start = cellFromEvent(ev);
  if (edit.outil === 'remplir') { applyCells(fillCells(...start), val); flushPaint(); return; }
  mapEl.setPointerCapture(ev.pointerId);
  let last = start;
  if (edit.outil === 'pinceau') applyCells(brushCells(...start), val);
  showCursor(start);
  const move = (pe) => {
    const p = cellFromEvent(pe);
    if (edit.outil === 'pinceau') {
      if (p[0] !== last[0] || p[1] !== last[1]) applyCells(lineCells(last, p).flatMap((q) => brushCells(...q)), val);
      showCursor(p);
    } else {
      showCursor(start, p);
    }
    last = p;
  };
  const up = () => {
    mapEl.removeEventListener('pointermove', move);
    mapEl.removeEventListener('pointerup', up);
    mapEl.removeEventListener('pointercancel', up);
    if (edit.outil === 'rect') applyCells(rectCells(start, last), val);
    flushPaint();
    showCursor(last);
  };
  mapEl.addEventListener('pointermove', move);
  mapEl.addEventListener('pointerup', up);
  mapEl.addEventListener('pointercancel', up);
});

function setEditing(on) {
  edit.on = on;
  flushPaint();
  $('#map-editor').hidden = !on;
  mapEl.classList.toggle('editing', on);
  $('#map-edit').classList.toggle('active', on);
  $('#map-cursor').hidden = true;
  if (on) renderMapEditor();
}
$('#map-edit').addEventListener('click', () => setEditing(!edit.on));
$('#me-close').addEventListener('click', () => setEditing(false));
$('#mj-battlemap').addEventListener('click', () => { setEditing(true); $('#me-create').open = true; $('#bm-titre').focus(); });

// palettes
const swatchUrl = (key) => terrainTexture(key, 40, 1).toDataURL();
$('#me-sols').append(...SOLS.map(([k, nom]) => el('button', {
  type: 'button', class: 'tile-btn', title: nom, 'data-val': k, style: { backgroundImage: `url(${swatchUrl(k)})` },
  onclick: () => { edit.calque = 'sol'; edit.val = k; renderMapEditor(); },
})));
$('#me-decos').append(...DECOS.map(([k, nom]) => el('button', {
  type: 'button', class: 'tile-btn deco', title: nom, 'data-val': k,
  onclick: () => { edit.calque = 'deco'; edit.val = k; renderMapEditor(); },
}, k)));
$$('#map-editor [data-calque-gomme]').forEach((b) => b.addEventListener('click', () => {
  edit.calque = b.dataset.calqueGomme; edit.val = null; renderMapEditor();
}));
$$('#map-editor [data-outil]').forEach((b) => b.addEventListener('click', () => { edit.outil = b.dataset.outil; renderMapEditor(); }));
$$('#map-editor [data-taille]').forEach((b) => b.addEventListener('click', () => { edit.taille = Number(b.dataset.taille); renderMapEditor(); }));
for (const sel of [$('#me-fond'), $('#bm-fond')]) {
  sel.append(...SOLS.filter(([k]) => !MURS.has(k) && k !== 'gouffre').map(([k, nom]) => el('option', { value: k, text: nom })));
}
$('#me-fond').addEventListener('change', (ev) => socket.emit('map:fond', { fond: ev.target.value }));
$('#me-taille').addEventListener('submit', (ev) => {
  ev.preventDefault();
  socket.emit('map:taille', { cols: $('#me-cols').value, rows: $('#me-rows').value });
});
$('#me-clear-deco').addEventListener('click', () => { if (confirm('Retirer tout le décor de cette carte ?')) socket.emit('map:effacer', { calque: 'deco' }); });
$('#me-clear-sol').addEventListener('click', () => { if (confirm('Effacer tout le sol peint de cette carte ?')) socket.emit('map:effacer', { calque: 'sol' }); });
$('#bm-form').addEventListener('submit', (ev) => {
  ev.preventDefault();
  socket.emit('scene:battlemap', {
    titre: $('#bm-titre').value, cols: $('#bm-cols').value, rows: $('#bm-rows').value, fond: $('#bm-fond').value,
  }, (res) => {
    if (res && !res.ok) return toast(res.error);
    $('#bm-titre').value = '';
    $('#me-create').open = false;
    toast('Battle map créée et affichée aux joueurs. Peins le sol, puis ajoute le décor !');
  });
});

function renderMapEditor() {
  if (!edit.on || !S) return;
  const sc = S.scene;
  $('#me-tools').hidden = !sc.id;
  $('#me-none').hidden = !!sc.id;
  if (!sc.id) $('#me-create').open = true;
  $('#me-scene').textContent = sc.id ? sc.titre : '';
  $$('#map-editor .tile-btn').forEach((b) => {
    const isSol = !b.classList.contains('deco');
    b.classList.toggle('active', (edit.calque === 'sol') === isSol && b.dataset.val === edit.val);
  });
  $$('#map-editor [data-calque-gomme]').forEach((b) => b.classList.toggle('active', edit.val == null && b.dataset.calqueGomme === edit.calque));
  $$('#map-editor [data-outil]').forEach((b) => b.classList.toggle('active', b.dataset.outil === edit.outil));
  $$('#map-editor [data-taille]').forEach((b) => b.classList.toggle('active', Number(b.dataset.taille) === edit.taille));
  $('#me-brush').hidden = edit.outil !== 'pinceau';
  $('#me-battle').hidden = !sc.battle;
  const fond = $('#me-fond');
  if (document.activeElement !== fond) fond.value = sc.fond || 'herbe';
  if (sc.battle && !$('#me-taille').contains(document.activeElement)) {
    $('#me-cols').value = sc.cols;
    $('#me-rows').value = sc.rows;
  }
  const v = edit.val == null ? 'Gomme' : edit.calque === 'sol' ? (SOLS.find(([k]) => k === edit.val) || [])[1] : (DECOS.find(([k]) => k === edit.val) || [])[1];
  $('#me-current').textContent = `${edit.calque === 'sol' ? 'Sol' : 'Décor'} : ${v || ''}`;
}
