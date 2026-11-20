'use strict';
/* Petits personnages (figurines chibi) dessinés en SVG à partir d'options simples. */

const FIG = {
  peaux: ['#f6d3b3', '#e8b48a', '#c68642', '#8d5524', '#5c3a1e', '#7fae5a', '#8a9bb0', '#b5473a', '#9fd3e6'],
  cheveuxCouleurs: ['#2b1a10', '#6b3e1f', '#c98a3a', '#e8d27a', '#f4f1e8', '#b8b8b8', '#a3271b', '#2f4fa8', '#7d3fb0', '#2e8b57'],
  options: {
    cheveux: [['chauve', 'Chauve'], ['court', 'Courts'], ['long', 'Longs'], ['herisse', 'Hérissés'], ['queue', 'Queue'], ['chignon', 'Chignon']],
    barbe: [['aucune', 'Aucune'], ['courte', 'Courte'], ['longue', 'Longue'], ['moustache', 'Moustache']],
    oreilles: [['humaines', 'Humaines'], ['elfiques', 'Elfiques']],
    tenue: [['tunique', 'Tunique'], ['robe', 'Robe'], ['armure', 'Armure'], ['cuir', 'Cuir']],
    tete: [['rien', 'Rien'], ['casque', 'Casque'], ['capuche', 'Capuche'], ['mage', 'Chapeau'], ['couronne', 'Couronne'], ['cornes', 'Cornes'], ['bandeau', 'Bandeau']],
    arme: [['aucune', 'Aucune'], ['epee', 'Épée'], ['hache', 'Hache'], ['baton', 'Bâton'], ['arc', 'Arc'], ['dague', 'Dague'], ['masse', 'Masse'], ['lance', 'Lance']],
  },
  modeles: {
    Guerrier: { tenue: 'armure', couleurTenue: '#7a1f1f', arme: 'epee', bouclier: true, tete: 'casque', cape: true, couleurCape: '#7a1f1f', barbe: 'aucune', oreilles: 'humaines', crocs: false },
    Mage: { tenue: 'robe', couleurTenue: '#2f4fa8', arme: 'baton', bouclier: false, tete: 'mage', cape: false, cheveux: 'long', couleurCheveux: '#f4f1e8', barbe: 'longue', oreilles: 'humaines', crocs: false },
    Roublard: { tenue: 'cuir', couleurTenue: '#3a3a3a', arme: 'dague', bouclier: false, tete: 'capuche', cape: true, couleurCape: '#2a2a2a', barbe: 'aucune', crocs: false },
    Clerc: { tenue: 'robe', couleurTenue: '#e8e2d0', arme: 'masse', bouclier: true, tete: 'rien', cape: true, couleurCape: '#c9a43a', crocs: false },
    Rôdeur: { tenue: 'cuir', couleurTenue: '#2f6b2f', arme: 'arc', bouclier: false, tete: 'capuche', cape: true, couleurCape: '#2f5a2a', oreilles: 'elfiques', crocs: false },
    Barbare: { tenue: 'cuir', couleurTenue: '#7a4a22', arme: 'hache', bouclier: false, tete: 'bandeau', cape: false, cheveux: 'long', barbe: 'courte', crocs: false },
    Paladin: { tenue: 'armure', couleurTenue: '#c9a43a', arme: 'epee', bouclier: true, tete: 'rien', cape: true, couleurCape: '#f4f1e8', crocs: false },
    Nain: { tenue: 'armure', couleurTenue: '#8a6a2a', arme: 'hache', bouclier: true, tete: 'casque', barbe: 'longue', couleurCheveux: '#a3271b', cape: false, oreilles: 'humaines', crocs: false },
    Elfe: { tenue: 'tunique', couleurTenue: '#2e8b57', arme: 'arc', oreilles: 'elfiques', cheveux: 'long', couleurCheveux: '#e8d27a', barbe: 'aucune', tete: 'rien', crocs: false },
    Orc: { peau: '#7fae5a', crocs: true, tenue: 'cuir', couleurTenue: '#5a3a1a', arme: 'hache', cheveux: 'queue', couleurCheveux: '#2b1a10', barbe: 'aucune', tete: 'rien', oreilles: 'humaines' },
    Tieffelin: { peau: '#b5473a', tete: 'cornes', tenue: 'robe', couleurTenue: '#3a1f4a', arme: 'dague', cheveux: 'long', couleurCheveux: '#2b1a10', barbe: 'aucune', oreilles: 'elfiques', crocs: false },
  },
};

function figRandom() {
  const r = (l) => l[Math.floor(Math.random() * l.length)];
  const o = FIG.options;
  const tenues = ['#24456b', '#7a1f1f', '#2f6b2f', '#5b2a86', '#8a6a2a', '#3a3a3a', '#e8e2d0', '#c9a43a'];
  return {
    peau: r(FIG.peaux), cheveux: r(o.cheveux)[0], couleurCheveux: r(FIG.cheveuxCouleurs),
    barbe: Math.random() < 0.3 ? r(o.barbe)[0] : 'aucune', oreilles: Math.random() < 0.3 ? 'elfiques' : 'humaines',
    crocs: Math.random() < 0.12, tenue: r(o.tenue)[0], couleurTenue: r(tenues),
    tete: Math.random() < 0.5 ? 'rien' : r(o.tete)[0], arme: r(o.arme.slice(1))[0], bouclier: Math.random() < 0.3,
    cape: Math.random() < 0.6, couleurCape: r(tenues),
  };
}

/** Assombrit / éclaircit une couleur #rrggbb (k entre -1 et 1). */
function shade(hex, k) {
  const m = /^#([0-9a-f]{6})$/i.exec(hex || '');
  if (!m) return '#777777';
  const n = parseInt(m[1], 16);
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => Math.round(k < 0 ? c * (1 + k) : c + (255 - c) * k));
  return '#' + ch.map((c) => c.toString(16).padStart(2, '0')).join('');
}
const safeColor = (c, d) => (/^#[0-9a-f]{6}$/i.test(c || '') ? c : d);

/** SVG de la figurine. mode 'pied' = en entier (socle), 'buste' = cadré pour un médaillon. */
function figurineSVG(f = {}, mode = 'pied') {
  const O = '#2a1a10'; // contour
  const peau = safeColor(f.peau, '#e8b48a');
  const hair = safeColor(f.couleurCheveux, '#6b3e1f');
  const cloth = safeColor(f.couleurTenue, '#24456b');
  const cape = safeColor(f.couleurCape, '#7a1f1f');
  const steel = '#c3cad3';
  const wood = '#7a4a22';
  const gold = '#e0b040';
  const sw = 'stroke="' + O + '" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"';
  const p = [];

  // --- cape (derrière)
  if (f.cape) p.push(`<path d="M31 63 L69 63 L80 108 Q50 115 20 108 Z" fill="${cape}" ${sw}/>`, `<path d="M31 63 L69 63 L72 72 L28 72 Z" fill="${shade(cape, -0.25)}"/>`);
  // --- capuche (arrière)
  if (f.tete === 'capuche') p.push(`<path d="M20 50 Q16 6 50 6 Q84 6 80 50 Q80 68 66 68 L34 68 Q20 68 20 50 Z" fill="${cape}" ${sw}/>`);
  // --- cheveux longs / queue (arrière)
  const hideHair = f.tete === 'casque' || f.tete === 'capuche';
  if (!hideHair && f.cheveux === 'long') p.push(`<path d="M24 40 Q22 12 50 12 Q78 12 76 40 L80 78 Q66 74 62 64 L38 64 Q34 74 20 78 Z" fill="${hair}" ${sw}/>`);
  if (!hideHair && f.cheveux === 'queue') p.push(`<path d="M70 30 Q90 40 84 70 Q80 80 74 74 Q80 56 66 40 Z" fill="${hair}" ${sw}/>`);

  // --- jambes et bottes
  const pants = f.tenue === 'armure' ? shade(steel, -0.25) : shade(cloth, -0.45);
  p.push(`<rect x="38" y="88" width="10" height="16" rx="3" fill="${pants}" ${sw}/>`, `<rect x="52" y="88" width="10" height="16" rx="3" fill="${pants}" ${sw}/>`);
  p.push(`<rect x="35" y="100" width="14" height="9" rx="4" fill="#3a2414" ${sw}/>`, `<rect x="51" y="100" width="14" height="9" rx="4" fill="#3a2414" ${sw}/>`);

  // --- torse selon la tenue
  const belt = `<rect x="33" y="82" width="34" height="5" fill="#3a2414" ${sw}/><rect x="47" y="81.5" width="6" height="6" rx="1" fill="${gold}" ${sw}/>`;
  if (f.tenue === 'robe') {
    p.push(`<path d="M34 62 Q50 58 66 62 L76 106 Q50 112 24 106 Z" fill="${cloth}" ${sw}/>`,
      `<path d="M50 62 L50 108" stroke="${shade(cloth, -0.3)}" stroke-width="2"/>`,
      `<path d="M34 80 Q50 85 66 80 L66 85 Q50 90 34 85 Z" fill="${gold}" ${sw}/>`);
  } else if (f.tenue === 'armure') {
    p.push(`<path d="M33 62 Q50 57 67 62 L68 93 Q50 97 32 93 Z" fill="${steel}" ${sw}/>`,
      `<path d="M40 66 Q50 72 60 66 M50 70 L50 82" stroke="${shade(steel, -0.35)}" stroke-width="2" fill="none"/>`,
      `<path d="M36 89 L64 89" stroke="${cloth}" stroke-width="4"/>`, belt,
      `<ellipse cx="32" cy="65" rx="8" ry="6" fill="${cloth}" ${sw}/>`, `<ellipse cx="68" cy="65" rx="8" ry="6" fill="${cloth}" ${sw}/>`);
  } else if (f.tenue === 'cuir') {
    p.push(`<path d="M33 62 Q50 58 67 62 L68 94 Q50 97 32 94 Z" fill="${cloth}" ${sw}/>`,
      `<path d="M36 63 L64 88" stroke="#3a2414" stroke-width="4"/>`,
      `<circle cx="44" cy="70" r="1.6" fill="${gold}"/><circle cx="56" cy="80" r="1.6" fill="${gold}"/>`, belt);
  } else {
    p.push(`<path d="M33 62 Q50 58 67 62 L68 94 Q50 97 32 94 Z" fill="${cloth}" ${sw}/>`,
      `<path d="M44 61 L50 70 L56 61" fill="${shade(cloth, 0.25)}" ${sw}/>`, belt);
  }

  // --- bras gauche (à l'écran) + bouclier
  const sleeve = f.tenue === 'armure' ? steel : cloth;
  p.push(`<path d="M33 65 Q26 74 28 86" stroke="${O}" stroke-width="10" fill="none" stroke-linecap="round"/>`,
    `<path d="M33 65 Q26 74 28 86" stroke="${sleeve}" stroke-width="6.5" fill="none" stroke-linecap="round"/>`,
    `<circle cx="28" cy="87" r="4.6" fill="${peau}" ${sw}/>`);
  if (f.bouclier) {
    p.push(`<path d="M15 72 L39 72 L39 86 Q39 97 27 102 Q15 97 15 86 Z" fill="${cloth}" ${sw}/>`,
      `<path d="M18 75 L36 75 L36 86 Q36 94 27 98 Q18 94 18 86 Z" fill="none" stroke="${gold}" stroke-width="1.6"/>`,
      `<path d="M27 78 L27 94 M21 84 L33 84" stroke="${gold}" stroke-width="2.4"/>`);
  }

  // --- arme (main droite à l'écran)
  const W = {
    epee: `<rect x="71.5" y="38" width="5" height="44" rx="1" fill="${steel}" ${sw}/><path d="M74 38 L74 80" stroke="#fff" stroke-width="1" opacity=".7"/><rect x="66" y="80" width="16" height="4" rx="1.5" fill="${gold}" ${sw}/><rect x="72.5" y="84" width="3" height="9" fill="${wood}" ${sw}/><circle cx="74" cy="94" r="2.6" fill="${gold}" ${sw}/>`,
    hache: `<path d="M74 46 L74 100" stroke="${O}" stroke-width="5" stroke-linecap="round"/><path d="M74 46 L74 100" stroke="${wood}" stroke-width="2.6" stroke-linecap="round"/><path d="M75 48 Q94 44 94 62 Q86 60 75 66 Z" fill="${steel}" ${sw}/>`,
    baton: `<path d="M74 26 L74 108" stroke="${O}" stroke-width="5.5" stroke-linecap="round"/><path d="M74 26 L74 108" stroke="${wood}" stroke-width="3" stroke-linecap="round"/><circle cx="74" cy="22" r="7" fill="#6ad1ff" ${sw}/><circle cx="71.5" cy="19.5" r="2.2" fill="#fff" opacity=".8"/>`,
    arc: `<path d="M76 50 Q96 80 76 110" stroke="${O}" stroke-width="5" fill="none" stroke-linecap="round"/><path d="M76 50 Q96 80 76 110" stroke="${wood}" stroke-width="2.6" fill="none" stroke-linecap="round"/><path d="M76 50 L76 110" stroke="#eee" stroke-width="1"/>`,
    dague: `<path d="M72 66 L74 58 L76 66 L76 80 L72 80 Z" fill="${steel}" ${sw}/><rect x="69" y="80" width="10" height="3" rx="1" fill="${gold}" ${sw}/><rect x="72.5" y="83" width="3" height="7" fill="${wood}" ${sw}/>`,
    masse: `<path d="M74 60 L74 98" stroke="${O}" stroke-width="5" stroke-linecap="round"/><path d="M74 60 L74 98" stroke="${wood}" stroke-width="2.6" stroke-linecap="round"/><path d="M74 46 L78 52 L84 52 L80 58 L82 64 L74 61 L66 64 L68 58 L64 52 L70 52 Z" fill="${steel}" ${sw}/>`,
    lance: `<path d="M74 10 L74 110" stroke="${O}" stroke-width="5" stroke-linecap="round"/><path d="M74 10 L74 110" stroke="${wood}" stroke-width="2.6" stroke-linecap="round"/><path d="M70 16 L74 -2 L78 16 Z" fill="${steel}" ${sw}/>`,
  };
  if (W[f.arme]) p.push(f.arme === 'arc' ? W.arc : `<g transform="rotate(${f.arme === 'baton' || f.arme === 'lance' ? 12 : 28} 74 87)">${W[f.arme]}</g>`);
  // bras droit + main (par-dessus la poignée)
  p.push(`<path d="M67 65 Q74 74 73 86" stroke="${O}" stroke-width="10" fill="none" stroke-linecap="round"/>`,
    `<path d="M67 65 Q74 74 73 86" stroke="${sleeve}" stroke-width="6.5" fill="none" stroke-linecap="round"/>`,
    `<circle cx="74" cy="87" r="4.6" fill="${peau}" ${sw}/>`);

  // --- oreilles
  if (f.oreilles === 'elfiques') {
    p.push(`<path d="M29 38 L10 26 L28 50 Z" fill="${peau}" ${sw}/>`, `<path d="M71 38 L90 26 L72 50 Z" fill="${peau}" ${sw}/>`);
  } else {
    p.push(`<circle cx="27" cy="43" r="5" fill="${peau}" ${sw}/>`, `<circle cx="73" cy="43" r="5" fill="${peau}" ${sw}/>`);
  }
  // --- tête
  p.push(`<circle cx="50" cy="40" r="24" fill="${peau}" ${sw}/>`,
    `<ellipse cx="36" cy="50" rx="4.5" ry="2.6" fill="#ff6b6b" opacity=".3"/><ellipse cx="64" cy="50" rx="4.5" ry="2.6" fill="#ff6b6b" opacity=".3"/>`);
  // yeux, sourcils, bouche
  p.push(`<ellipse cx="41" cy="43" rx="3.3" ry="4.2" fill="${O}"/><ellipse cx="59" cy="43" rx="3.3" ry="4.2" fill="${O}"/>`,
    `<circle cx="42.2" cy="41.4" r="1.3" fill="#fff"/><circle cx="60.2" cy="41.4" r="1.3" fill="#fff"/>`,
    `<path d="M36 35 L45 34 M55 34 L64 35" stroke="${shade(hair, -0.2)}" stroke-width="2.2" stroke-linecap="round"/>`,
    `<path d="M45 52 Q50 56 55 52" stroke="#6b2a1a" stroke-width="1.8" fill="none" stroke-linecap="round"/>`);
  if (f.crocs) p.push(`<path d="M44 53 L45 47 L47.5 53 Z M52.5 53 L55 47 L56 53 Z" fill="#fffbe8" ${sw.replace('1.8', '1')}/>`);
  // barbe
  const B = {
    courte: `<path d="M30 45 Q31 64 50 66 Q69 64 70 45 Q64 55 50 56 Q36 55 30 45 Z" fill="${hair}" ${sw}/><path d="M45 54 Q50 57 55 54" stroke="#6b2a1a" stroke-width="1.6" fill="none"/>`,
    longue: `<path d="M29 45 Q28 72 50 88 Q72 72 71 45 Q64 55 50 56 Q36 55 29 45 Z" fill="${hair}" ${sw}/><path d="M42 64 Q46 74 50 80 M58 64 Q54 74 50 80" stroke="${shade(hair, -0.3)}" stroke-width="1.4" fill="none"/><path d="M45 54 Q50 57 55 54" stroke="#6b2a1a" stroke-width="1.6" fill="none"/>`,
    moustache: `<path d="M41 52 Q45 48 50 51 Q55 48 59 52 Q55 55 50 53 Q45 55 41 52 Z" fill="${hair}" ${sw}/>`,
  };
  if (B[f.barbe]) p.push(B[f.barbe]);

  // --- cheveux (devant)
  if (!hideHair) {
    const fringe = `<path d="M26 38 Q25 14 50 14 Q75 14 74 38 Q67 28 58 30 Q50 23 42 30 Q33 28 26 38 Z" fill="${hair}" ${sw}/>`;
    const H = {
      court: fringe, long: fringe, queue: fringe,
      herisse: `<path d="M26 38 L22 22 L33 25 L34 10 L45 20 L51 5 L57 20 L68 10 L68 25 L79 22 L74 38 Q66 28 50 28 Q34 28 26 38 Z" fill="${hair}" ${sw}/>`,
      chignon: `<circle cx="50" cy="12" r="8" fill="${hair}" ${sw}/>${fringe}`,
    };
    if (H[f.cheveux]) p.push(H[f.cheveux]);
  }

  // --- couvre-chef
  const T = {
    casque: `<path d="M24 42 Q23 13 50 13 Q77 13 76 42 L68 42 L68 36 L32 36 L32 42 Z" fill="${steel}" ${sw}/><rect x="47.5" y="34" width="5" height="14" rx="1" fill="${steel}" ${sw}/><path d="M50 13 L50 30" stroke="${shade(steel, -0.3)}" stroke-width="2"/><circle cx="30" cy="38" r="1.3" fill="${O}"/><circle cx="70" cy="38" r="1.3" fill="${O}"/>`,
    capuche: `<path d="M24 46 Q22 14 50 14 Q78 14 76 46" stroke="${shade(cape, -0.35)}" stroke-width="5" fill="none"/>`,
    mage: `<ellipse cx="50" cy="20" rx="32" ry="7" fill="${cloth}" ${sw}/><path d="M32 19 Q42 2 58 -18 Q60 -4 68 19 Q50 23 32 19 Z" fill="${cloth}" ${sw}/><path d="M35 15 Q50 19 65 15 L66 19 Q50 23 34 19 Z" fill="${gold}"/><path d="M52 2 l1.5 3 3 .4 -2.2 2 .6 3 -2.9 -1.5 -2.9 1.5 .6 -3 -2.2 -2 3 -.4 Z" fill="${gold}"/>`,
    couronne: `<path d="M33 22 L33 8 L41 15 L50 4 L59 15 L67 8 L67 22 Z" fill="${gold}" ${sw}/><circle cx="50" cy="17" r="2.4" fill="#d32f2f"/><circle cx="40" cy="18" r="1.6" fill="#2f6bd3"/><circle cx="60" cy="18" r="1.6" fill="#2f6bd3"/>`,
    cornes: `<path d="M36 22 Q24 12 28 -4 Q33 10 43 17 Z" fill="#3a2a2a" ${sw}/><path d="M64 22 Q76 12 72 -4 Q67 10 57 17 Z" fill="#3a2a2a" ${sw}/>`,
    bandeau: `<path d="M26 30 Q50 22 74 30 L74 36 Q50 28 26 36 Z" fill="${cloth}" ${sw}/><path d="M74 32 Q84 34 86 44 M74 34 Q80 40 80 50" stroke="${cloth}" stroke-width="3" fill="none" stroke-linecap="round"/>`,
  };
  if (T[f.tete]) p.push(T[f.tete]);

  const vb = mode === 'buste' ? '8 -2 84 84' : '0 -20 100 132';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${vb}" class="fig-svg" overflow="visible">${p.join('')}</svg>`;
}
