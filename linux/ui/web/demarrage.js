// Une seule page HTML pour toutes les fenêtres : Rust dit laquelle par
// `window.GRENOS_PAGE` ; dans un navigateur, c'est l'ancre (#maj).
"use strict";
(function () {
  const PAGES = ["premier", "maj", "perf", "lance", "arret", "recuperation", "grenai", "agenda", "horloge", "fichiers", "reglages", "jeux", "grenplace", "hub", "taches", "connexions", "secours", "bienvenue"];
  // Dans l'aperçu, « #reglages:ecran » ouvre aussi l'onglet, comme Rust le
  // fait avec GRENOS_ONGLET (« grenos-ui reglages:ecran »).
  const [ancre, onglet] = location.hash.slice(1).split(":");
  let page = window.GRENOS_PAGE || ancre || "premier";
  if (!PAGES.includes(page)) page = "premier";
  if (!window.GRENOS_PAGE && onglet) window.GRENOS_ONGLET = onglet;
  document.body.dataset.page = page;
  const s = document.createElement("script");
  s.src = page + ".js";
  document.body.appendChild(s);
})();
