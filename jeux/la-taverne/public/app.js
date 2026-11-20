'use strict';
/* La Taverne — client */

// ------------------------------------------------------------------ utilitaires
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
function el(tag, props = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') e.className = v;
    else if (k === 'text') e.textContent = v;
    else if (k === 'style' && typeof v === 'object') Object.assign(e.style, v);
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
    else if (k === 'value') e.value = v;
    else if (k === 'checked') e.checked = !!v;
    else e.setAttribute(k, v === true ? '' : v);
  }
  for (const kid of kids.flat()) {
    if (kid == null || kid === false) continue;
    e.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
  }
  return e;
}
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const mod = (v) => Math.floor(((Number(v) || 10) - 10) / 2);
const fmtMod = (m) => (m >= 0 ? '+' : '') + m;
const maitrise = (niv) => 2 + Math.floor(((Number(niv) || 1) - 1) / 4);
const initials = (s) => String(s || '?').split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join('').toUpperCase();
const STATS = [['for', 'Force'], ['dex', 'Dextérité'], ['con', 'Constitution'], ['int', 'Intelligence'], ['sag', 'Sagesse'], ['cha', 'Charisme']];

let toastTimer;
function toast(msg) {
  const t = $('#toast');
  t.textContent = msg;
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.hidden = true; }, 3500);
}

// ------------------------------------------------------------------ session
const KEY = 'taverne-session';
const store = {
  get() { try { return JSON.parse(localStorage.getItem(KEY)); } catch { return null; } },
  set(v) { try { localStorage.setItem(KEY, JSON.stringify(v)); } catch { /* navigation privée */ } },
  clear() { try { localStorage.removeItem(KEY); } catch { /* idem */ } },
};

const socket = io();
let session = store.get();
let me = { role: null, charId: null };
let S = null; // état reçu du serveur
let ficheId = null;

function showLogin(error = '') {
  $('#app').hidden = true;
  $('#login').hidden = false;
  $('#login-error').textContent = error;
}

function join(sess) {
  if (!socket.connected) socket.connect();
  socket.emit('join', sess, (res) => {
    if (!res || !res.ok) {
      session = null;
      store.clear();
      return showLogin(res ? res.error : 'Erreur de connexion');
    }
    session = sess;
    store.set(session);
    me = { role: res.role, charId: res.charId };
    document.body.classList.toggle('is-mj', me.role === 'mj');
    $('#login').hidden = true;
    $('#app').hidden = false;
    const log = $('#log');
    log.replaceChildren(...res.log.map(renderEntry));
    log.scrollTop = log.scrollHeight;
    if (me.role !== 'mj') ficheId = me.charId;
    if (S) renderAll();
  });
}

socket.on('connect', () => {
  $('#conn').classList.add('on');
  if (session) join(session);
});
socket.on('disconnect', () => $('#conn').classList.remove('on'));
socket.on('kicked', (msg) => { session = null; store.clear(); showLogin(msg); });

socket.on('state', (s) => { S = s; renderAll(); });
socket.on('log', (e) => (e.type === 'roll' ? onRollEntry(e) : addLogEntry(e)));
socket.on('log:reset', () => $('#log').replaceChildren());
socket.on('token:moved', ({ id, x, y }) => {
  if (!S || !S.tokens[id]) return;
  S.tokens[id].x = x;
  S.tokens[id].y = y;
  if (dragging === id) return;
  const e = tokenEls.get(id);
  if (e) { e.style.left = x + '%'; e.style.top = y + '%'; }
});

// Formulaires de connexion
$$('[data-login]').forEach((b) => b.addEventListener('click', () => {
  $$('[data-login]').forEach((x) => x.classList.toggle('active', x === b));
  $('#form-player').hidden = b.dataset.login !== 'player';
  $('#form-mj').hidden = b.dataset.login !== 'mj';
  $('#login-error').textContent = '';
}));
$('#form-player').addEventListener('submit', (ev) => {
  ev.preventDefault();
  join({ role: 'player', name: $('#in-name').value.trim() });
});
$('#form-mj').addEventListener('submit', (ev) => {
  ev.preventDefault();
  join({ role: 'mj', password: $('#in-pass').value });
});
$('#btn-logout').addEventListener('click', () => {
  store.clear();
  location.reload();
});

// ------------------------------------------------------------------ onglets
function showTab(name) {
  $$('.side-tabs button').forEach((b) => b.classList.toggle('active', b.dataset.tab === name));
  $$('.tab').forEach((t) => { t.hidden = t.id !== 'tab-' + name; });
  if (name === 'chat') { const l = $('#log'); l.scrollTop = l.scrollHeight; }
}
$$('.side-tabs button').forEach((b) => b.addEventListener('click', () => showTab(b.dataset.tab)));

// ------------------------------------------------------------------ rendu global
function renderAll() {
  if (!S || !me.role) return;
  if (me.role === 'player') {
    const c = S.characters[me.charId];
    if (c && session && session.name !== c.name) { session.name = c.name; store.set(session); }
    $('#me-label').textContent = c ? c.name : '';
  } else {
    $('#me-label').textContent = 'Maître du jeu';
  }
  $('#mj-status').textContent = me.role === 'mj' ? '' : (S.mjOnline ? 'MJ présent' : 'MJ absent');
  $('#mj-status').hidden = me.role === 'mj';
  $('#scene-title').textContent = S.scene.titre || '';
  renderMap();
  renderParty();
  renderSheet();
  renderPion();
  renderInit();
  renderChatTargets();
  if (me.role === 'mj') { renderMj(); renderSlides(); }
  renderProjection();
}

// ------------------------------------------------------------------ carte
const BLANK = { w: 1400, h: 900 };
let imgSize = { ...BLANK };
let currentImg;
let zoom = 1;
let dragging = null;
const tokenEls = new Map();

let currentKey;
function renderMap() {
  const img = $('#map-img');
  const sc = S.scene;
  const key = sc.battle ? 'bm:' + sc.id : sc.image || '';
  const map = $('#map');
  map.classList.toggle('battle', !!sc.battle);
  map.classList.toggle('blank', !sc.id);
  if (sc.battle) {
    // battle map : la taille vient du nombre de cases
    const w = sc.cols * sc.cellule;
    const h = sc.rows * sc.cellule;
    const fresh = key !== currentKey || w !== imgSize.w || h !== imgSize.h;
    if (key !== currentKey) { currentImg = null; img.removeAttribute('src'); img.hidden = true; }
    currentKey = key;
    imgSize = { w, h };
    if (fresh) fitZoom();
  } else if (key !== currentKey) {
    currentKey = key;
    currentImg = sc.image;
    if (currentImg) {
      img.onload = () => { imgSize = { w: img.naturalWidth, h: img.naturalHeight }; fitZoom(); renderTiles(); };
      img.src = currentImg;
      img.hidden = false;
    } else {
      img.removeAttribute('src');
      img.hidden = true;
      imgSize = { ...BLANK };
      fitZoom();
    }
  }
  layoutMap();
  renderTiles();
  if (me.role === 'mj') renderMapEditor();
}

function layoutMap() {
  const map = $('#map');
  map.style.width = imgSize.w * zoom + 'px';
  map.style.height = imgSize.h * zoom + 'px';
  const grid = $('#map-grid');
  grid.hidden = !S.scene.grille;
  const c = S.scene.cellule * zoom;
  grid.style.backgroundSize = `${c}px ${c}px`;
  $('#zoom-val').textContent = Math.round(zoom * 100) + '%';
  renderTokens();
}

function setZoom(z) { zoom = clamp(z, 0.1, 4); if (S) layoutMap(); }
function fitZoom() {
  const vp = $('#map-viewport');
  setZoom(Math.min(vp.clientWidth / imgSize.w, vp.clientHeight / imgSize.h) || 1);
}
$('#zoom-in').addEventListener('click', () => setZoom(zoom * 1.25));
$('#zoom-out').addEventListener('click', () => setZoom(zoom / 1.25));
$('#zoom-fit').addEventListener('click', fitZoom);
$('#map-viewport').addEventListener('wheel', (ev) => {
  if (!ev.ctrlKey) return;
  ev.preventDefault();
  setZoom(zoom * (ev.deltaY < 0 ? 1.1 : 1 / 1.1));
}, { passive: false });

const canMove = (t) => me.role === 'mj' || (t.ownerId && t.ownerId === me.charId);

function activeTokenIds() {
  const e = S.initiative.entries.find((x) => x.id === S.initiative.actif);
  if (!e) return new Set();
  return new Set(Object.values(S.tokens).filter((t) => (e.charId && t.ownerId === e.charId) || t.id === e.tokenId).map((t) => t.id));
}

function renderTokens() {
  const layer = $('#tokens');
  const cell = S.scene.cellule * zoom;
  const active = activeTokenIds();
  const seen = new Set();
  for (const t of Object.values(S.tokens)) {
    seen.add(t.id);
    let e = tokenEls.get(t.id);
    if (!e) {
      e = el('div', { class: 'token' }, tokenVisual(), el('span', { class: 'token-label' }));
      e.dataset.id = t.id;
      e.addEventListener('pointerdown', onTokenDown);
      e.addEventListener('dblclick', () => { const tk = S.tokens[e.dataset.id]; if (tk && canMove(tk)) openPion(tk.id); });
      layer.append(e);
      tokenEls.set(t.id, e);
    }
    const size = cell * t.taille * 0.92;
    e.style.width = e.style.height = size + 'px';
    if (dragging !== t.id) { e.style.left = t.x + '%'; e.style.top = t.y + '%'; }
    applySkin(e.querySelector('.tk'), t, size);
    e.querySelector('.token-label').textContent = t.label;
    e.title = t.label + (t.cache ? ' (caché aux joueurs)' : '');
    e.classList.toggle('is-hidden', !!t.cache);
    e.classList.toggle('movable', canMove(t));
    e.classList.toggle('mine', t.ownerId === me.charId);
    e.classList.toggle('turn', active.has(t.id));
  }
  for (const [id, e] of tokenEls) if (!seen.has(id)) { e.remove(); tokenEls.delete(id); }
}

function snap(p, t) {
  if (!S.scene.grille) return p;
  const cell = S.scene.cellule;
  const off = t.taille % 2 ? 0.5 : 0;
  const sx = (Math.round((p.x / 100) * imgSize.w / cell - off) + off) * cell;
  const sy = (Math.round((p.y / 100) * imgSize.h / cell - off) + off) * cell;
  return { x: clamp((sx / imgSize.w) * 100, 0, 100), y: clamp((sy / imgSize.h) * 100, 0, 100) };
}

function onTokenDown(ev) {
  const e = ev.currentTarget;
  const id = e.dataset.id;
  const t = S.tokens[id];
  if (!t || !canMove(t) || ev.button > 0) return;
  ev.preventDefault();
  e.setPointerCapture(ev.pointerId);
  dragging = id;
  e.classList.add('dragging');
  let last = 0;
  const pos = (pe) => {
    const r = $('#map').getBoundingClientRect();
    return { x: clamp(((pe.clientX - r.left) / r.width) * 100, 0, 100), y: clamp(((pe.clientY - r.top) / r.height) * 100, 0, 100) };
  };
  const move = (pe) => {
    const p = pos(pe);
    e.style.left = p.x + '%';
    e.style.top = p.y + '%';
    const now = Date.now();
    if (now - last > 60) { last = now; socket.emit('token:move', { id, ...p }); }
  };
  const up = (pe) => {
    e.removeEventListener('pointermove', move);
    e.removeEventListener('pointerup', up);
    e.removeEventListener('pointercancel', up);
    const p = snap(pos(pe), t);
    e.style.left = p.x + '%';
    e.style.top = p.y + '%';
    socket.emit('token:move', { id, ...p });
    dragging = null;
    e.classList.remove('dragging');
  };
  e.addEventListener('pointermove', move);
  e.addEventListener('pointerup', up);
  e.addEventListener('pointercancel', up);
}

// ------------------------------------------------------------------ apparence des pions
const FORMES = [['socle', 'Debout'], ['rond', 'Rond'], ['carre', 'Carré'], ['hexa', 'Hexagone'], ['ecu', 'Écu']];
const CADRES = [['simple', 'Simple'], ['bronze', 'Bronze'], ['argent', 'Argent'], ['or', 'Or'], ['feu', 'Feu'],
  ['glace', 'Glace'], ['nature', 'Nature'], ['arcane', 'Arcane'], ['ombre', 'Ombre'], ['sacre', 'Sacré'], ['sang', 'Sang']];
const EMBLEMES = {
  Classes: [['⚔️', 'Guerrier'], ['🪓', 'Barbare'], ['🛡️', 'Paladin'], ['🙏', 'Clerc'], ['🧙', 'Magicien'], ['🔥', 'Ensorceleur'],
    ['👁️', 'Occultiste'], ['🗡️', 'Roublard'], ['🏹', 'Rôdeur'], ['🎻', 'Barde'], ['🌿', 'Druide'], ['👊', 'Moine']],
  Peuples: [['🧑', 'Humain'], ['🧝', 'Elfe'], ['🧔', 'Nain'], ['🧒', 'Halfelin'], ['😈', 'Tieffelin'], ['🐲', 'Drakéide'],
    ['👺', 'Orc'], ['🧚', 'Fée'], ['🧞', 'Génie']],
  Créatures: [['🐉', 'Dragon'], ['💀', 'Squelette'], ['🧟', 'Zombie'], ['👻', 'Spectre'], ['🧛', 'Vampire'], ['🐺', 'Loup'],
    ['🕷️', 'Araignée'], ['🦇', 'Chauve-souris'], ['🐍', 'Serpent'], ['🐗', 'Sanglier'], ['🐻', 'Ours'], ['🐀', 'Rat'],
    ['👹', 'Démon'], ['🧌', 'Troll'], ['🦅', 'Aigle'], ['🐴', 'Cheval'], ['🦂', 'Scorpion'], ['🐙', 'Kraken']],
  Divers: [['👑', 'Roi'], ['💰', 'Marchand'], ['📜', 'Érudit'], ['⚓', 'Marin'], ['🍺', 'Aubergiste'], ['🔮', 'Devin'],
    ['☠️', 'Pirate'], ['⭐', 'Héros'], ['🗿', 'Golem'], ['🌳', 'Sylvanien'], ['❄️', 'Élémentaire'], ['❓', 'Inconnu']],
};

/** Élément visuel d'un pion (cadre + visage), utilisé sur la carte, dans le groupe, l'initiative et l'éditeur. */
function tokenVisual() {
  return el('div', { class: 'tk' },
    el('div', { class: 'tk-socle' }),
    el('div', { class: 'tk-frame' }, el('div', { class: 'tk-face' }, el('span', { class: 'tk-txt' }), el('div', { class: 'tk-fig' }))),
    el('div', { class: 'tk-stand' }));
}
function setFigure(box, svg) {
  if (box.dataset.key !== svg) { box.innerHTML = svg; box.dataset.key = svg; }
}
function applySkin(v, t, size, override = {}) {
  const s = { ...(t.skin || {}), ...override };
  const isFig = s.contenu === 'figurine';
  const forme = s.forme === 'socle' && !isFig ? 'rond' : (s.forme || 'rond');
  v.className = `tk forme-${forme} cadre-${s.cadre || 'simple'}`;
  v.style.width = v.style.height = size + 'px';
  const face = v.querySelector('.tk-face');
  const txt = v.querySelector('.tk-txt');
  const figBox = v.querySelector('.tk-fig');
  const stand = v.querySelector('.tk-stand');
  face.style.backgroundColor = t.couleur;
  face.style.backgroundImage = '';
  txt.textContent = '';
  setFigure(figBox, isFig && forme !== 'socle' ? figurineSVG(s.figurine, 'buste') : '');
  setFigure(stand, isFig && forme === 'socle' ? figurineSVG(s.figurine, 'pied') : '');
  if (isFig) return v;
  if (s.contenu === 'image' && s.image) {
    face.style.backgroundImage = `url("${s.image}")`;
  } else {
    const emb = s.contenu === 'embleme';
    txt.textContent = emb ? s.embleme : initials(t.label);
    txt.classList.toggle('emoji', emb);
    txt.style.fontSize = Math.max(8, size * (emb ? 0.48 : 0.34)) + 'px';
  }
  return v;
}
// Petites vignettes (groupe, initiative…) : une figurine debout est montrée en buste dans un médaillon.
const miniToken = (t, size) => (t ? applySkin(tokenVisual(), t, size, t.skin && t.skin.forme === 'socle' ? { forme: 'rond' } : {}) : null);
const tokenOfChar = (charId) => Object.values(S.tokens).find((t) => t.ownerId === charId);

// ------------------------------------------------------------------ éditeur de pion
let pionId = null;

function editableTokens() {
  return Object.values(S.tokens).filter((t) => me.role === 'mj' || t.ownerId === me.charId);
}
function sendSkin(patch) {
  if (pionId) socket.emit('token:skin', { id: pionId, ...patch });
}
function openPion(id) { pionId = id; showTab('pion'); renderPion(); }

// Grille d'emblèmes (construite une fois)
$('#pion-emblemes').append(...Object.entries(EMBLEMES).flatMap(([groupe, list]) => [
  el('div', { class: 'emb-group', text: groupe }),
  el('div', { class: 'emb-grid' }, ...list.map(([e, nom]) => el('button', {
    type: 'button', class: 'emb', title: nom, 'data-emb': e,
    onclick: () => sendSkin({ skin: { contenu: 'embleme', embleme: e } }),
  }, e))),
]));

$$('#pion-contenu [data-contenu]').forEach((b) => b.addEventListener('click', () => {
  const t = S.tokens[pionId];
  if (!t) return;
  const c = b.dataset.contenu;
  if (c === 'image' && !t.skin.image) { $('#pion-file').click(); return; }
  sendSkin({ skin: c === 'figurine' && t.skin.contenu !== 'figurine' ? { contenu: c, forme: 'socle' } : { contenu: c } });
}));

// ---- créateur de petit personnage
function setFig(patch) {
  const t = S.tokens[pionId];
  if (!t) return;
  const f = { ...t.skin.figurine, ...patch };
  t.skin = { ...t.skin, contenu: 'figurine', figurine: f }; // affichage immédiat
  renderPion();
  renderTokens();
  sendSkin({ skin: { contenu: 'figurine', figurine: f } });
}
(function buildFigEditor() {
  $('#fig-modeles').append(...Object.keys(FIG.modeles).map((nom) => el('button', {
    type: 'button', class: 'chip', onclick: () => setFig(FIG.modeles[nom]),
  }, nom)));
  $('#fig-random').addEventListener('click', () => setFig(figRandom()));

  const row = (label, ...kids) => el('div', { class: 'fig-row' }, el('div', { class: 'fig-label', text: label }), el('div', { class: 'chips' }, ...kids));
  const chips = (k) => FIG.options[k].map(([v, nom]) => el('button', { type: 'button', class: 'chip', 'data-k': k, 'data-v': v, onclick: () => setFig({ [k]: v }) }, nom));
  const swatches = (k, list) => list.map((c) => el('button', {
    type: 'button', class: 'swatch-btn', 'data-k': k, 'data-v': c, title: c, style: { background: c }, onclick: () => setFig({ [k]: c }),
  }));
  const colorInput = (k) => el('input', { type: 'color', class: 'fig-color', 'data-kc': k, title: 'Autre couleur', onchange: (ev) => setFig({ [k]: ev.target.value }) });
  const toggle = (k, nom) => el('button', { type: 'button', class: 'chip', 'data-toggle': k, onclick: () => setFig({ [k]: !S.tokens[pionId].skin.figurine[k] }) }, nom);

  $('#fig-options').append(
    row('Peau', ...swatches('peau', FIG.peaux), colorInput('peau')),
    row('Coiffure', ...chips('cheveux')),
    row('Couleur des cheveux', ...swatches('couleurCheveux', FIG.cheveuxCouleurs), colorInput('couleurCheveux')),
    row('Barbe', ...chips('barbe')),
    row('Visage', ...chips('oreilles'), toggle('crocs', 'Crocs')),
    row('Tenue', ...chips('tenue'), colorInput('couleurTenue')),
    row('Couvre-chef', ...chips('tete')),
    row('Arme', ...chips('arme'), toggle('bouclier', '🛡 Bouclier')),
    row('Cape', toggle('cape', 'Porter une cape'), colorInput('couleurCape')),
  );
})();

function renderFigEditor(f) {
  for (const b of $$('#fig-options [data-k]')) b.classList.toggle('active', String(f[b.dataset.k]).toLowerCase() === b.dataset.v.toLowerCase());
  for (const b of $$('#fig-options [data-toggle]')) b.classList.toggle('active', !!f[b.dataset.toggle]);
  for (const i of $$('#fig-options [data-kc]')) if (document.activeElement !== i) i.value = f[i.dataset.kc];
}
$('#pion-select').addEventListener('change', (ev) => { pionId = ev.target.value; renderPion(); });
$('#pion-couleur').addEventListener('change', (ev) => sendSkin({ couleur: ev.target.value }));
$('#pion-label').addEventListener('change', (ev) => sendSkin({ label: ev.target.value }));
$('#pion-taille').addEventListener('change', (ev) => sendSkin({ taille: ev.target.value }));

function renderPion() {
  const list = editableTokens();
  if (!S.tokens[pionId] || !list.some((t) => t.id === pionId)) pionId = list[0] ? list[0].id : null;
  if (me.role === 'mj') {
    const sel = $('#pion-select');
    if (document.activeElement !== sel) {
      const sorted = list.slice().sort((a, b) => (!!a.ownerId === !!b.ownerId ? a.label.localeCompare(b.label) : a.ownerId ? -1 : 1));
      sel.replaceChildren(...sorted.map((t) => el('option', { value: t.id, text: t.label + (t.ownerId ? ' (joueur)' : '') })));
      sel.value = pionId || '';
    }
  }
  const t = S.tokens[pionId];
  $('#pion-editor').hidden = !t;
  $('#pion-empty').hidden = !!t;
  if (!t) return;

  $('#pion-preview').replaceChildren(applySkin(tokenVisual(), t, 120));
  $('#pion-preview-name').textContent = t.label;
  if (document.activeElement !== $('#pion-label')) $('#pion-label').value = t.label;
  $('#pion-label').disabled = !!t.ownerId;
  $('#pion-label').title = t.ownerId ? 'Le nom d\'un pion de joueur suit le nom du personnage' : '';
  $('#pion-taille').value = String(t.taille);
  $('#pion-couleur').value = t.couleur;

  const s = t.skin;
  $$('#pion-contenu [data-contenu]').forEach((b) => b.classList.toggle('active', b.dataset.contenu === s.contenu));
  $('#pion-emblemes').hidden = s.contenu !== 'embleme';
  $$('#pion-emblemes [data-emb]').forEach((b) => b.classList.toggle('active', b.dataset.emb === s.embleme));
  $('#pion-image-zone').hidden = s.contenu !== 'image' && $('#cropper').hidden;
  $('#pion-figurine').hidden = s.contenu !== 'figurine';
  if (s.contenu === 'figurine') renderFigEditor(s.figurine);

  $('#pion-formes').replaceChildren(...FORMES.filter(([k]) => k !== 'socle' || s.contenu === 'figurine').map(([k, nom]) => el('button', {
    type: 'button', class: 'choice' + (s.forme === k ? ' active' : ''), title: nom, onclick: () => sendSkin({ skin: { forme: k } }),
  }, applySkin(tokenVisual(), t, 44, { forme: k }), el('span', { text: nom }))));
  $('#pion-cadres').replaceChildren(...CADRES.map(([k, nom]) => el('button', {
    type: 'button', class: 'choice' + (s.cadre === k ? ' active' : ''), title: nom, onclick: () => sendSkin({ skin: { cadre: k } }),
  }, applySkin(tokenVisual(), t, 44, { cadre: k }), el('span', { text: nom }))));
}

// ---- recadrage du portrait
let crop = null;
const cropCanvas = $('#crop-canvas');

function cropGeometry(size) {
  const { img, zoom: z } = crop;
  const scale = Math.max(size / img.width, size / img.height) * z;
  const w = img.width * scale;
  const h = img.height * scale;
  const k = size / 240;
  crop.ox = clamp(crop.ox, -(w / k - 240) / 2, (w / k - 240) / 2);
  crop.oy = clamp(crop.oy, -(h / k - 240) / 2, (h / k - 240) / 2);
  return { x: (size - w) / 2 + crop.ox * k, y: (size - h) / 2 + crop.oy * k, w, h };
}
function drawCrop() {
  const ctx = cropCanvas.getContext('2d');
  const g = cropGeometry(240);
  ctx.clearRect(0, 0, 240, 240);
  ctx.drawImage(crop.img, g.x, g.y, g.w, g.h);
  // voile hors du cercle
  ctx.save();
  ctx.fillStyle = 'rgba(0,0,0,.55)';
  ctx.beginPath();
  ctx.rect(0, 0, 240, 240);
  ctx.arc(120, 120, 112, 0, Math.PI * 2, true);
  ctx.fill();
  ctx.strokeStyle = '#f0c96a';
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(120, 120, 112, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
}
function closeCropper() {
  crop = null;
  $('#cropper').hidden = true;
  $('#pion-file').value = '';
  if (S) renderPion();
}

$('#pion-file').addEventListener('change', (ev) => {
  const file = ev.target.files[0];
  if (!file) return;
  if (file.size > 20 * 1024 * 1024) { toast('Image trop lourde (20 Mo maximum).'); return; }
  const img = new Image();
  img.onload = () => {
    crop = { img, zoom: 1, ox: 0, oy: 0 };
    $('#crop-zoom').value = 1;
    $('#cropper').hidden = false;
    $('#pion-image-zone').hidden = false;
    drawCrop();
  };
  img.onerror = () => toast('Impossible de lire cette image.');
  img.src = URL.createObjectURL(file);
});
$('#crop-zoom').addEventListener('input', (ev) => { if (crop) { crop.zoom = Number(ev.target.value); drawCrop(); } });
cropCanvas.addEventListener('pointerdown', (ev) => {
  if (!crop) return;
  cropCanvas.setPointerCapture(ev.pointerId);
  let lx = ev.clientX;
  let ly = ev.clientY;
  const ratio = 240 / cropCanvas.getBoundingClientRect().width;
  const move = (pe) => {
    crop.ox += (pe.clientX - lx) * ratio;
    crop.oy += (pe.clientY - ly) * ratio;
    lx = pe.clientX;
    ly = pe.clientY;
    drawCrop();
  };
  const up = () => { cropCanvas.removeEventListener('pointermove', move); cropCanvas.removeEventListener('pointerup', up); };
  cropCanvas.addEventListener('pointermove', move);
  cropCanvas.addEventListener('pointerup', up);
});
cropCanvas.addEventListener('wheel', (ev) => {
  if (!crop) return;
  ev.preventDefault();
  crop.zoom = clamp(crop.zoom * (ev.deltaY < 0 ? 1.08 : 1 / 1.08), 1, 4);
  $('#crop-zoom').value = crop.zoom;
  drawCrop();
}, { passive: false });
$('#crop-cancel').addEventListener('click', closeCropper);
$('#crop-ok').addEventListener('click', () => {
  if (!crop || !pionId) return;
  const out = document.createElement('canvas');
  out.width = out.height = 256;
  const g = cropGeometry(256);
  out.getContext('2d').drawImage(crop.img, g.x, g.y, g.w, g.h);
  let data = out.toDataURL('image/webp', 0.88);
  if (!data.startsWith('data:image/webp')) data = out.toDataURL('image/png');
  socket.emit('token:image', { id: pionId, data }, (res) => {
    if (res && !res.ok) return toast(res.error);
    toast('Portrait appliqué !');
    closeCropper();
  });
});

// ------------------------------------------------------------------ groupe (sous la carte)
const hpColor = (p) => (p > 50 ? '#3f8f3f' : p > 25 ? '#c98a1b' : '#b22a2a');

function renderParty() {
  const chars = Object.values(S.characters);
  $('#party').replaceChildren(...(chars.length ? chars.map((c) => {
    const p = clamp((c.pv / c.pvMax) * 100, 0, 100);
    return el('div', { class: 'pc' + (c.online ? ' online' : '') + (c.id === me.charId ? ' me' : '') },
      el('div', { class: 'pc-top' },
        el('span', { class: 'dot', title: c.online ? 'Connecté' : 'Hors ligne' }),
        miniToken(tokenOfChar(c.id), 26),
        el('span', { class: 'pc-name', style: { color: c.couleur }, text: c.name }),
        me.role === 'mj' ? el('span', { class: 'pc-btns' },
          el('button', { class: 'tiny-btn', title: '−1 PV', onclick: () => sendPatch(c.id, { pv: c.pv - 1 }) }, '−'),
          el('button', { class: 'tiny-btn', title: '+1 PV', onclick: () => sendPatch(c.id, { pv: c.pv + 1 }) }, '+'),
          el('button', { class: 'tiny-btn', title: 'Ouvrir la fiche', onclick: () => { ficheId = c.id; showTab('fiche'); renderSheet(); } }, '📜')) : null),
      el('div', { class: 'pc-sub', text: [c.race, c.classe, 'niv. ' + c.niveau].filter(Boolean).join(' · ') }),
      el('div', { class: 'hp' },
        el('div', { class: 'hp-fill', style: { width: p + '%', background: hpColor(p) } }),
        el('span', { class: 'hp-txt', text: `${c.pv} / ${c.pvMax} PV` })));
  }) : [el('span', { class: 'muted', text: 'Aucun aventurier pour le moment.' })]));
}

// ------------------------------------------------------------------ fiche de personnage
$('#stats').append(...STATS.map(([k, label]) => el('div', { class: 'stat' },
  el('span', { class: 'stat-label', text: label }),
  el('button', { class: 'stat-mod', 'data-mod': k, title: `Jet de ${label}` }),
  el('input', { type: 'number', min: 1, max: 30, 'data-stat': k }))));

let pending = { id: null, patch: {} };
let pendingTimer;
function sendPatch(id, patch) { socket.emit('sheet:update', { id, patch }); }
function queuePatch(p) {
  if (pending.id !== ficheId) { flushPatch(); pending = { id: ficheId, patch: {} }; }
  for (const [k, v] of Object.entries(p)) {
    if (k === 'stats') pending.patch.stats = { ...(pending.patch.stats || {}), ...v };
    else pending.patch[k] = v;
  }
  clearTimeout(pendingTimer);
  pendingTimer = setTimeout(flushPatch, 350);
}
function flushPatch() {
  clearTimeout(pendingTimer);
  if (pending.id && Object.keys(pending.patch).length) sendPatch(pending.id, pending.patch);
  pending = { id: null, patch: {} };
}

$('#sheet').addEventListener('input', (ev) => {
  const t = ev.target;
  if (!ficheId) return;
  if (t.dataset.field && t.dataset.field !== 'name') {
    queuePatch({ [t.dataset.field]: t.type === 'number' ? Number(t.value) : t.value });
  } else if (t.dataset.stat) {
    queuePatch({ stats: { [t.dataset.stat]: Number(t.value) } });
  }
});
$('#sheet').addEventListener('change', (ev) => {
  if (ev.target.dataset.field === 'name' && ficheId) sendPatch(ficheId, { name: ev.target.value.trim() });
});
$('#sheet').addEventListener('click', (ev) => {
  const b = ev.target.closest('[data-mod]');
  const c = S && S.characters[ficheId];
  if (!b || !c) return;
  const k = b.dataset.mod;
  const label = STATS.find(([x]) => x === k)[1];
  roll('1d20' + fmtMod(mod(c.stats[k])), me.role === 'mj' ? `${c.name} — ${label}` : label);
});
$('#calc-init').addEventListener('click', () => {
  if (me.role === 'mj') {
    const c = S.characters[ficheId];
    if (c) roll('1d20' + fmtMod(mod(c.stats.dex)), `${c.name} — Initiative`);
  } else socket.emit('init:roll');
});

function hpChange(sign) {
  const c = S && S.characters[ficheId];
  const n = Math.abs(parseInt($('#dmg-amount').value, 10));
  if (!c || !n) return;
  if (sign < 0) {
    const absorbed = Math.min(c.pvTemp || 0, n);
    sendPatch(c.id, { pvTemp: (c.pvTemp || 0) - absorbed, pv: c.pv - (n - absorbed) });
  } else {
    sendPatch(c.id, { pv: Math.min(c.pvMax, c.pv + n) });
  }
  $('#dmg-amount').value = '';
}
$('#btn-dmg').addEventListener('click', () => hpChange(-1));
$('#btn-heal').addEventListener('click', () => hpChange(1));

$('#fiche-select').addEventListener('change', (ev) => { flushPatch(); ficheId = ev.target.value; renderSheet(); });
$('#fiche-delete').addEventListener('click', () => {
  const c = S.characters[ficheId];
  if (c && confirm(`Supprimer définitivement ${c.name} ? Sa fiche sera perdue.`)) socket.emit('character:delete', { id: c.id });
});

function renderSheet() {
  if (me.role === 'mj') {
    const ids = Object.keys(S.characters);
    if (!S.characters[ficheId]) ficheId = ids[0] || null;
    const sel = $('#fiche-select');
    if (document.activeElement !== sel) {
      sel.replaceChildren(...Object.values(S.characters).map((c) => el('option', { value: c.id, text: c.name })));
      sel.value = ficheId || '';
    }
  } else {
    ficheId = me.charId;
  }
  const c = S.characters[ficheId];
  $('#sheet').hidden = !c;
  $('#sheet-empty').hidden = !!c;
  if (!c) return;
  for (const inp of $$('[data-field]', $('#sheet'))) {
    if (inp === document.activeElement) continue;
    inp.value = c[inp.dataset.field] ?? '';
  }
  for (const [k] of STATS) {
    const inp = $(`[data-stat="${k}"]`);
    if (inp !== document.activeElement) inp.value = c.stats[k];
    $(`[data-mod="${k}"]`).textContent = fmtMod(mod(c.stats[k]));
  }
  $('#calc-init').textContent = fmtMod(mod(c.stats.dex));
  $('#calc-maitrise').textContent = '+' + maitrise(c.niveau);
}

// ------------------------------------------------------------------ dés & journal
function roll(formula, label = '', secret = $('#roll-secret').checked) {
  socket.emit('roll', { formula, label, secret }, (res) => { if (res && !res.ok) toast(res.error); });
}
// ---- composition du lancer : on choisit ses dés, puis on appuie sur « Lancer »
const DIE_TYPES = [4, 6, 8, 10, 12, 20, 100];
const pool = { dice: {}, mod: 0, adv: null };

function poolFormula() {
  const parts = [];
  for (const f of DIE_TYPES) {
    const n = pool.dice[f] || 0;
    if (!n) continue;
    const label = f === 100 ? '%' : f;
    if (f === 20 && pool.adv && n === 1) parts.push(`2d20${pool.adv}1`);
    else parts.push(`${n}d${label}`);
  }
  let s = parts.join('+');
  if (s && pool.mod) s += (pool.mod > 0 ? '+' : '') + pool.mod;
  return s;
}
function renderPool() {
  for (const b of $$('#dice-bar [data-die]')) {
    const n = pool.dice[b.dataset.die] || 0;
    b.classList.toggle('active', n > 0);
    b.querySelector('.die-count').textContent = n > 1 ? '×' + n : n === 1 ? '×1' : '';
  }
  $('#pool-mod').textContent = (pool.mod >= 0 ? '+' : '') + pool.mod;
  $$('[data-adv]').forEach((b) => b.classList.toggle('active', pool.adv === b.dataset.adv));
}
function updatePool() {
  $('#roll-formula').value = poolFormula();
  renderPool();
}
function resetPool() {
  pool.dice = {};
  pool.mod = 0;
  pool.adv = null;
  updatePool();
}

$('#dice-bar').append(...DIE_TYPES.map((f) => {
  const b = el('button', { type: 'button', class: 'die-btn', 'data-die': f, title: `d${f === 100 ? '%' : f}` });
  b.innerHTML = dieShapeSVG(f, '#8c6420', f === 100 ? '%' : f);
  b.append(el('span', { class: 'die-count' }));
  b.addEventListener('click', () => {
    pool.dice[f] = Math.min(20, (pool.dice[f] || 0) + 1);
    if (f === 20 && pool.dice[20] > 1) pool.adv = null;
    updatePool();
  });
  b.addEventListener('contextmenu', (ev) => {
    ev.preventDefault();
    pool.dice[f] = Math.max(0, (pool.dice[f] || 0) - 1);
    updatePool();
  });
  return b;
}));
$('#mod-plus').addEventListener('click', () => { pool.mod = Math.min(99, pool.mod + 1); updatePool(); });
$('#mod-minus').addEventListener('click', () => { pool.mod = Math.max(-99, pool.mod - 1); updatePool(); });
$$('[data-adv]').forEach((b) => b.addEventListener('click', () => {
  pool.adv = pool.adv === b.dataset.adv ? null : b.dataset.adv;
  if (pool.adv) pool.dice[20] = 1; // avantage / désavantage : un seul d20, lancé deux fois
  updatePool();
}));
$('#roll-clear').addEventListener('click', resetPool);
// Si le joueur tape sa formule à la main, la sélection de dés ne s'applique plus
$('#roll-formula').addEventListener('input', () => { pool.dice = {}; pool.mod = 0; pool.adv = null; renderPool(); });
$('#roll-form').addEventListener('submit', (ev) => {
  ev.preventDefault();
  const f = $('#roll-formula').value.trim();
  if (!f) { toast('Choisis d\'abord un ou plusieurs dés.'); return; }
  const label = pool.adv === 'kh' ? 'Avantage' : pool.adv === 'kl' ? 'Désavantage' : '';
  socket.emit('roll', { formula: f, label, secret: $('#roll-secret').checked }, (res) => {
    if (res && !res.ok) toast(res.error);
    else resetPool();
  });
});

// ---- options d'animation (mémorisées sur cet appareil)
const OPT_KEY = 'taverne-options';
const opts = (() => { try { return { anim: true, sound: true, ...JSON.parse(localStorage.getItem(OPT_KEY)) }; } catch { return { anim: true, sound: true }; } })();
$('#opt-anim').checked = opts.anim;
$('#opt-sound').checked = opts.sound;
const saveOpts = () => { try { localStorage.setItem(OPT_KEY, JSON.stringify(opts)); } catch { /* ignoré */ } };
$('#opt-anim').addEventListener('change', (ev) => { opts.anim = ev.target.checked; saveOpts(); });
$('#opt-sound').addEventListener('change', (ev) => { opts.sound = ev.target.checked; saveOpts(); });

/** Nouveau jet reçu : les dés roulent sur la table, puis le résultat s'inscrit dans le journal. */
function onRollEntry(e) {
  if (!opts.anim || document.hidden) return addLogEntry(e);
  animateRoll($('#dice-tray'), e, { sound: opts.sound });
  setTimeout(() => addLogEntry(e), DICE_ANIM_MS);
}

$('#chat-form').addEventListener('submit', (ev) => {
  ev.preventDefault();
  const inp = $('#chat-input');
  const text = inp.value.trim();
  if (!text) return;
  const m = /^\/(s?r)\s+(\S+)\s*(.*)$/i.exec(text);
  if (m) roll(m[2], m[3], m[1].toLowerCase() === 'sr');
  else socket.emit('chat', { text, to: $('#chat-to').value || null, narration: me.role === 'mj' && $('#chat-narr').checked });
  inp.value = '';
});

function renderChatTargets() {
  const sel = $('#chat-to');
  const cur = sel.value;
  const opts = [el('option', { value: '', text: 'À tous' })];
  if (me.role === 'player') opts.push(el('option', { value: 'mj', text: 'Murmure au MJ' }));
  for (const c of Object.values(S.characters)) {
    if (c.id !== me.charId) opts.push(el('option', { value: c.id, text: 'Murmure à ' + c.name }));
  }
  sel.replaceChildren(...opts);
  sel.value = [...sel.options].some((o) => o.value === cur) ? cur : '';
}

function addLogEntry(e) {
  const box = $('#log');
  const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 60;
  box.append(renderEntry(e));
  while (box.children.length > 300) box.firstChild.remove();
  if (atBottom) box.scrollTop = box.scrollHeight;
  if ($('#tab-chat').hidden) $('.side-tabs [data-tab="chat"]').classList.add('unread');
}
$('.side-tabs [data-tab="chat"]').addEventListener('click', (ev) => ev.currentTarget.classList.remove('unread'));

function renderEntry(e) {
  const time = new Date(e.t).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
  const who = e.from ? el('span', { class: 'who', style: { color: e.from.couleur }, text: e.from.name }) : null;
  if (e.type === 'system') return el('div', { class: 'entry system', text: e.text });
  if (e.type === 'narration') return el('div', { class: 'entry narration', text: e.text });
  if (e.type === 'chat') {
    return el('div', { class: 'entry chat' + (e.whisper ? ' whisper' : '') },
      el('span', { class: 'time', text: time }), who,
      e.whisper ? el('span', { class: 'to', text: ' → ' + e.toName }) : null,
      el('span', { class: 'msg', text: ' ' + e.text }));
  }
  if (e.type === 'roll') {
    const r = e.roll;
    const d20 = r.parts.filter((p) => p.type === 'dice' && p.faces === 20).flatMap((p) => p.rolls.filter((_, i) => p.kept[i]));
    const crit = d20.length === 1 && d20[0] === 20 ? ' crit' : d20.length === 1 && d20[0] === 1 ? ' fumble' : '';
    return el('div', { class: 'entry roll' + (e.secret ? ' secret' : '') + crit },
      el('div', { class: 'roll-head' },
        el('span', { class: 'time', text: time }), who,
        e.label ? el('span', { class: 'roll-label', text: ' — ' + e.label }) : null,
        e.secret ? el('span', { class: 'tag', text: '🔒 secret' }) : null),
      el('div', { class: 'roll-body' },
        el('span', { class: 'roll-detail' }, ...renderDetail(r)),
        el('span', { class: 'roll-total', text: r.total })),
      crit === ' crit' ? el('div', { class: 'crit-msg', text: 'Réussite critique !' }) : null,
      crit === ' fumble' ? el('div', { class: 'crit-msg', text: 'Échec critique…' }) : null);
  }
  return el('div', { class: 'entry', text: e.text || '' });
}

function renderDetail(r) {
  const out = [el('span', { class: 'roll-formula', text: r.formula + ' ⟶ ' })];
  r.parts.forEach((p, i) => {
    if (i > 0 || p.sign < 0) out.push(p.sign < 0 ? ' − ' : ' + ');
    if (p.type === 'const') { out.push(String(p.value)); return; }
    out.push('[');
    p.rolls.forEach((v, j) => {
      if (j) out.push(', ');
      const cls = [!p.kept[j] && 'dropped', p.faces === 20 && v === 20 && 'nat20', p.faces === 20 && v === 1 && 'nat1'].filter(Boolean).join(' ');
      out.push(el('span', { class: cls || null, text: v }));
    });
    out.push(']');
  });
  return out;
}

// ------------------------------------------------------------------ initiative
$('#init-roll').addEventListener('click', () => socket.emit('init:roll'));
$('#init-next').addEventListener('click', () => socket.emit('init:step', { dir: 1 }));
$('#init-prev').addEventListener('click', () => socket.emit('init:step', { dir: -1 }));
$('#init-clear').addEventListener('click', () => { if (confirm('Terminer le combat et vider l\'initiative ?')) socket.emit('init:clear'); });
$('#init-form').addEventListener('submit', (ev) => {
  ev.preventDefault();
  socket.emit('init:add', { name: $('#init-name').value, valeur: $('#init-val').value, cache: $('#init-hidden').checked });
  $('#init-name').value = '';
  $('#init-val').value = '';
});

function renderInit() {
  const ini = S.initiative;
  const list = $('#init-list');
  if (list.contains(document.activeElement)) return;
  const isMj = me.role === 'mj';
  $('#init-round').textContent = 'Round ' + ini.round;
  $('#init-empty').hidden = ini.entries.length > 0;
  list.replaceChildren(...ini.entries.map((e) => el('li', { class: (e.id === ini.actif ? 'active ' : '') + (e.cache ? 'is-hidden' : '') },
    isMj
      ? el('input', { type: 'number', class: 'init-val', value: e.valeur, onchange: (ev) => socket.emit('init:set', { id: e.id, valeur: ev.target.value }) })
      : el('span', { class: 'init-val', text: e.valeur }),
    miniToken(e.charId ? tokenOfChar(e.charId) : S.tokens[e.tokenId], 24),
    el('span', { class: 'init-name', style: { color: e.couleur }, text: e.name }),
    isMj ? el('span', { class: 'init-btns' },
      el('button', { class: 'tiny-btn', title: e.cache ? 'Montrer aux joueurs' : 'Cacher aux joueurs', onclick: () => socket.emit('init:set', { id: e.id, cache: !e.cache }) }, e.cache ? '🙈' : '👁'),
      el('button', { class: 'tiny-btn', title: 'Retirer', onclick: () => socket.emit('init:remove', { id: e.id }) }, '✕')) : null)));
}

// ------------------------------------------------------------------ outils MJ
$('#scene-form').addEventListener('submit', (ev) => {
  ev.preventDefault();
  const file = $('#scene-file').files[0];
  if (!file) return;
  if (file.size > 12 * 1024 * 1024) return toast('Image trop lourde (12 Mo maximum).');
  const reader = new FileReader();
  reader.onload = () => {
    toast('Envoi de la carte…');
    socket.emit('scene:upload', { titre: $('#scene-titre').value || file.name.replace(/\.[^.]+$/, ''), data: reader.result }, (res) => {
      if (res && !res.ok) return toast(res.error);
      toast('Carte affichée aux joueurs.');
      $('#scene-form').reset();
    });
  };
  reader.readAsDataURL(file);
});
$('#scene-none').addEventListener('click', () => socket.emit('scene:activer', { id: null }));
$('#grid-on').addEventListener('change', (ev) => socket.emit('scene:grille', { grille: ev.target.checked }));
$('#grid-size').addEventListener('change', (ev) => socket.emit('scene:grille', { cellule: ev.target.value }));
$('#token-form').addEventListener('submit', (ev) => {
  ev.preventDefault();
  socket.emit('token:add', {
    label: $('#token-name').value, couleur: $('#token-color').value,
    taille: $('#token-size').value, cache: $('#token-hidden').checked,
  });
  $('#token-name').value = '';
});
$('#log-clear').addEventListener('click', () => { if (confirm('Effacer tout le journal pour tout le monde ?')) socket.emit('log:clear'); });

function copyLink(url) {
  navigator.clipboard?.writeText(url).then(() => toast('Lien copié !'), () => toast(url));
}

function renderMj() {
  // Liens d'invitation
  const L = S.liens || { local: [] };
  $('#links').replaceChildren(
    L.tunnel
      ? el('div', { class: 'link main' }, el('span', { text: '🌍 Internet : ' }), el('code', { text: L.tunnel }),
        el('button', { class: 'tiny-btn', onclick: () => copyLink(L.tunnel) }, 'Copier'))
      : el('p', { class: 'muted tiny', text: 'Pas de lien Internet actif (cloudflared non installé ou en cours de démarrage). Voir le README.' }),
    ...L.local.map((u) => el('div', { class: 'link' }, el('span', { text: '🏠 Même réseau : ' }), el('code', { text: u }),
      el('button', { class: 'tiny-btn', onclick: () => copyLink(u) }, 'Copier'))));

  // Scènes
  $('#scene-list').replaceChildren(...S.scenes.map((s) => el('li', { class: s.id === S.scene.actif ? 'active' : '' },
    s.image ? el('img', { src: s.image, class: 'thumb', alt: '' })
      : el('span', { class: 'thumb thumb-bm', title: 'Battle map', style: { backgroundImage: `url(${terrainTexture(s.fond || 'herbe', 32, 0).toDataURL()})` } }, '⚔'),
    el('span', { class: 'grow', text: s.titre }),
    s.id === S.scene.actif ? el('span', { class: 'tag', text: 'affichée' })
      : el('button', { class: 'tiny-btn', onclick: () => socket.emit('scene:activer', { id: s.id }) }, 'Afficher'),
    el('button', { class: 'tiny-btn', title: 'Supprimer', onclick: () => { if (confirm(`Supprimer la scène « ${s.titre} » ?`)) socket.emit('scene:supprimer', { id: s.id }); } }, '✕'))));

  // Grille
  $('#grid-on').checked = S.scene.grille;
  if (document.activeElement !== $('#grid-size')) $('#grid-size').value = S.scene.cellule;

  // Pions
  const tokens = Object.values(S.tokens).sort((a, b) => (!!a.ownerId === !!b.ownerId ? a.label.localeCompare(b.label) : a.ownerId ? -1 : 1));
  $('#token-list').replaceChildren(...tokens.map((t) => el('li', { class: t.cache ? 'is-hidden' : '' },
    miniToken(t, 24),
    el('span', { class: 'grow', text: t.label + (t.ownerId ? ' (joueur)' : '') }),
    el('button', { class: 'tiny-btn', title: 'Personnaliser l\'apparence', onclick: () => openPion(t.id) }, '🎨'),
    el('button', { class: 'tiny-btn', title: t.cache ? 'Révéler aux joueurs' : 'Cacher aux joueurs', onclick: () => socket.emit('token:update', { id: t.id, cache: !t.cache }) }, t.cache ? '🙈' : '👁'),
    t.ownerId ? null : el('button', { class: 'tiny-btn', title: 'Ajouter à l\'initiative (d20 secret)', onclick: () => socket.emit('init:add', { name: t.label, couleur: t.couleur, tokenId: t.id, cache: t.cache }) }, '⚔'),
    t.ownerId ? null : el('button', { class: 'tiny-btn', title: 'Supprimer le pion', onclick: () => socket.emit('token:remove', { id: t.id }) }, '✕'))));
}

window.addEventListener('resize', () => { if (S && !S.scene.image) fitZoom(); });
if (!session) showLogin();
