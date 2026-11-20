'use strict';
/* Dés animés : ils roulent sur la table puis s'arrêtent sur le résultat tiré par le serveur. */

const DICE_ANIM_MS = 1500;

/** Forme SVG d'un dé (vue de dessus) selon son nombre de faces. */
let dieGradSeq = 0;
function dieShapeSVG(faces, color, value) {
  const gid = "dg" + (++dieGradSeq);
  const c = /^#[0-9a-f]{6}$/i.test(color || '') ? color : '#b33';
  const dark = typeof shade === 'function' ? shade(c, -0.45) : '#222';
  const light = typeof shade === 'function' ? shade(c, 0.35) : '#fff';
  const line = `stroke="${dark}" stroke-width="2" fill="none" opacity=".55"`;
  const out = `stroke="${dark}" stroke-width="3" stroke-linejoin="round"`;
  let body = '';
  let ty = 58; // position verticale du chiffre
  let fs = 34;
  switch (faces) {
    case 4:
      body = `<polygon points="50,6 95,88 5,88" fill="url(#${gid})" ${out}/><path d="M50 6 L50 62 M5 88 L50 62 L95 88" ${line}/>`;
      ty = 70; fs = 26; break;
    case 6:
      body = `<rect x="8" y="8" width="84" height="84" rx="16" fill="url(#${gid})" ${out}/><rect x="18" y="18" width="64" height="64" rx="10" ${line}/>`;
      break;
    case 8:
      body = `<polygon points="50,4 95,50 50,96 5,50" fill="url(#${gid})" ${out}/><path d="M5 50 L95 50 M50 4 L28 50 L50 96 L72 50 Z" ${line}/>`;
      fs = 28; break;
    case 10: case 100:
      body = `<polygon points="50,4 95,42 50,96 5,42" fill="url(#${gid})" ${out}/><path d="M5 42 L50 58 L95 42 M50 58 L50 96" ${line}/>`;
      ty = 48; fs = faces === 100 ? 22 : 28; break;
    case 12: {
      const pt = (r, n, off) => Array.from({ length: n }, (_, i) => {
        const a = off + (i * 2 * Math.PI) / n;
        return `${(50 + r * Math.cos(a)).toFixed(1)},${(52 + r * Math.sin(a)).toFixed(1)}`;
      });
      const outer = pt(46, 10, -Math.PI / 2);
      const inner = pt(25, 5, -Math.PI / 2);
      body = `<polygon points="${outer.join(' ')}" fill="url(#${gid})" ${out}/><polygon points="${inner.join(' ')}" ${line}/>`
        + inner.map((p, i) => `<line x1="${p.split(',')[0]}" y1="${p.split(',')[1]}" x2="${outer[i * 2].split(',')[0]}" y2="${outer[i * 2].split(',')[1]}" ${line}/>`).join('');
      fs = 26; break;
    }
    default: { // d20 et autres
      const hex = Array.from({ length: 6 }, (_, i) => {
        const a = -Math.PI / 2 + (i * Math.PI) / 3;
        return [50 + 47 * Math.cos(a), 52 + 47 * Math.sin(a)];
      });
      const tri = [[50, 22], [77, 68], [23, 68]];
      body = `<polygon points="${hex.map((p) => p.map((v) => v.toFixed(1)).join(',')).join(' ')}" fill="url(#${gid})" ${out}/>`
        + `<polygon points="${tri.map((p) => p.join(',')).join(' ')}" ${line}/>`
        + [[0, 0], [1, 1], [1, 2], [2, 3], [2, 4], [0, 5]].map(([t, h]) => `<line x1="${tri[t][0]}" y1="${tri[t][1]}" x2="${hex[h][0].toFixed(1)}" y2="${hex[h][1].toFixed(1)}" ${line}/>`).join('');
      ty = 60; fs = 24;
    }
  }
  return `<svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
    <defs><linearGradient id="${gid}" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${light}"/><stop offset=".55" stop-color="${c}"/><stop offset="1" stop-color="${dark}"/></linearGradient></defs>
    ${body}
    <text x="50" y="${ty}" text-anchor="middle" dominant-baseline="middle" font-family="Cinzel, Georgia, serif" font-weight="800"
      font-size="${fs}" fill="#fff" stroke="${dark}" stroke-width="3" paint-order="stroke" class="die-num">${value}</text>
  </svg>`;
}

// ---------------------------------------------------------------- son (petit cliquetis synthétisé)
let audioCtx = null;
function diceSound() {
  try {
    audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
    const now = audioCtx.currentTime;
    const hits = [0, 0.12, 0.22, 0.38, 0.55, 0.78, 1.0, 1.18];
    for (const [i, t] of hits.entries()) {
      const len = 0.035;
      const buf = audioCtx.createBuffer(1, Math.floor(audioCtx.sampleRate * len), audioCtx.sampleRate);
      const d = buf.getChannelData(0);
      for (let j = 0; j < d.length; j++) d[j] = (Math.random() * 2 - 1) * Math.pow(1 - j / d.length, 3);
      const src = audioCtx.createBufferSource();
      src.buffer = buf;
      const f = audioCtx.createBiquadFilter();
      f.type = 'bandpass';
      f.frequency.value = 1800 + Math.random() * 1600;
      f.Q.value = 2.5;
      const g = audioCtx.createGain();
      g.gain.value = 0.55 * (1 - i / hits.length) + 0.1;
      src.connect(f).connect(g).connect(audioCtx.destination);
      src.start(now + t);
    }
  } catch { /* pas de son disponible */ }
}

// ---------------------------------------------------------------- animation
let trayTimer = null;

/**
 * Fait rouler les dés d'un jet dans la zone `tray`.
 * entry : entrée de journal { roll, from, label, secret }
 */
function animateRoll(tray, entry, { sound = true } = {}) {
  const r = entry.roll;
  const color = (entry.from && entry.from.couleur) || '#b33';
  const dice = [];
  for (const p of r.parts) {
    if (p.type !== 'dice') continue;
    p.rolls.forEach((v, i) => dice.push({ faces: p.faces, value: v, kept: p.kept[i] }));
  }
  const shown = dice.slice(0, 12);
  clearTimeout(trayTimer);
  tray.replaceChildren();
  tray.classList.add('show');

  const W = tray.clientWidth || 600;
  const H = tray.clientHeight || 400;
  const size = Math.max(54, Math.min(84, W / (Math.min(shown.length, 6) + 1.5)));
  const perRow = Math.max(1, Math.floor((W * 0.8) / (size * 1.15)));
  const rows = Math.ceil(shown.length / perRow);

  shown.forEach((d, i) => {
    const row = Math.floor(i / perRow);
    const inRow = Math.min(perRow, shown.length - row * perRow);
    const col = i % perRow;
    const fx = W / 2 + (col - (inRow - 1) / 2) * size * 1.15 - size / 2;
    const fy = H / 2 - (rows * size * 1.15) / 2 + row * size * 1.15 - size * 0.2;
    const fromLeft = Math.random() < 0.5;
    const sx = fromLeft ? -size * 1.5 : W + size * 0.5;
    const sy = H * (0.55 + Math.random() * 0.4);
    const rot0 = (fromLeft ? 1 : -1) * (540 + Math.random() * 540);
    const restRot = Math.random() * 24 - 12;

    const die = document.createElement('div');
    die.className = 'die';
    die.style.width = die.style.height = size + 'px';
    die.innerHTML = dieShapeSVG(d.faces, color, '?');
    tray.append(die);
    const num = die.querySelector('.die-num');

    const anim = die.animate([
      { transform: `translate(${sx}px, ${sy}px) rotate(${rot0}deg) scale(.7)`, offset: 0 },
      { transform: `translate(${(sx + fx) / 2}px, ${fy - size * 1.2}px) rotate(${rot0 * 0.45}deg) scale(1.1)`, offset: 0.35 },
      { transform: `translate(${fx + (fromLeft ? 14 : -14)}px, ${fy}px) rotate(${rot0 * 0.12}deg) scale(1)`, offset: 0.62 },
      { transform: `translate(${fx + (fromLeft ? 6 : -6)}px, ${fy - size * 0.18}px) rotate(${restRot + 25}deg) scale(1.02)`, offset: 0.8 },
      { transform: `translate(${fx}px, ${fy}px) rotate(${restRot}deg) scale(1)`, offset: 1 },
    ], { duration: DICE_ANIM_MS - 100 + i * 25, easing: 'cubic-bezier(.25,.7,.35,1)', fill: 'forwards' });

    // chiffres qui défilent puis ralentissent
    const t0 = performance.now();
    (function tick() {
      const el = performance.now() - t0;
      if (el < DICE_ANIM_MS - 250) {
        num.textContent = 1 + Math.floor(Math.random() * d.faces);
        setTimeout(tick, 45 + (el / DICE_ANIM_MS) * 160);
      }
    })();
    anim.onfinish = () => {
      num.textContent = d.value;
      if (!d.kept) die.classList.add('dropped');
      if (d.faces === 20 && d.kept && d.value === 20) die.classList.add('nat20');
      if (d.faces === 20 && d.kept && d.value === 1) die.classList.add('nat1');
    };
  });

  if (sound) diceSound();

  // résultat final
  setTimeout(() => {
    const d20 = dice.filter((d) => d.faces === 20 && d.kept);
    const crit = d20.length === 1 && d20[0].value === 20 ? 'crit' : d20.length === 1 && d20[0].value === 1 ? 'fumble' : '';
    const box = document.createElement('div');
    box.className = 'tray-result ' + crit;
    const who = document.createElement('div');
    who.className = 'tray-who';
    who.style.color = color;
    who.textContent = (entry.from ? entry.from.name : '') + (entry.label ? ' — ' + entry.label : '') + (entry.secret ? ' 🔒' : '');
    const total = document.createElement('div');
    total.className = 'tray-total';
    total.textContent = r.total;
    const formula = document.createElement('div');
    formula.className = 'tray-formula';
    formula.textContent = r.formula + (dice.length > shown.length ? ` (+${dice.length - shown.length} dés)` : '')
      + (crit === 'crit' ? ' · Réussite critique !' : crit === 'fumble' ? ' · Échec critique…' : '');
    box.append(who, total, formula);
    tray.append(box);
  }, DICE_ANIM_MS);

  trayTimer = setTimeout(() => {
    tray.classList.remove('show');
    setTimeout(() => { if (!tray.classList.contains('show')) tray.replaceChildren(); }, 400);
  }, DICE_ANIM_MS + 2600);
}
