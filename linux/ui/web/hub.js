// grenHub : tout ce que la machine sait d'elle-même, et un diagnostic qui le
// vérifie au lieu de le croire. Le diagnostic part une fois la page affichée.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret } = window.grenos;
  const D = {};
  const AFFICHE = { "bon": "Bon", "a surveiller": "À surveiller", "en echec": "En échec" };
  const GENRE = { "bon": "ok", "a surveiller": "alerte", "en echec": "danger" };

  function pave(p) {
    return el("section.carte.verre.hb-pave",
      el("div.etiquette", p.titre),
      el("dl", p.lignes.map(([n, v]) => [el("dt", n), el("dd", v)])));
  }

  function lancer() {
    D.liste.replaceChildren();
    D.verdict.className = "hb-verdict";
    D.verdict.textContent = "Essais en cours — le processeur, la mémoire, le disque, le réseau, l'affichage, l'USB et les services.";
    D.relancer.disabled = true;
    api.appeler("hub_diagnostic").catch((e) => { D.verdict.textContent = String(e); D.relancer.disabled = false; });
  }

  api.ecouter("hub-ligne", (r) => {
    D.liste.append(el("div.hb-essai",
      el("span.hb-nom", r.nom),
      el(`span.badge.${GENRE[r.etat] || "neutre"}`, AFFICHE[r.etat] || r.etat),
      el("span.hb-detail", r.detail)));
  });
  api.ecouter("hub-fin", (f) => {
    D.verdict.textContent = f.phrase;
    D.verdict.className = `hb-verdict ${GENRE[f.etat] || ""}`;
    D.relancer.disabled = false;
  });

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.paves = el("div.hb-paves", el("p.note", "Lecture de la machine…"));
    D.verdict = el("p.hb-verdict");
    D.liste = el("div.hb-liste");
    D.relancer = el("button.bouton", { onclick: lancer, disabled: true }, svg("maj"), "Relancer le diagnostic");
    document.body.append(el("div.page-hub",
      el("header.tete", el("div.logo", svg("bouclier")),
        el("div", el("h1", "grenHub"), el("small.note", "Tout ce que cette machine sait d'elle-même, et un diagnostic qui le vérifie au lieu de le croire."))),
      D.paves,
      el("section.carte.verre.hb-diag", el("div.etiquette", "Diagnostic"), D.verdict, D.liste, D.relancer)));
    pret();
    try {
      const v = await api.appeler("hub_etat");
      if (v) D.paves.replaceChildren(...v.paves.map(pave));
    } catch (e) {
      D.paves.replaceChildren(el("p.note", `La machine n'a pas pu être lue : ${e}`));
    }
    lancer();
  })();
})();
