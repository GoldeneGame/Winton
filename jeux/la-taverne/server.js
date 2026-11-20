'use strict';
/**
 * La Taverne — serveur de table de jeu de rôle.
 * Un MJ (mot de passe) + des joueurs (nom du personnage).
 * État de la partie sauvegardé dans data/partie.json.
 */
const express = require('express');
const http = require('http');
const { Server } = require('socket.io');
const { spawn, spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');
const crypto = require('crypto');

// Version .exe (Node « single executable ») : les fichiers de l'interface sont
// embarqués dans l'exécutable.
let sea = null;
try { sea = require('node:sea'); if (!sea.isSea()) sea = null; } catch { sea = null; }

/** Dossier du programme (remplacé à chaque mise à jour par le launcher). */
const ROOT = sea ? path.dirname(process.execPath) : __dirname;

/**
 * Dossier des données du MJ (partie, images, config.json, cloudflared) : jamais dans le
 * dossier du programme, pour qu'une mise à jour ne puisse rien effacer.
 *  - LATAVERNE_DATA_DIR s'il est défini ;
 *  - sinon, pour l'exe : %APPDATA%\LaTaverne ;
 *  - sinon (lancé depuis le code source) : le dossier du projet, comme avant.
 */
const USER_DIR = process.env.LATAVERNE_DATA_DIR
  || (sea ? path.join(process.env.APPDATA || os.homedir(), 'LaTaverne') : __dirname);
fs.mkdirSync(USER_DIR, { recursive: true });

// Les anciennes versions gardaient data\ et config.json à côté du .exe :
// on les recopie une fois dans USER_DIR (l'original n'est pas supprimé).
if (USER_DIR !== ROOT) {
  for (const name of ['data', 'config.json', 'cloudflared.exe']) {
    const from = path.join(ROOT, name);
    const to = path.join(USER_DIR, name);
    if (fs.existsSync(from) && !fs.existsSync(to)) {
      try { fs.cpSync(from, to, { recursive: true }); } catch (e) { console.log(`  Copie de ${name} impossible : ${e.message}`); }
    }
  }
}

const DATA_DIR = path.join(USER_DIR, 'data');
const IMG_DIR = path.join(DATA_DIR, 'images');
const SAVE_FILE = path.join(DATA_DIR, 'partie.json');

fs.mkdirSync(IMG_DIR, { recursive: true });

function readJson(file, fallback) {
  try { return JSON.parse(fs.readFileSync(file, 'utf8')); } catch { return fallback; }
}

const CONFIG_FILE = path.join(USER_DIR, 'config.json');
const DEFAULT_CONFIG = { port: 3000, motDePasseMJ: 'dragon', tunnelCloudflare: true, ouvrirNavigateur: true, cleApiAnthropic: '' };
if (!fs.existsSync(CONFIG_FILE)) {
  try { fs.writeFileSync(CONFIG_FILE, JSON.stringify(DEFAULT_CONFIG, null, 2)); } catch { /* dossier en lecture seule */ }
}
const config = Object.assign({}, DEFAULT_CONFIG, readJson(CONFIG_FILE, {}));

// ---------------------------------------------------------------- utilitaires
const uid = () => crypto.randomBytes(6).toString('hex');
const norm = (s) => String(s || '').trim().toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');
const str = (v, max = 200) => String(v ?? '').slice(0, max);
const num = (v, min, max, def = 0) => {
  const n = Math.round(Number(v));
  return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : def;
};
const pct = (v) => { const n = Number(v); return Number.isFinite(n) ? Math.min(100, Math.max(0, n)) : 50; };
const mod = (v) => Math.floor(((Number(v) || 10) - 10) / 2);
const fmtMod = (m) => (m >= 0 ? '+' : '') + m;
const isColor = (c) => /^#[0-9a-f]{6}$/i.test(String(c || ''));
const COLORS = ['#e6194b', '#3cb44b', '#4363d8', '#f58231', '#911eb4', '#42d4f4', '#f032e6',
  '#bfef45', '#469990', '#9a6324', '#e6beff', '#fffac8', '#aaffc3', '#ffd8b1'];

// ---------------------------------------------------------------- dés
/** Formules : 1d20+5, 2d6+1d4-1, 2d20kh1 (avantage), 2d20kl1, 4d6kh3, d% */
function rollFormula(formula) {
  const f = String(formula || '').toLowerCase().replace(/\s+/g, '');
  if (!f || f.length > 60) throw new Error('Formule invalide');
  const re = /([+-])?(?:(\d*)d(\d+|%)(?:(kh|kl)(\d+))?|(\d+))/y;
  const parts = [];
  let pos = 0, total = 0, diceCount = 0;
  while (pos < f.length) {
    re.lastIndex = pos;
    const m = re.exec(f);
    if (!m || !m[0]) throw new Error('Formule invalide : ' + f);
    if (pos > 0 && !m[1]) throw new Error('Formule invalide : ' + f);
    pos = re.lastIndex;
    const sign = m[1] === '-' ? -1 : 1;
    if (m[3]) {
      const n = m[2] ? parseInt(m[2], 10) : 1;
      const faces = m[3] === '%' ? 100 : parseInt(m[3], 10);
      if (n < 1 || n > 100 || faces < 2 || faces > 1000) throw new Error('Trop de dés ou dé invalide');
      diceCount += n;
      if (diceCount > 200) throw new Error('Trop de dés');
      const rolls = Array.from({ length: n }, () => crypto.randomInt(1, faces + 1));
      let kept = rolls.map(() => true);
      if (m[4]) {
        const k = Math.min(n, Math.max(1, parseInt(m[5], 10)));
        const order = rolls.map((v, i) => [v, i]).sort((a, b) => (m[4] === 'kh' ? b[0] - a[0] : a[0] - b[0]));
        const keep = new Set(order.slice(0, k).map(([, i]) => i));
        kept = rolls.map((_, i) => keep.has(i));
      }
      const sum = rolls.reduce((s, v, i) => s + (kept[i] ? v : 0), 0);
      total += sign * sum;
      parts.push({ type: 'dice', sign, n, faces, keep: m[4] ? m[4] + m[5] : null, rolls, kept });
    } else {
      const value = parseInt(m[6], 10);
      total += sign * value;
      parts.push({ type: 'const', sign, value });
    }
  }
  if (!parts.some((p) => p.type === 'dice')) throw new Error('Il faut au moins un dé (ex : 1d20)');
  return { formula: f, total, parts };
}

// ---------------------------------------------------------------- état de la partie
const defaultState = () => ({
  characters: {},
  scenes: [],
  scene: { actif: null, grille: true, cellule: 70 },
  tokens: {},
  initiative: { entries: [], actif: null, round: 1 },
  log: [],
});
const saved = readJson(SAVE_FILE, {});
const state = Object.assign(defaultState(), saved);
state.scene = Object.assign(defaultState().scene, saved.scene || {});
state.initiative = Object.assign(defaultState().initiative, saved.initiative || {});

let saveTimer = null;
function save() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, 300);
}
function saveNow() {
  clearTimeout(saveTimer);
  try {
    fs.writeFileSync(SAVE_FILE + '.tmp', JSON.stringify(state, null, 1));
    fs.renameSync(SAVE_FILE + '.tmp', SAVE_FILE);
  } catch (e) { console.error('Sauvegarde impossible :', e.message); }
}

function newCharacter(name) {
  const used = new Set(Object.values(state.characters).map((c) => c.couleur));
  return {
    id: uid(), name, race: '', classe: '', niveau: 1, alignement: '', historique: '',
    pv: 10, pvMax: 10, pvTemp: 0, ca: 10, vitesse: 9, xp: 0, po: 0,
    stats: { for: 10, dex: 10, con: 10, int: 10, sag: 10, cha: 10 },
    competences: '', inventaire: '', sorts: '', notes: '',
    couleur: COLORS.find((c) => !used.has(c)) || COLORS[Math.floor(Math.random() * COLORS.length)],
  };
}

// ---- apparence des pions (« skins »)
const CONTENUS = ['figurine', 'initiales', 'embleme', 'image'];
const FORMES = ['socle', 'rond', 'carre', 'hexa', 'ecu'];
const CADRES = ['simple', 'bronze', 'argent', 'or', 'feu', 'glace', 'nature', 'arcane', 'ombre', 'sacre', 'sang'];

// Petit personnage personnalisable : chaque option est une liste fermée (le client dessine le SVG).
const FIG_ENUMS = {
  cheveux: ['chauve', 'court', 'long', 'herisse', 'queue', 'chignon'],
  barbe: ['aucune', 'courte', 'longue', 'moustache'],
  oreilles: ['humaines', 'elfiques'],
  tenue: ['tunique', 'robe', 'armure', 'cuir'],
  tete: ['rien', 'casque', 'capuche', 'mage', 'couronne', 'cornes', 'bandeau'],
  arme: ['aucune', 'epee', 'hache', 'baton', 'arc', 'dague', 'masse', 'lance'],
};
const FIG_COLORS = ['peau', 'couleurCheveux', 'couleurTenue', 'couleurCape'];
const FIG_BOOLS = ['crocs', 'bouclier', 'cape'];
const PEAUX = ['#f6d3b3', '#e8b48a', '#c68642', '#8d5524', '#5c3a1e'];
const CHEVEUX = ['#2b1a10', '#6b3e1f', '#c98a3a', '#e8d27a', '#b8b8b8', '#a3271b'];
const TENUES = ['#24456b', '#7a1f1f', '#2f6b2f', '#5b2a86', '#8a6a2a'];

function defaultFigurine() {
  const r = (l) => l[Math.floor(Math.random() * l.length)];
  return {
    peau: r(PEAUX), cheveux: r(['court', 'long', 'queue', 'herisse']), couleurCheveux: r(CHEVEUX),
    barbe: 'aucune', oreilles: 'humaines', crocs: false,
    tenue: r(['tunique', 'cuir', 'armure']), couleurTenue: r(TENUES),
    tete: 'rien', arme: r(['epee', 'hache', 'baton', 'arc', 'dague']), bouclier: false,
    cape: true, couleurCape: r(TENUES),
  };
}
function normalizeFigurine(f = {}, base = defaultFigurine()) {
  const out = {};
  for (const [k, list] of Object.entries(FIG_ENUMS)) out[k] = list.includes(f[k]) ? f[k] : (list.includes(base[k]) ? base[k] : list[0]);
  for (const k of FIG_COLORS) out[k] = isColor(f[k]) ? f[k] : (isColor(base[k]) ? base[k] : '#888888');
  for (const k of FIG_BOOLS) out[k] = typeof f[k] === 'boolean' ? f[k] : !!base[k];
  return out;
}

function normalizeSkin(s = {}, base = {}) {
  const pick = (v, list, def) => (list.includes(v) ? v : def);
  const skin = {
    contenu: pick(s.contenu, CONTENUS, pick(base.contenu, CONTENUS, 'initiales')),
    embleme: s.embleme !== undefined ? str(s.embleme, 16) : (base.embleme || '⚔️'),
    image: base.image || null, // l'image ne change que via « token:image »
    forme: pick(s.forme, FORMES, pick(base.forme, FORMES, 'rond')),
    cadre: pick(s.cadre, CADRES, pick(base.cadre, CADRES, 'simple')),
    figurine: normalizeFigurine(s.figurine && typeof s.figurine === 'object' ? s.figurine : {}, base.figurine || defaultFigurine()),
  };
  if (skin.contenu === 'image' && !skin.image) skin.contenu = 'initiales';
  return skin;
}
for (const t of Object.values(state.tokens)) t.skin = normalizeSkin({}, t.skin || {});

function removeTokenImage(t) {
  const img = t && t.skin && t.skin.image;
  if (img && /^\/images\/pion-[a-f0-9]+\.(png|jpg|webp)$/.test(img)) fs.unlink(path.join(IMG_DIR, path.basename(img)), () => {});
}

/** Première place libre en haut à gauche de la carte, pour que les nouveaux pions ne se chevauchent pas. */
function freeSpot() {
  const taken = Object.values(state.tokens);
  for (let row = 0; row < 8; row++) {
    for (let col = 0; col < 8; col++) {
      const x = 6 + col * 7;
      const y = 8 + row * 11;
      if (!taken.some((t) => Math.abs(t.x - x) < 4 && Math.abs(t.y - y) < 6)) return { x, y };
    }
  }
  return { x: 10 + Math.random() * 40, y: 10 + Math.random() * 40 };
}

function createToken({ label, couleur, taille = 1, ownerId = null, cache = false }) {
  const t = {
    id: uid(), label: str(label, 30), couleur: isColor(couleur) ? couleur : '#8b1e1e',
    taille: num(taille, 1, 4, 1), ownerId, cache: !!cache,
    ...freeSpot(),
    skin: normalizeSkin(ownerId ? { contenu: 'figurine', forme: 'socle', cadre: 'or' } : { cadre: 'simple' }),
  };
  state.tokens[t.id] = t;
  return t;
}

const TEXT_FIELDS = { race: 40, classe: 40, alignement: 40, historique: 60, competences: 5000, inventaire: 5000, sorts: 5000, notes: 5000 };
const NUM_FIELDS = { niveau: [1, 30], pv: [-999, 9999], pvMax: [1, 9999], pvTemp: [0, 999], ca: [0, 50], vitesse: [0, 99], xp: [0, 9999999], po: [0, 9999999] };
const STAT_KEYS = ['for', 'dex', 'con', 'int', 'sag', 'cha'];

function applyPatch(c, patch) {
  for (const [k, max] of Object.entries(TEXT_FIELDS)) if (k in patch) c[k] = str(patch[k], max);
  for (const [k, [min, max]] of Object.entries(NUM_FIELDS)) if (k in patch) c[k] = num(patch[k], min, max, c[k]);
  if (patch.stats && typeof patch.stats === 'object') {
    for (const k of STAT_KEYS) if (k in patch.stats) c.stats[k] = num(patch.stats[k], 1, 30, c.stats[k]);
  }
  if ('couleur' in patch && isColor(patch.couleur)) c.couleur = patch.couleur;
  if ('name' in patch) {
    const name = str(patch.name, 40).trim();
    const taken = Object.values(state.characters).some((o) => o.id !== c.id && norm(o.name) === norm(name));
    if (name.length >= 2 && !taken) c.name = name;
  }
  // Répercute nom / couleur sur le pion et l'initiative
  for (const t of Object.values(state.tokens)) if (t.ownerId === c.id) { t.label = c.name; t.couleur = c.couleur; }
  for (const e of state.initiative.entries) if (e.charId === c.id) { e.name = c.name; e.couleur = c.couleur; }
}

function sortInitiative() {
  state.initiative.entries.sort((a, b) => b.valeur - a.valeur);
}

// ---------------------------------------------------------------- diaporamas (slides projetées aux joueurs)
const SLIDE_STYLES = ['parchemin', 'sombre', 'sang', 'mystique', 'nature', 'glace', 'epique'];
state.slides = Object.assign({ decks: [], projection: null }, state.slides || {});

function newSlide(p = {}) {
  return {
    id: uid(),
    titre: str(p.titre, 120),
    texte: str(p.texte, 3000),
    style: SLIDE_STYLES.includes(p.style) ? p.style : 'parchemin',
    image: typeof p.image === 'string' && /^\/images\/[\w.-]+$/.test(p.image) ? p.image : null,
  };
}
const findDeck = (id) => state.slides.decks.find((d) => d.id === id);

/** Slide projetée en ce moment (ce que voient les joueurs), ou null. */
function currentProjection() {
  const p = state.slides.projection;
  const deck = p && findDeck(p.deckId);
  const slide = deck && deck.slides[p.index];
  if (!slide) return null;
  return { deckId: deck.id, deckTitre: deck.titre, index: p.index, total: deck.slides.length, ...slide };
}

// ---- génération par l'IA (Claude)
const IA_MODEL = 'claude-opus-5-5';
const iaKey = () => String(config.cleApiAnthropic || process.env.ANTHROPIC_API_KEY || '').trim();
let anthropicClient = null;
function getAnthropic() {
  if (!iaKey()) return null;
  if (!anthropicClient) {
    const Anthropic = require('@anthropic-ai/sdk').default;
    anthropicClient = new Anthropic({ apiKey: iaKey() });
  }
  return anthropicClient;
}

const SLIDES_SCHEMA = {
  type: 'object',
  properties: {
    slides: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          titre: { type: 'string' },
          texte: { type: 'string' },
          style: { type: 'string', enum: SLIDE_STYLES },
        },
        required: ['titre', 'texte', 'style'],
        additionalProperties: false,
      },
    },
  },
  required: ['slides'],
  additionalProperties: false,
};

const SLIDES_SYSTEM = `Tu aides le maître du jeu (MJ) d'une partie de jeu de rôle médiéval-fantastique, dans l'esprit de Donjons & Dragons, en français.
Tu rédiges des slides qui seront projetées sur l'écran de tous les joueurs pendant la séance, pendant que le MJ les lit à voix haute.

Pour chaque slide :
- "titre" : court et évocateur (6 mots au maximum), comme un titre de chapitre ou le nom d'un lieu ou d'un personnage.
- "texte" : un texte d'ambiance de 2 à 5 phrases, à lire à voix haute. Écris pour les joueurs, au présent, en t'adressant à eux ("vous"). Fais appel aux sens (bruits, odeurs, lumière, température) et donne un ou deux détails concrets sur lesquels les joueurs pourront agir.
- "style" : le thème visuel qui correspond le mieux à l'ambiance de la slide (parchemin = neutre ou récit, sombre = danger ou nuit, sang = combat ou horreur, mystique = magie ou divin, nature = forêt ou extérieur, glace = froid ou montagne, epique = victoire ou révélation majeure).

Les joueurs lisent ces slides : ne révèle jamais de secret du scénario, de statistique de jeu ni d'information que leurs personnages ne peuvent pas percevoir. Ne décide jamais à la place des joueurs de ce que font, disent ou ressentent leurs personnages ; termine plutôt sur une situation ouverte qui les invite à agir.
Les slides se suivent dans l'ordre de la séance et forment une progression cohérente.`;

async function generateSlides({ prompt, nombre, ton, deck, groupe }) {
  const client = getAnthropic();
  if (!client) throw new Error('Aucune clé API Anthropic : ajoute "cleApiAnthropic" dans config.json puis relance l\'application.');
  const lignes = [`Demande du MJ : ${prompt}`, `Nombre de slides à rédiger : ${nombre}.`];
  if (ton) lignes.push(`Ton souhaité : ${ton}.`);
  if (groupe.length) lignes.push(`Personnages joueurs du groupe : ${groupe.join(' ; ')}.`);
  if (deck && deck.slides.length) {
    lignes.push(`Le diaporama « ${deck.titre} » contient déjà ces slides, dans l'ordre ; les nouvelles viennent à la suite :\n`
      + deck.slides.map((s, i) => `${i + 1}. ${s.titre}`).join('\n'));
  }

  const response = await client.beta.messages.create({
    model: IA_MODEL,
    max_tokens: 16000,
    betas: ['server-side-fallback-2026-07-01'],
    fallbacks: 'default',
    output_config: { effort: 'medium', format: { type: 'json_schema', schema: SLIDES_SCHEMA } },
    system: SLIDES_SYSTEM,
    messages: [{ role: 'user', content: lignes.join('\n\n') }],
  });

  if (response.stop_reason === 'refusal') throw new Error('Claude a refusé cette demande. Reformule-la autrement.');
  if (response.stop_reason === 'max_tokens') throw new Error('La réponse était trop longue : demande moins de slides.');
  const text = response.content.filter((b) => b.type === 'text').map((b) => b.text).join('');
  let data;
  try { data = JSON.parse(text); } catch { throw new Error('Réponse illisible de l\'IA, réessaie.'); }
  return (data.slides || []).slice(0, nombre).map(newSlide);
}

function iaErrorMessage(e) {
  const Anthropic = require('@anthropic-ai/sdk').default;
  if (e instanceof Anthropic.AuthenticationError) return 'Clé API Anthropic invalide (vérifie "cleApiAnthropic" dans config.json).';
  if (e instanceof Anthropic.PermissionDeniedError) return 'Cette clé API n\'a pas accès au modèle.';
  if (e instanceof Anthropic.RateLimitError) return 'Trop de demandes en peu de temps : patiente un instant puis réessaie.';
  if (e instanceof Anthropic.APIConnectionError) return 'Impossible de joindre l\'IA : vérifie la connexion Internet.';
  if (e instanceof Anthropic.APIError) return `Erreur de l'IA (${e.status || '?'}) : ${e.message}`;
  return e.message || 'Erreur inconnue';
}

// ---------------------------------------------------------------- serveur
const MIME = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml', '.ico': 'image/x-icon', '.json': 'application/json',
};
const app = express();
if (sea) {
  app.use((req, res, next) => {
    if (req.method !== 'GET' && req.method !== 'HEAD') return next();
    const rel = req.path === '/' ? 'index.html' : req.path.replace(/^\/+/, '');
    let data;
    try { data = sea.getAsset('public/' + rel); } catch { return next(); }
    res.type(MIME[path.extname(rel)] || 'application/octet-stream').send(Buffer.from(data));
  });
} else {
  app.use(express.static(path.join(ROOT, 'public')));
}
app.use('/images', express.static(IMG_DIR, { maxAge: '1d' }));
const server = http.createServer(app);
const io = new Server(server, { maxHttpBufferSize: 15e6, serveClient: false });

let tunnelUrl = null;
const lanUrls = () => Object.values(os.networkInterfaces()).flat()
  .filter((i) => i && i.family === 'IPv4' && !i.internal)
  .map((i) => `http://${i.address}:${config.port}`);

function presence() {
  const online = new Set();
  let mj = false;
  for (const so of io.sockets.sockets.values()) {
    if (so.data.role === 'player') online.add(so.data.charId);
    if (so.data.role === 'mj') mj = true;
  }
  return { online, mj };
}

function viewFor(so) {
  const isMj = so.data.role === 'mj';
  const { online, mj } = presence();
  const characters = {};
  for (const c of Object.values(state.characters)) {
    characters[c.id] = isMj || c.id === so.data.charId
      ? { ...c, online: online.has(c.id) }
      : { id: c.id, name: c.name, race: c.race, classe: c.classe, niveau: c.niveau, pv: c.pv, pvMax: c.pvMax,
          couleur: c.couleur, online: online.has(c.id), resume: true };
  }
  const tokens = {};
  for (const t of Object.values(state.tokens)) if (isMj || !t.cache) tokens[t.id] = t;
  const active = state.scenes.find((s) => s.id === state.scene.actif);
  const ini = state.initiative;
  return {
    characters,
    tokens,
    scene: {
      ...state.scene,
      id: active ? active.id : null,
      image: active ? active.image : null,
      titre: active ? active.titre : '',
      battle: !!(active && !active.image),
      cols: active ? active.cols : null,
      rows: active ? active.rows : null,
      fond: active ? active.fond || null : null,
      sol: (active && active.sol) || {},
      deco: (active && active.deco) || {},
    },
    scenes: isMj ? state.scenes.map((s) => ({ id: s.id, titre: s.titre, image: s.image, battle: !s.image, fond: s.fond || null })) : [],
    initiative: {
      round: ini.round,
      actif: isMj || !ini.entries.find((e) => e.id === ini.actif)?.cache ? ini.actif : null,
      entries: ini.entries.filter((e) => isMj || !e.cache),
    },
    mjOnline: mj,
    liens: isMj ? { tunnel: tunnelUrl, local: lanUrls() } : null,
    projection: currentProjection(),
    slides: isMj ? { decks: state.slides.decks, ia: !!iaKey() } : null,
  };
}

function broadcastState() {
  for (const so of io.sockets.sockets.values()) if (so.data.role) so.emit('state', viewFor(so));
}

function canSee(so, e) {
  if (!so.data.role) return false;
  if (so.data.role === 'mj') return true;
  return !e.visibleTo || e.visibleTo.includes(so.data.charId);
}

function addLog(entry) {
  entry.id = uid();
  entry.t = Date.now();
  state.log.push(entry);
  if (state.log.length > 300) state.log.splice(0, state.log.length - 300);
  for (const so of io.sockets.sockets.values()) if (canSee(so, entry)) so.emit('log', entry);
  save();
}

const system = (text) => addLog({ type: 'system', text });

// ---------------------------------------------------------------- battle maps (sol + décor peints case par case)
const SOLS = ['herbe', 'foret', 'terre', 'sable', 'neige', 'paves', 'dalles', 'plancher', 'tapis',
  'eau', 'eau-prof', 'marais', 'glace', 'lave', 'gouffre', 'mur', 'roche'];
const DECOS = ['🌲', '🌳', '🌴', '🌿', '🍄', '🌾', '🪨', '🪵', '🔥', '⛺', '🛢️', '📦', '💰', '🪑', '🛏️', '🚪',
  '🗝️', '🗿', '⚰️', '🕯️', '🕸️', '🦴', '💀', '⛲', '🏛️', '⚱️', '📜', '⚗️', '❌', '⚠️'];
const activeScene = () => state.scenes.find((s) => s.id === state.scene.actif);
function pruneMap(s) {
  for (const calque of ['sol', 'deco']) {
    for (const k of Object.keys(s[calque] || {})) {
      const [c, r] = k.split(',').map(Number);
      if (c >= s.cols || r >= s.rows) delete s[calque][k];
    }
  }
}

io.on('connection', (socket) => {
  const isMj = () => socket.data.role === 'mj';
  const mjOnly = (fn) => (...args) => { if (isMj()) fn(...args); };
  const who = () => {
    if (isMj()) return { name: 'MJ', couleur: '#d4a64a', role: 'mj' };
    const c = state.characters[socket.data.charId];
    return c ? { name: c.name, couleur: c.couleur, role: 'player', id: c.id } : null;
  };
  const ok = (ack, data = {}) => typeof ack === 'function' && ack({ ok: true, ...data });
  const fail = (ack, error) => typeof ack === 'function' && ack({ ok: false, error });

  // ---- connexion
  socket.on('join', (p = {}, ack) => {
    if (typeof ack !== 'function') return;
    if (p.role === 'mj') {
      if (String(p.password || '') !== String(config.motDePasseMJ)) return fail(ack, 'Mot de passe MJ incorrect.');
      if (!presence().mj) system('Le Maître du jeu prend place derrière son écran.');
      socket.data.role = 'mj';
    } else {
      const name = str(p.name, 40).trim();
      if (name.length < 2) return fail(ack, 'Le nom doit faire au moins 2 caractères.');
      let c = Object.values(state.characters).find((x) => norm(x.name) === norm(name));
      if (!c) {
        c = newCharacter(name);
        state.characters[c.id] = c;
        createToken({ label: c.name, couleur: c.couleur, ownerId: c.id });
        system(`${c.name} rejoint l'aventure pour la première fois !`);
      } else if (!presence().online.has(c.id)) {
        system(`${c.name} est de retour à la table.`);
      }
      socket.data.role = 'player';
      socket.data.charId = c.id;
    }
    ok(ack, { role: socket.data.role, charId: socket.data.charId || null, log: state.log.filter((e) => canSee(socket, e)) });
    broadcastState();
    save();
  });

  socket.on('disconnect', () => {
    if (socket.data.role) broadcastState();
  });

  // ---- fiches
  socket.on('sheet:update', ({ id, patch } = {}) => {
    const c = state.characters[id];
    if (!c || !patch || typeof patch !== 'object') return;
    if (!isMj() && socket.data.charId !== id) return;
    applyPatch(c, patch);
    broadcastState();
    save();
  });

  socket.on('character:delete', mjOnly(({ id } = {}) => {
    const c = state.characters[id];
    if (!c) return;
    delete state.characters[id];
    for (const t of Object.values(state.tokens)) if (t.ownerId === id) { removeTokenImage(t); delete state.tokens[t.id]; }
    state.initiative.entries = state.initiative.entries.filter((e) => e.charId !== id);
    for (const so of io.sockets.sockets.values()) {
      if (so.data.charId === id) { so.emit('kicked', `Le MJ a supprimé le personnage ${c.name}.`); so.disconnect(true); }
    }
    system(`${c.name} a quitté l'aventure.`);
    broadcastState();
  }));

  // ---- dés & discussion
  socket.on('roll', ({ formula, label, secret } = {}, ack) => {
    const from = who();
    if (!from) return;
    let roll;
    try { roll = rollFormula(formula); } catch (e) { return fail(ack, e.message); }
    const visibleTo = secret ? (isMj() ? [] : [socket.data.charId]) : null;
    addLog({ type: 'roll', from, label: str(label, 60), roll, secret: !!secret, visibleTo });
    ok(ack);
  });

  socket.on('chat', ({ text, to, narration } = {}) => {
    const from = who();
    text = str(text, 2000).trim();
    if (!from || !text) return;
    if (narration && isMj()) return addLog({ type: 'narration', from, text });
    if (to) {
      let visibleTo, toName;
      if (to === 'mj') {
        if (isMj()) return;
        visibleTo = [socket.data.charId];
        toName = 'MJ';
      } else {
        const c = state.characters[to];
        if (!c) return;
        visibleTo = [c.id, socket.data.charId].filter(Boolean);
        toName = c.name;
      }
      return addLog({ type: 'chat', from, text, whisper: true, toName, visibleTo });
    }
    addLog({ type: 'chat', from, text });
  });

  socket.on('log:clear', mjOnly(() => {
    state.log = [];
    io.emit('log:reset');
    save();
  }));

  // ---- pions
  socket.on('token:move', ({ id, x, y } = {}) => {
    const t = state.tokens[id];
    if (!t) return;
    if (!isMj() && t.ownerId !== socket.data.charId) return;
    t.x = pct(x);
    t.y = pct(y);
    for (const so of io.sockets.sockets.values()) {
      if (so.data.role && (so.data.role === 'mj' || !t.cache)) so.emit('token:moved', { id, x: t.x, y: t.y });
    }
    save();
  });

  socket.on('token:add', mjOnly((p = {}) => {
    if (!str(p.label, 30).trim()) return;
    createToken({ label: p.label.trim(), couleur: p.couleur, taille: p.taille, cache: p.cache });
    broadcastState();
    save();
  }));

  socket.on('token:update', mjOnly(({ id, ...p } = {}) => {
    const t = state.tokens[id];
    if (!t) return;
    if ('label' in p && !t.ownerId && str(p.label, 30).trim()) t.label = str(p.label, 30).trim();
    if ('couleur' in p && isColor(p.couleur)) t.couleur = p.couleur;
    if ('taille' in p) t.taille = num(p.taille, 1, 4, 1);
    if ('cache' in p) t.cache = !!p.cache;
    broadcastState();
    save();
  }));

  // Apparence d'un pion : son propriétaire ou le MJ
  const canEditToken = (t) => t && (isMj() || (t.ownerId && t.ownerId === socket.data.charId));

  socket.on('token:skin', ({ id, skin, couleur, label, taille } = {}) => {
    const t = state.tokens[id];
    if (!canEditToken(t)) return;
    if (skin && typeof skin === 'object') t.skin = normalizeSkin(skin, t.skin);
    if (isColor(couleur)) {
      t.couleur = couleur;
      const c = t.ownerId && state.characters[t.ownerId];
      if (c) applyPatch(c, { couleur }); // couleur du personnage = couleur du pion
    }
    if (isMj()) {
      if (label !== undefined && !t.ownerId && str(label, 30).trim()) t.label = str(label, 30).trim();
      if (taille !== undefined) t.taille = num(taille, 1, 4, t.taille);
    }
    broadcastState();
    save();
  });

  socket.on('token:image', ({ id, data } = {}, ack) => {
    const t = state.tokens[id];
    if (!canEditToken(t)) return fail(ack, 'Ce pion ne t\'appartient pas.');
    const m = /^data:image\/(png|jpeg|webp);base64,(.+)$/.exec(String(data || ''));
    if (!m || m[2].length > 4e6) return fail(ack, 'Image invalide ou trop lourde.');
    const file = `pion-${uid()}.${m[1] === 'jpeg' ? 'jpg' : m[1]}`;
    fs.writeFileSync(path.join(IMG_DIR, file), Buffer.from(m[2], 'base64'));
    removeTokenImage(t);
    t.skin.image = '/images/' + file;
    t.skin.contenu = 'image';
    broadcastState();
    save();
    ok(ack);
  });

  socket.on('token:remove', mjOnly(({ id } = {}) => {
    if (!state.tokens[id]) return;
    removeTokenImage(state.tokens[id]);
    delete state.tokens[id];
    state.initiative.entries = state.initiative.entries.filter((e) => e.tokenId !== id);
    broadcastState();
    save();
  }));

  // ---- scènes / cartes
  socket.on('scene:upload', mjOnly(({ titre, data } = {}, ack) => {
    const m = /^data:image\/(png|jpeg|webp|gif);base64,(.+)$/.exec(String(data || ''));
    if (!m) return fail(ack, 'Image invalide (PNG, JPG, WEBP ou GIF).');
    const id = uid();
    const file = `${id}.${m[1] === 'jpeg' ? 'jpg' : m[1]}`;
    fs.writeFileSync(path.join(IMG_DIR, file), Buffer.from(m[2], 'base64'));
    const scene = { id, titre: str(titre, 60).trim() || 'Scène sans titre', image: '/images/' + file };
    state.scenes.push(scene);
    state.scene.actif = id;
    system(`Nouvelle scène : ${scene.titre}`);
    broadcastState();
    save();
    ok(ack);
  }));

  socket.on('scene:activer', mjOnly(({ id } = {}) => {
    const s = state.scenes.find((x) => x.id === id);
    state.scene.actif = s ? s.id : null;
    if (s) system(`Nouvelle scène : ${s.titre}`);
    broadcastState();
    save();
  }));

  socket.on('scene:supprimer', mjOnly(({ id } = {}) => {
    const s = state.scenes.find((x) => x.id === id);
    if (!s) return;
    state.scenes = state.scenes.filter((x) => x.id !== id);
    if (state.scene.actif === id) state.scene.actif = null;
    if (s.image) fs.unlink(path.join(IMG_DIR, path.basename(s.image)), () => {});
    broadcastState();
    save();
  }));

  // ---- battle maps
  socket.on('scene:battlemap', mjOnly(({ titre, cols, rows, fond } = {}, ack) => {
    const scene = {
      id: uid(), titre: str(titre, 60).trim() || 'Battle map', image: null,
      cols: num(cols, 4, 100, 20), rows: num(rows, 4, 100, 14),
      fond: SOLS.includes(fond) ? fond : 'herbe', sol: {}, deco: {},
    };
    state.scenes.push(scene);
    state.scene.actif = scene.id;
    system(`Nouvelle scène : ${scene.titre}`);
    broadcastState();
    save();
    ok(ack, { id: scene.id });
  }));

  socket.on('map:taille', mjOnly(({ cols, rows } = {}) => {
    const s = activeScene();
    if (!s || s.image) return;
    s.cols = num(cols, 4, 100, s.cols);
    s.rows = num(rows, 4, 100, s.rows);
    pruneMap(s);
    broadcastState();
    save();
  }));

  socket.on('map:fond', mjOnly(({ fond } = {}) => {
    const s = activeScene();
    if (!s) return;
    if (SOLS.includes(fond)) s.fond = fond;
    else if (s.image) s.fond = null; // une carte importée peut rester sans fond peint
    broadcastState();
    save();
  }));

  socket.on('map:effacer', mjOnly(({ calque } = {}) => {
    const s = activeScene();
    if (!s || !['sol', 'deco'].includes(calque)) return;
    s[calque] = {};
    broadcastState();
    save();
  }));

  // Coups de pinceau : on ne renvoie aux autres que les cases modifiées (léger même en peignant vite).
  socket.on('map:paint', mjOnly(({ calque, cells } = {}) => {
    const s = activeScene();
    if (!s || !['sol', 'deco'].includes(calque) || !Array.isArray(cells)) return;
    const allowed = calque === 'sol' ? SOLS : DECOS;
    const layer = s[calque] || (s[calque] = {});
    const maxC = s.image ? 400 : s.cols;
    const maxR = s.image ? 400 : s.rows;
    const applied = [];
    for (const it of cells.slice(0, 12000)) {
      if (!Array.isArray(it)) continue;
      const c = Number(it[0]);
      const r = Number(it[1]);
      if (!Number.isInteger(c) || !Number.isInteger(r) || c < 0 || r < 0 || c >= maxC || r >= maxR) continue;
      const v = it[2] == null ? null : String(it[2]);
      if (v !== null && !allowed.includes(v)) continue;
      if (v === null) delete layer[c + ',' + r]; else layer[c + ',' + r] = v;
      applied.push([c, r, v]);
    }
    if (!applied.length) return;
    for (const so of io.sockets.sockets.values()) {
      if (so.data.role && so !== socket) so.emit('map:patch', { sceneId: s.id, calque, cells: applied });
    }
    save();
  }));

  socket.on('scene:grille', mjOnly(({ grille, cellule } = {}) => {
    if (grille !== undefined) state.scene.grille = !!grille;
    if (cellule !== undefined) state.scene.cellule = num(cellule, 10, 400, 70);
    broadcastState();
    save();
  }));

  // ---- initiative
  const ini = state.initiative;
  const announceTurn = () => {
    const e = ini.entries.find((x) => x.id === ini.actif);
    if (e && !e.cache) system(`⚔ Au tour de ${e.name} !`);
  };

  socket.on('init:roll', () => {
    const c = state.characters[socket.data.charId];
    if (!c) return;
    const roll = rollFormula('1d20' + fmtMod(mod(c.stats.dex)));
    addLog({ type: 'roll', from: who(), label: 'Initiative', roll, visibleTo: null });
    const id = 'pc-' + c.id;
    const e = ini.entries.find((x) => x.id === id);
    if (e) e.valeur = roll.total;
    else ini.entries.push({ id, name: c.name, couleur: c.couleur, charId: c.id, valeur: roll.total, cache: false });
    sortInitiative();
    broadcastState();
    save();
  });

  socket.on('init:add', mjOnly(({ name, valeur, cache, couleur, tokenId } = {}) => {
    name = str(name, 30).trim();
    if (!name) return;
    let v = Number(valeur);
    if (valeur === '' || valeur == null || !Number.isFinite(v)) {
      const roll = rollFormula('1d20');
      addLog({ type: 'roll', from: who(), label: `Initiative de ${name}`, roll, secret: true, visibleTo: [] });
      v = roll.total;
    }
    ini.entries.push({ id: uid(), name, couleur: isColor(couleur) ? couleur : '#b33', tokenId: tokenId || null, valeur: num(v, -99, 999), cache: !!cache });
    sortInitiative();
    broadcastState();
    save();
  }));

  socket.on('init:set', mjOnly(({ id, valeur, cache } = {}) => {
    const e = ini.entries.find((x) => x.id === id);
    if (!e) return;
    if (valeur !== undefined) e.valeur = num(valeur, -99, 999, e.valeur);
    if (cache !== undefined) e.cache = !!cache;
    sortInitiative();
    broadcastState();
    save();
  }));

  socket.on('init:remove', mjOnly(({ id } = {}) => {
    ini.entries = ini.entries.filter((x) => x.id !== id);
    if (ini.actif === id) ini.actif = null;
    broadcastState();
    save();
  }));

  socket.on('init:step', mjOnly(({ dir } = {}) => {
    const list = ini.entries;
    if (!list.length) return;
    let i = list.findIndex((x) => x.id === ini.actif);
    if (i < 0) i = dir < 0 ? list.length - 1 : 0;
    else if (dir < 0) {
      i -= 1;
      if (i < 0) { i = list.length - 1; ini.round = Math.max(1, ini.round - 1); }
    } else {
      i += 1;
      if (i >= list.length) { i = 0; ini.round += 1; system(`— Round ${ini.round} —`); }
    }
    ini.actif = list[i].id;
    announceTurn();
    broadcastState();
    save();
  }));

  // ---- diaporamas (MJ uniquement)
  const changed = () => { broadcastState(); save(); };

  socket.on('slides:deck:create', mjOnly(({ titre } = {}, ack) => {
    const deck = { id: uid(), titre: str(titre, 80).trim() || 'Nouveau diaporama', slides: [] };
    state.slides.decks.push(deck);
    changed();
    ok(ack, { id: deck.id });
  }));
  socket.on('slides:deck:update', mjOnly(({ id, titre } = {}) => {
    const deck = findDeck(id);
    if (!deck) return;
    if (titre !== undefined) deck.titre = str(titre, 80).trim() || deck.titre;
    changed();
  }));
  socket.on('slides:deck:delete', mjOnly(({ id } = {}) => {
    state.slides.decks = state.slides.decks.filter((d) => d.id !== id);
    if (state.slides.projection && state.slides.projection.deckId === id) state.slides.projection = null;
    changed();
  }));

  socket.on('slides:slide:add', mjOnly(({ deckId, after } = {}, ack) => {
    const deck = findDeck(deckId);
    if (!deck) return fail(ack, 'Diaporama introuvable');
    const slide = newSlide({ titre: 'Nouvelle slide' });
    const i = deck.slides.findIndex((s) => s.id === after);
    deck.slides.splice(i >= 0 ? i + 1 : deck.slides.length, 0, slide);
    changed();
    ok(ack, { id: slide.id });
  }));
  socket.on('slides:slide:update', mjOnly(({ deckId, slideId, patch } = {}) => {
    const slide = findDeck(deckId)?.slides.find((s) => s.id === slideId);
    if (!slide || !patch) return;
    const clean = newSlide({ ...slide, ...patch });
    Object.assign(slide, { titre: clean.titre, texte: clean.texte, style: clean.style, image: 'image' in patch ? clean.image : slide.image });
    changed();
  }));
  socket.on('slides:slide:delete', mjOnly(({ deckId, slideId } = {}) => {
    const deck = findDeck(deckId);
    if (!deck) return;
    deck.slides = deck.slides.filter((s) => s.id !== slideId);
    const p = state.slides.projection;
    if (p && p.deckId === deckId && p.index >= deck.slides.length) p.index = Math.max(0, deck.slides.length - 1);
    changed();
  }));
  socket.on('slides:slide:move', mjOnly(({ deckId, slideId, dir } = {}) => {
    const deck = findDeck(deckId);
    if (!deck) return;
    const i = deck.slides.findIndex((s) => s.id === slideId);
    const j = i + (dir < 0 ? -1 : 1);
    if (i < 0 || j < 0 || j >= deck.slides.length) return;
    [deck.slides[i], deck.slides[j]] = [deck.slides[j], deck.slides[i]];
    changed();
  }));
  socket.on('slides:slide:image', mjOnly(({ deckId, slideId, data } = {}, ack) => {
    const slide = findDeck(deckId)?.slides.find((s) => s.id === slideId);
    if (!slide) return fail(ack, 'Slide introuvable');
    const m = /^data:image\/(png|jpeg|webp|gif);base64,(.+)$/.exec(String(data || ''));
    if (!m) return fail(ack, 'Image invalide (PNG, JPG, WEBP ou GIF).');
    const file = `slide-${uid()}.${m[1] === 'jpeg' ? 'jpg' : m[1]}`;
    fs.writeFileSync(path.join(IMG_DIR, file), Buffer.from(m[2], 'base64'));
    slide.image = '/images/' + file;
    changed();
    ok(ack);
  }));

  socket.on('slides:generate', mjOnly(async ({ deckId, prompt, nombre, ton, inclureGroupe } = {}, ack) => {
    const deck = findDeck(deckId);
    prompt = str(prompt, 2000).trim();
    if (!deck) return fail(ack, 'Choisis ou crée d\'abord un diaporama.');
    if (!prompt) return fail(ack, 'Décris ce que les slides doivent raconter.');
    const groupe = inclureGroupe
      ? Object.values(state.characters).map((c) => [c.name, c.race, c.classe && `${c.classe} niv. ${c.niveau}`].filter(Boolean).join(', '))
      : [];
    try {
      const slides = await generateSlides({ prompt, nombre: num(nombre, 1, 10, 3), ton: str(ton, 40), deck, groupe });
      if (!slides.length) return fail(ack, 'L\'IA n\'a renvoyé aucune slide, réessaie.');
      const target = findDeck(deckId); // le diaporama a pu être supprimé pendant la génération
      if (!target) return fail(ack, 'Le diaporama a été supprimé entre-temps.');
      target.slides.push(...slides);
      changed();
      ok(ack, { ajoutees: slides.length, premier: slides[0].id });
    } catch (e) {
      console.error('Génération de slides :', e.message);
      fail(ack, iaErrorMessage(e));
    }
  }));

  socket.on('slides:present', mjOnly(({ deckId, index } = {}) => {
    const deck = findDeck(deckId);
    if (!deck || !deck.slides.length) return;
    state.slides.projection = { deckId, index: num(index, 0, deck.slides.length - 1, 0) };
    changed();
  }));
  socket.on('slides:step', mjOnly(({ dir } = {}) => {
    const p = state.slides.projection;
    const deck = p && findDeck(p.deckId);
    if (!deck) return;
    p.index = Math.min(deck.slides.length - 1, Math.max(0, p.index + (dir < 0 ? -1 : 1)));
    changed();
  }));
  socket.on('slides:stop', mjOnly(() => {
    state.slides.projection = null;
    changed();
  }));

  socket.on('init:clear', mjOnly(() => {
    ini.entries = [];
    ini.actif = null;
    ini.round = 1;
    system('Fin du combat.');
    broadcastState();
    save();
  }));
});

// ---------------------------------------------------------------- tunnel Cloudflare (accès Internet)
const CLOUDFLARED_URL = 'https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe';

/** Trouve cloudflared ; sous Windows, le télécharge dans le dossier des données s'il manque (une seule fois). */
async function findCloudflared() {
  const local = path.join(USER_DIR, 'cloudflared.exe');
  if (fs.existsSync(local)) return local;
  if (!spawnSync('cloudflared', ['--version'], { windowsHide: true }).error) return 'cloudflared';
  if (process.platform !== 'win32' || process.arch !== 'x64') return null;
  console.log('  Téléchargement de cloudflared pour le lien Internet (une seule fois, ~60 Mo)…');
  try {
    const res = await fetch(CLOUDFLARED_URL);
    if (!res.ok) throw new Error('HTTP ' + res.status);
    fs.writeFileSync(local + '.part', Buffer.from(await res.arrayBuffer()));
    fs.renameSync(local + '.part', local);
    console.log('  cloudflared installé.\n');
    return local;
  } catch (e) {
    console.log('  Échec du téléchargement de cloudflared : ' + e.message);
    return null;
  }
}

async function startTunnel() {
  const bin = await findCloudflared();
  if (!bin) return tunnelMissing();
  console.log('  Ouverture du lien Internet…');
  let child;
  try {
    child = spawn(bin, ['tunnel', '--no-autoupdate', '--url', `http://localhost:${config.port}`], { windowsHide: true });
  } catch { return tunnelMissing(); }
  child.on('error', tunnelMissing);
  const onData = (buf) => {
    const m = /https:\/\/[a-z0-9-]+\.trycloudflare\.com/.exec(buf.toString());
    if (m && m[0] !== tunnelUrl) {
      tunnelUrl = m[0];
      console.log('\n  🌍 Lien Internet à envoyer aux joueurs :  ' + tunnelUrl + '\n');
      broadcastState();
    }
  };
  child.stdout.on('data', onData);
  child.stderr.on('data', onData);
  child.on('exit', () => { tunnelUrl = null; broadcastState(); });
  const stop = () => { try { child.kill(); } catch { /* déjà arrêté */ } };
  process.on('exit', stop);
}
function tunnelMissing() {
  console.log('  (cloudflared introuvable : pas de lien Internet automatique.');
  console.log('   Installe-le avec :  winget install --id Cloudflare.cloudflared  puis relance.)\n');
}

const quit = () => { saveNow(); process.exit(0); };
process.on('SIGINT', quit);
process.on('SIGTERM', quit);
process.on('SIGHUP', quit); // fermeture de la fenêtre sous Windows

server.on('error', (e) => {
  if (e.code === 'EADDRINUSE') {
    console.log(`\n  Le port ${config.port} est déjà utilisé : La Taverne est peut-être déjà ouverte.`);
    console.log('  Ferme l\'autre fenêtre, ou change "port" dans config.json.\n');
  } else console.error(e);
  if (sea) setTimeout(() => process.exit(1), 15000); else process.exit(1);
});

server.listen(config.port, '0.0.0.0', () => {
  const url = `http://localhost:${config.port}`;
  console.log('\n  ⚔  La Taverne est ouverte !\n');
  console.log(`  Sur ce PC          :  ${url}`);
  for (const u of lanUrls()) console.log(`  Réseau local       :  ${u}`);
  console.log(`  Mot de passe MJ    :  ${config.motDePasseMJ}   (modifiable dans config.json)`);
  console.log(`  Données            :  ${DATA_DIR}`);
  console.log('\n  Laisse cette fenêtre ouverte pendant la partie (la fermer arrête le serveur).\n');
  if (config.ouvrirNavigateur && process.platform === 'win32') {
    spawn('cmd', ['/c', 'start', '', url], { detached: true, stdio: 'ignore', windowsHide: true }).unref();
  }
  if (config.tunnelCloudflare) startTunnel();
});
