'use strict';
/* Diaporamas : édition (MJ), génération par l'IA, projection sur l'écran de tous. */

const SLIDE_THEMES = [['parchemin', 'Parchemin'], ['sombre', 'Sombre'], ['sang', 'Sang'], ['mystique', 'Mystique'],
  ['nature', 'Nature'], ['glace', 'Glace'], ['epique', 'Épique']];

/** Rendu d'une slide (utilisé pour la projection, l'aperçu et les vignettes). */
function slideElement(s) {
  const node = el('div', { class: `slide theme-${s.style || 'parchemin'}${s.image ? ' has-img' : ''}` });
  if (s.image) node.style.backgroundImage = `url("${s.image}")`;
  node.append(el('div', { class: 'slide-inner' },
    el('div', { class: 'slide-ornament', text: '❦' }),
    el('h2', { class: 'slide-title', text: s.titre || '' }),
    s.texte ? el('p', { class: 'slide-text', text: s.texte }) : null));
  return node;
}

// ------------------------------------------------------------------ édition (MJ)
let deckId = null;
let slideId = null;

const curDeck = () => (S && S.slides ? S.slides.decks.find((d) => d.id === deckId) : null);
const curSlide = () => { const d = curDeck(); return d ? d.slides.find((s) => s.id === slideId) : null; };

$('#slide-styles').append(...SLIDE_THEMES.map(([k, nom]) => el('button', {
  type: 'button', class: 'chip', 'data-style': k, onclick: () => patchSlide({ style: k }, true),
}, nom)));

let slidePending = null;
let slideTimer = null;
function patchSlide(patch, now = false) {
  const s = curSlide();
  if (!s) return;
  Object.assign(s, patch); // affichage immédiat
  if (!slidePending || slidePending.slideId !== s.id) { flushSlide(); slidePending = { deckId, slideId: s.id, patch: {} }; }
  Object.assign(slidePending.patch, patch);
  clearTimeout(slideTimer);
  if (now) flushSlide(); else slideTimer = setTimeout(flushSlide, 400);
  renderSlideList();
  renderSlidePreview();
}
function flushSlide() {
  clearTimeout(slideTimer);
  if (slidePending) socket.emit('slides:slide:update', slidePending);
  slidePending = null;
}

$('#deck-select').addEventListener('change', (ev) => { flushSlide(); deckId = ev.target.value; slideId = null; renderSlides(); });
$('#deck-new').addEventListener('click', () => {
  const titre = prompt('Titre du diaporama :', 'Séance ' + (((S.slides && S.slides.decks.length) || 0) + 1));
  if (titre === null) return;
  socket.emit('slides:deck:create', { titre }, (res) => { if (res && res.ok) { deckId = res.id; slideId = null; renderSlides(); } });
});
$('#deck-delete').addEventListener('click', () => {
  const d = curDeck();
  if (d && confirm(`Supprimer le diaporama « ${d.titre} » et ses ${d.slides.length} slides ?`)) socket.emit('slides:deck:delete', { id: d.id });
});
$('#deck-titre').addEventListener('change', (ev) => socket.emit('slides:deck:update', { id: deckId, titre: ev.target.value }));

$('#slide-add').addEventListener('click', () => {
  flushSlide();
  socket.emit('slides:slide:add', { deckId, after: slideId }, (res) => { if (res && res.ok) { slideId = res.id; renderSlides(); $('#slide-titre').select(); } });
});
$('#slide-titre').addEventListener('input', (ev) => patchSlide({ titre: ev.target.value }));
$('#slide-texte').addEventListener('input', (ev) => patchSlide({ texte: ev.target.value }));
$('#slide-noimg').addEventListener('click', () => patchSlide({ image: null }, true));
$('#slide-scene').addEventListener('change', (ev) => { if (ev.target.value) patchSlide({ image: ev.target.value }, true); ev.target.value = ''; });
$('#slide-file').addEventListener('change', (ev) => {
  const file = ev.target.files[0];
  ev.target.value = '';
  if (!file) return;
  if (file.size > 12 * 1024 * 1024) return toast('Image trop lourde (12 Mo maximum).');
  const reader = new FileReader();
  reader.onload = () => socket.emit('slides:slide:image', { deckId, slideId, data: reader.result }, (res) => {
    if (res && !res.ok) toast(res.error);
  });
  reader.readAsDataURL(file);
});
$('#slide-up').addEventListener('click', () => { flushSlide(); socket.emit('slides:slide:move', { deckId, slideId, dir: -1 }); });
$('#slide-down').addEventListener('click', () => { flushSlide(); socket.emit('slides:slide:move', { deckId, slideId, dir: 1 }); });
$('#slide-delete').addEventListener('click', () => {
  const s = curSlide();
  if (s && confirm(`Supprimer la slide « ${s.titre || 'sans titre'} » ?`)) { flushSlide(); socket.emit('slides:slide:delete', { deckId, slideId }); }
});
$('#slide-show').addEventListener('click', () => {
  const d = curDeck();
  flushSlide();
  if (d) socket.emit('slides:present', { deckId, index: d.slides.findIndex((s) => s.id === slideId) });
});
$('#deck-present').addEventListener('click', () => {
  const d = curDeck();
  if (!d || !d.slides.length) return toast('Ajoute ou génère d\'abord des slides.');
  flushSlide();
  socket.emit('slides:present', { deckId, index: 0 });
});

// ---- génération par l'IA
$('#ia-form').addEventListener('submit', (ev) => {
  ev.preventDefault();
  const promptText = $('#ia-prompt').value.trim();
  if (!promptText) return toast('Décris ce que les slides doivent raconter.');
  const btn = $('#ia-go');
  btn.disabled = true;
  btn.textContent = '⏳ Génération en cours…';
  $('#ia-status').textContent = 'Claude rédige tes slides, cela prend généralement 10 à 40 secondes.';
  socket.emit('slides:generate', {
    deckId, prompt: promptText, nombre: $('#ia-nombre').value, ton: $('#ia-ton').value, inclureGroupe: $('#ia-groupe').checked,
  }, (res) => {
    btn.disabled = false;
    btn.textContent = '✨ Générer';
    if (!res || !res.ok) { $('#ia-status').textContent = '⚠ ' + (res ? res.error : 'Erreur'); return; }
    $('#ia-status').textContent = `✔ ${res.ajoutees} slide(s) ajoutée(s) à la fin du diaporama. Tu peux les retoucher ci-dessous.`;
    slideId = res.premier;
    renderSlides();
  });
});

function renderSlideList() {
  const d = curDeck();
  if (!d) return;
  $('#slide-list').replaceChildren(...d.slides.map((s, i) => el('li', {
    class: 'slide-item' + (s.id === slideId ? ' active' : '') + (S.projection && S.projection.deckId === d.id && S.projection.index === i ? ' live' : ''),
    onclick: () => { flushSlide(); slideId = s.id; renderSlides(); },
  },
  el('span', { class: 'slide-num', text: i + 1 }),
  el('span', { class: `slide-dot theme-${s.style}` }),
  el('span', { class: 'grow', text: s.titre || 'Sans titre' }),
  s.image ? el('span', { class: 'muted tiny', text: '🖼' }) : null)));
}
function renderSlidePreview() {
  const s = curSlide();
  $('#slide-preview').replaceChildren(s ? slideElement(s) : '');
  $$('#slide-styles [data-style]').forEach((b) => b.classList.toggle('active', !!s && b.dataset.style === s.style));
}

function renderSlides() {
  if (!S || me.role !== 'mj' || !S.slides) return;
  const decks = S.slides.decks;
  if (!decks.some((d) => d.id === deckId)) deckId = decks.length ? decks[decks.length - 1].id : null;
  const sel = $('#deck-select');
  if (document.activeElement !== sel) {
    sel.replaceChildren(...decks.map((d) => el('option', { value: d.id, text: `${d.titre} (${d.slides.length})` })));
    sel.value = deckId || '';
  }
  sel.hidden = !decks.length;
  $('#deck-delete').hidden = !deckId;
  const d = curDeck();
  $('#deck-empty').hidden = !!d;
  $('#deck-editor').hidden = !d;
  $('#ia-off').hidden = S.slides.ia;
  $('#ia-form').hidden = !S.slides.ia;
  if (!d) return;
  if (document.activeElement !== $('#deck-titre')) $('#deck-titre').value = d.titre;
  if (!d.slides.some((s) => s.id === slideId)) slideId = d.slides.length ? d.slides[0].id : null;
  renderSlideList();

  const s = curSlide();
  $('#slide-editor').hidden = !s;
  if (!s) return;
  // ne pas écraser ce que le MJ est en train de taper
  if (!(slidePending && slidePending.slideId === s.id)) {
    if (document.activeElement !== $('#slide-titre')) $('#slide-titre').value = s.titre;
    if (document.activeElement !== $('#slide-texte')) $('#slide-texte').value = s.texte;
  }
  const scenes = $('#slide-scene');
  scenes.replaceChildren(el('option', { value: '', text: '— ou une carte —' }), ...S.scenes.filter((sc) => sc.image).map((sc) => el('option', { value: sc.image, text: sc.titre })));
  renderSlidePreview();
}

// ------------------------------------------------------------------ projection (tout le monde)
let shownKey = null;  // slide actuellement affichée
let hiddenKey = null; // slide que ce joueur a choisi de réduire

function renderProjection() {
  const p = S && S.projection;
  const overlay = $('#projection');
  if (!p) {
    overlay.hidden = true;
    $('#projection-pill').hidden = true;
    shownKey = null;
    hiddenKey = null;
    return;
  }
  const key = `${p.deckId}:${p.index}:${p.id}`;
  const content = JSON.stringify([p.titre, p.texte, p.style, p.image]);
  if (key !== shownKey || overlay.dataset.content !== content) {
    const fresh = key !== shownKey;
    shownKey = key;
    overlay.dataset.content = content;
    const node = slideElement(p);
    if (fresh) node.classList.add('enter');
    $('#projection-slide').replaceChildren(node);
  }
  $('#projection-count').textContent = `${p.deckTitre} · ${p.index + 1} / ${p.total}`;
  $('#proj-prev').disabled = p.index <= 0;
  $('#proj-next').disabled = p.index >= p.total - 1;
  const hidden = hiddenKey === key;
  overlay.hidden = hidden;
  $('#projection-pill').hidden = !hidden;
}

$('#proj-prev').addEventListener('click', () => socket.emit('slides:step', { dir: -1 }));
$('#proj-next').addEventListener('click', () => socket.emit('slides:step', { dir: 1 }));
$('#proj-stop').addEventListener('click', () => socket.emit('slides:stop'));
$('#proj-hide').addEventListener('click', () => { hiddenKey = shownKey; renderProjection(); });
$('#projection-pill').addEventListener('click', () => { hiddenKey = null; renderProjection(); });
document.addEventListener('keydown', (ev) => {
  if ($('#projection').hidden || /INPUT|TEXTAREA|SELECT/.test(document.activeElement.tagName)) return;
  if (ev.key === 'Escape') { hiddenKey = shownKey; renderProjection(); return; }
  if (me.role !== 'mj') return;
  if (ev.key === 'ArrowRight' || ev.key === ' ' || ev.key === 'PageDown') { ev.preventDefault(); socket.emit('slides:step', { dir: 1 }); }
  if (ev.key === 'ArrowLeft' || ev.key === 'PageUp') { ev.preventDefault(); socket.emit('slides:step', { dir: -1 }); }
});
