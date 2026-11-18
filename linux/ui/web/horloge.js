// Horloge : l'heure ici et dans d'autres villes, un minuteur, un chronomètre.
//
// Le minuteur prévient par une notification (horloge.rs) : il tourne tant que
// la fenêtre est ouverte, même cachée — et la page le DIT, plutôt que de
// laisser croire qu'il survit à sa fermeture.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret } = window.grenos;
  const D = {};
  const VILLES = [["Paris", "Europe/Paris"], ["Montréal", "America/Montreal"], ["New York", "America/New_York"],
    ["Londres", "Europe/London"], ["Dubaï", "Asia/Dubai"], ["Tokyo", "Asia/Tokyo"], ["Sydney", "Australia/Sydney"], ["Los Angeles", "America/Los_Angeles"]];
  const z = (n) => String(n).padStart(2, "0");
  let onglet = "horloge";

  // ---- Horloge ------------------------------------------------------------------
  function horloge() {
    const t = new Date();
    D.heure.textContent = `${z(t.getHours())}:${z(t.getMinutes())}`;
    D.secondes.textContent = z(t.getSeconds());
    D.date.textContent = t.toLocaleDateString("fr-FR", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
    for (const [ville, fuseau, noeud] of D.villes) {
      noeud.textContent = t.toLocaleTimeString("fr-FR", { timeZone: fuseau, hour: "2-digit", minute: "2-digit" });
    }
  }

  // ---- Minuteur -------------------------------------------------------------------
  const M = { total: 0, reste: 0, fin: 0, tourne: false };
  function afficherMinuteur() {
    const s = Math.max(0, Math.ceil(M.reste / 1000));
    D.minuteur.textContent = `${z(Math.floor(s / 3600))}:${z(Math.floor(s / 60) % 60)}:${z(s % 60)}`;
    D.anneau.style.setProperty("--part", M.total ? `${(1 - M.reste / M.total) * 360}deg` : "0deg");
    D.lancer.textContent = M.tourne ? "Pause" : (M.reste > 0 && M.reste < M.total ? "Reprendre" : "Démarrer");
  }
  function regler(secondes) {
    if (M.tourne) return;
    M.total = M.reste = secondes * 1000;
    afficherMinuteur();
  }
  function basculer() {
    if (M.tourne) { M.tourne = false; M.reste = M.fin - Date.now(); afficherMinuteur(); return; }
    if (M.reste <= 0) {
      const s = Number(D.h.value) * 3600 + Number(D.m.value) * 60 + Number(D.s.value);
      if (!s) return;
      M.total = M.reste = s * 1000;
    }
    M.tourne = true;
    M.fin = Date.now() + M.reste;
    afficherMinuteur();
  }
  function remettre() { M.tourne = false; M.reste = M.total; afficherMinuteur(); }
  function tic() {
    if (M.tourne) {
      M.reste = M.fin - Date.now();
      if (M.reste <= 0) {
        M.tourne = false; M.reste = 0;
        const d = Math.round(M.total / 1000);
        api.appeler("horloge_sonner", { texte: `${d >= 60 ? Math.round(d / 60) + " min" : d + " s"} écoulées.` });
        D.cadreMinuteur.classList.add("fini");
        setTimeout(() => D.cadreMinuteur.classList.remove("fini"), 4000);
      }
      afficherMinuteur();
    }
    if (C.tourne) afficherChrono();
    if (onglet === "horloge") horloge();
  }

  // ---- Chronomètre -------------------------------------------------------------------
  const C = { depart: 0, cumul: 0, tourne: false, tours: [] };
  const ecoule = () => C.cumul + (C.tourne ? performance.now() - C.depart : 0);
  function format(ms) {
    const c = Math.floor(ms / 10) % 100, s = Math.floor(ms / 1000);
    return `${z(Math.floor(s / 60))}:${z(s % 60)},${z(c)}`;
  }
  function afficherChrono() {
    D.chrono.textContent = format(ecoule());
    D.chronoLancer.textContent = C.tourne ? "Pause" : (C.cumul ? "Reprendre" : "Démarrer");
    D.tour.textContent = C.tourne ? "Tour" : "Remettre à zéro";
  }
  function chronoBasculer() {
    if (C.tourne) { C.cumul = ecoule(); C.tourne = false; } else { C.depart = performance.now(); C.tourne = true; }
    afficherChrono();
  }
  function chronoTour() {
    if (C.tourne) {
      const t = ecoule();
      const avant = C.tours.length ? C.tours[0].total : 0;
      C.tours.unshift({ total: t, duree: t - avant });
    } else { C.cumul = 0; C.tours = []; }
    D.tours.replaceChildren(...C.tours.map((t, i) => el("li", el("span", `Tour ${C.tours.length - i}`), el("span.mono", format(t.duree)), el("span.mono.note", format(t.total)))));
    afficherChrono();
  }

  function choisir(cle) {
    onglet = cle;
    for (const p of document.querySelectorAll(".hg-page")) p.hidden = p.dataset.cle !== cle;
    for (const b of document.querySelectorAll(".hg-onglets button")) b.classList.toggle("actif", b.dataset.cle === cle);
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.heure = el("span.hg-heure");
    D.secondes = el("span.hg-secondes");
    D.date = el("p.hg-date");
    D.villes = VILLES.map(([v, f]) => [v, f, el("strong.mono")]);
    D.minuteur = el("div.hg-grand.mono", "00:00:00");
    D.anneau = el("div.hg-anneau", D.minuteur);
    D.cadreMinuteur = el("div.hg-cadre", D.anneau);
    D.lancer = el("button.bouton.principal", { onclick: basculer }, "Démarrer");
    const champ = (max) => el("input.champ.hg-champ", { type: "number", min: 0, max, value: 0 });
    D.h = champ(23); D.m = champ(59); D.s = champ(59);
    D.m.value = 5;
    D.chrono = el("div.hg-grand.mono", "00:00,00");
    D.chronoLancer = el("button.bouton.principal", { onclick: chronoBasculer }, "Démarrer");
    D.tour = el("button.bouton", { onclick: chronoTour }, "Remettre à zéro");
    D.tours = el("ol.hg-tours");
    const onglets = [["horloge", "Horloge"], ["minuteur", "Minuteur"], ["chrono", "Chronomètre"]];
    document.body.append(el("div.page-horloge",
      el("nav.cx-onglets.hg-onglets", onglets.map(([c, t]) => el(`button${c === "horloge" ? ".actif" : ""}`, { "data-cle": c, onclick: () => choisir(c) }, t))),
      el("section.hg-page", { "data-cle": "horloge" },
        el("div.hg-cadran", D.heure, D.secondes), D.date,
        el("div.hg-villes", D.villes.map(([v, , n]) => el("div.carte.verre", el("small.note", v), n)))),
      el("section.hg-page", { "data-cle": "minuteur", hidden: true },
        D.cadreMinuteur,
        el("div.hg-reglage", D.h, el("span", "h"), D.m, el("span", "min"), D.s, el("span", "s")),
        el("div.hg-rapides", [1, 3, 5, 10, 15, 25, 30, 60].map((m) => el("button.bouton.petit", { onclick: () => { D.h.value = Math.floor(m / 60); D.m.value = m % 60; D.s.value = 0; regler(m * 60); } }, `${m} min`))),
        el("div.hg-actions", D.lancer, el("button.bouton", { onclick: remettre }, "Remettre")),
        el("p.note.hg-note", "Le minuteur prévient par une notification, même si la fenêtre est cachée. Il s'arrête si tu fermes la fenêtre.")),
      el("section.hg-page", { "data-cle": "chrono", hidden: true },
        D.chrono, el("div.hg-actions", D.chronoLancer, D.tour), D.tours)));
    for (const c of [D.h, D.m, D.s]) c.addEventListener("input", () => { if (!M.tourne) { M.reste = 0; M.total = 0; afficherMinuteur(); } });
    horloge();
    afficherMinuteur();
    pret();
    setInterval(tic, 100);
  })();
})();
