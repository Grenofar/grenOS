// Ce que toutes les pages partagent : parler à Rust, dessiner, s'habiller.
"use strict";

const TAURI = window.__TAURI__;
const EN_APERCU = !TAURI; // ouvert dans un navigateur, pour regarder le dessin

const api = {
  async appeler(commande, args) {
    if (EN_APERCU) return window.APERCU ? window.APERCU(commande, args) : null;
    return TAURI.core.invoke(commande, args || {});
  },
  ecouter(evenement, rappel) {
    if (EN_APERCU) {
      (window.APERCU_ECOUTE = window.APERCU_ECOUTE || {})[evenement] = rappel;
      return;
    }
    TAURI.event.listen(evenement, (e) => rappel(e.payload));
  },
};

// Des icônes dessinées en traits : quelques centaines d'octets, aucune police
// d'icônes ni thème à charger (Papirus coûtait ~610 ms à chaque lancement).
const TRACES = {
  fleche: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  retour: '<path d="M19 12H5M11 18l-6-6 6-6"/>',
  coche: '<path d="M5 12.5l4.5 4.5L19 7"/>',
  langue: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 010 18M12 3a14 14 0 000 18"/>',
  clavier: '<rect x="2.5" y="6" width="19" height="12" rx="2.5"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>',
  compte: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0116 0"/>',
  eclair: '<path d="M13 2L4 14h7l-1 8 9-12h-7z"/>',
  reglages: '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
  code: '<path d="M8 7l-5 5 5 5M16 7l5 5-5 5M14 4l-4 16"/>',
  wifi: '<path d="M2 8.5a15 15 0 0120 0M5 12a10 10 0 0114 0M8.5 15.5a5 5 0 017 0"/><circle cx="12" cy="19" r="1"/>',
  cadenas: '<rect x="4.5" y="10.5" width="15" height="10" rx="2.5"/><path d="M8 10.5V7a4 4 0 018 0v3.5"/>',
  palette: '<path d="M12 3a9 9 0 100 18c1.1 0 1.5-.8 1.5-1.6 0-1.3-1-1.6-1-2.9 0-.9.7-1.5 1.6-1.5H16a5 5 0 005-5c0-3.9-4-7-9-7z"/><circle cx="7.5" cy="11" r="1"/><circle cx="10" cy="7.3" r="1"/><circle cx="14.5" cy="7.3" r="1"/>',
  bureau: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
  manette: '<path d="M6 9h4M8 7v4M15 9h.01M17.5 11h.01"/><path d="M7 5h10a5 5 0 015 5v3a3 3 0 01-5.2 2L15 13H9l-1.8 2A3 3 0 012 13v-3a5 5 0 015-5z"/>',
  film: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4"/>',
  oeil: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
  bouclier: '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/><path d="M9 12l2 2 4-4"/>',
  maj: '<path d="M21 12a9 9 0 11-3-6.7L21 8M21 3v5h-5"/>',
  paquet: '<path d="M21 8l-9-5-9 5 9 5 9-5zM3 8v8l9 5 9-5V8M12 13v8"/>',
  cpu: '<rect x="6" y="6" width="12" height="12" rx="2"/><rect x="9.5" y="9.5" width="5" height="5"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>',
  ram: '<rect x="2" y="7" width="20" height="10" rx="1.5"/><path d="M6 11v2M10 11v2M14 11v2M18 11v2M5 17v2M19 17v2"/>',
  gpu: '<rect x="2" y="6" width="20" height="12" rx="2"/><circle cx="9" cy="12" r="3"/><path d="M15 10h4M15 14h4"/>',
  disque: '<ellipse cx="12" cy="6" rx="8" ry="3"/><path d="M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/>',
  bt: '<path d="M7 7l10 10-5 5V2l5 5L7 17"/>',
  son: '<path d="M11 5L6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 010 7M18.5 5.5a9 9 0 010 13"/>',
  fusee: '<path d="M5 15c-1.5 1.5-2 5-2 5s3.5-.5 5-2M14 4c3 0 6 3 6 6l-7 7-6-6 7-7zM15 9h.01"/><path d="M8 11L4 11l3-4h4M13 16v4l4-3v-4"/>',
  sablier: '<path d="M6 2h12M6 22h12M7 2c0 5 5 6 5 10s-5 5-5 10M17 2c0 5-5 6-5 10s5 5 5 10"/>',
  etoile: '<path d="M12 2l2.9 6.6 7.1.6-5.4 4.7 1.6 7L12 17.3 5.8 21l1.6-7L2 9.3l7.1-.6z"/>',
  redemarrer: '<path d="M3 12a9 9 0 109-9 9.7 9.7 0 00-6.7 2.8L3 8M3 3v5h5"/>',
  terminal: '<rect x="2.5" y="4" width="19" height="16" rx="2.5"/><path d="M7 9l3 3-3 3M13 15h4"/>',
  nuit: '<path d="M20 14.5A8 8 0 019.5 4 8 8 0 1020 14.5z"/>',
  jour: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  auto: '<circle cx="12" cy="12" r="9"/><path d="M12 3v18" /><path d="M12 3a9 9 0 010 18z" fill="currentColor" stroke="none"/>',
};

function icone(nom) {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${TRACES[nom] || ""}</svg>`;
}

// el("div.carte", {onclick}, enfant, "texte", …)
function el(selecteur, attributs, ...enfants) {
  const [balise, ...classes] = selecteur.split(".");
  const n = document.createElement(balise || "div");
  if (classes.length) n.className = classes.join(" ");
  if (attributs && (typeof attributs !== "object" || attributs instanceof Node || Array.isArray(attributs))) {
    enfants.unshift(attributs);
    attributs = null;
  }
  for (const [cle, valeur] of Object.entries(attributs || {})) {
    if (cle.startsWith("on")) n.addEventListener(cle.slice(2), valeur);
    else if (cle === "html") n.innerHTML = valeur;
    else if (valeur !== false && valeur != null) n.setAttribute(cle, valeur === true ? "" : valeur);
  }
  for (const e of enfants.flat()) {
    if (e == null || e === false) continue;
    n.append(e instanceof Node ? e : document.createTextNode(String(e)));
  }
  return n;
}

function svg(nom) {
  const s = document.createElement("span");
  s.style.display = "contents";
  s.innerHTML = icone(nom);
  return s;
}

// « auto » = sombre le soir et la nuit, clair le jour. Décidé à l'ouverture.
function themeReel(theme) {
  if (theme === "jour" || theme === "nuit") return theme;
  const h = new Date().getHours();
  return h >= 7 && h < 19 ? "jour" : "nuit";
}

function habiller({ theme = "auto", accent = "bleu", effets = "equilibres" } = {}) {
  const r = document.documentElement;
  r.dataset.theme = themeReel(theme);
  r.dataset.accent = accent;
  r.dataset.effets = effets;
}

// Le fond vivant. En « performance », deux taches immobiles et rien d'autre.
function aurore() {
  return el("div.aurore", el("div.tache.t1"), el("div.tache.t2"), el("div.tache.t3"));
}

// La machine est-elle assez rapide pour les effets ? On le MESURE, sur une
// seconde d'images, au lieu de le deviner sur le nom de la carte graphique.
function mesurerFluidite() {
  return new Promise((resolve) => {
    let images = 0;
    const debut = performance.now();
    function tic() {
      images++;
      if (performance.now() - debut < 1000) requestAnimationFrame(tic);
      else resolve(images);
    }
    requestAnimationFrame(tic);
  });
}

window.grenos = { api, icone, el, svg, habiller, aurore, mesurerFluidite, themeReel, EN_APERCU };
