// Bienvenue : quelques gestes utiles, des astuces, et le nom de la personne
// en tête — c'est sa machine. Pas de leçon.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, ouvrirProgramme, toast } = window.grenos;

  (async function () {
    habiller(await api.appeler("apparence"));
    const v = await api.appeler("bienvenue_vue");
    document.body.append(aurore());
    const grille = el("div.bv-grille");
    for (const g of v.gestes) {
      const carte = el("button.bv-geste.carte.verre",
        el("span.bv-pastille", svg(g.icone)),
        el("span.bv-texte", el("strong", g.nom), el("small", g.note)),
        el("span.bv-fleche", svg("fleche")));
      carte.addEventListener("click", () => ouvrirProgramme(g.programme).catch((e) => toast(String(e))));
      grille.append(carte);
    }
    document.body.append(el("div.page-bienvenue.apparait",
      el("header.bv-tete",
        el("div.logo", svg("marque")),
        el("div", el("h1.titre", el("span.degrade", v.salut)),
          el("p.sous", `Cette machine est à toi : ${v.adresse}. Voici de quoi commencer.`))),
      grille,
      el("section.carte.verre.bv-astuces", el("div.etiquette", "Astuces"),
        el("ul", v.astuces.map((a) => el("li", a))))));
    pret();
  })();
})();
