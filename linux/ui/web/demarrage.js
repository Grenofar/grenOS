// Une seule page HTML pour toutes les fenêtres : Rust dit laquelle par
// `window.GRENOS_PAGE` ; dans un navigateur, c'est l'ancre (#maj).
"use strict";
(function () {
  const PAGES = ["premier", "maj", "perf", "lance", "arret", "taches", "connexions", "secours", "bienvenue"];
  let page = window.GRENOS_PAGE || location.hash.slice(1) || "premier";
  if (!PAGES.includes(page)) page = "premier";
  document.body.dataset.page = page;
  const s = document.createElement("script");
  s.src = page + ".js";
  document.body.appendChild(s);
})();
