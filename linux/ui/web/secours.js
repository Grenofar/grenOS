// Mode sans échec : dire ce qui s'est passé, et n'allumer que ce qu'on peut
// tenir. Une action impossible reste visible, éteinte, avec sa raison.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret } = window.grenos;

  (async function () {
    habiller(await api.appeler("apparence"));
    const v = await api.appeler("secours_vue");
    document.body.append(aurore());
    const etat = el("p.sc-etat");
    const tete = v.echecs > 0
      ? [el("h1", "Le dernier démarrage n'a pas abouti"), el("p.sous", "grenOS n'a pas ouvert son bureau.")]
      : [el("h1", "Tout va bien"), el("p.sous", "Le dernier démarrage a ouvert son bureau.")];
    const resume = v.echecs > 0
      ? el("section.carte.verre.sc-compte", el("strong", String(v.echecs)),
          el("span", v.echecs > 1 ? "démarrage(s) d'affilée sans bureau" : "démarrage sans bureau"))
      : el("section.carte.verre", el("p.sc-rassurant", "Cette fenêtre s'ouvre d'elle-même après un démarrage raté. Tu peux aussi l'ouvrir quand tu veux."));
    const actions = el("div.sc-actions");
    for (const p of v.propositions) {
      const b = el("button.bouton", { disabled: !p.possible }, p.intitule);
      b.addEventListener("click", async () => {
        b.disabled = true;
        try { etat.textContent = (await api.appeler("secours_faire", { cle: p.cle })) || ""; }
        catch (e) { etat.textContent = String(e); }
        b.disabled = false;
      });
      actions.append(el("div.sc-ligne", b, p.pourquoi ? el("small", p.pourquoi) : null));
    }
    document.body.append(el("div.page-secours",
      el("header.tete", el("div.logo", svg("bouclier")), el("div", ...tete)),
      resume,
      v.cause ? el("section.carte.verre.sc-cause", el("div.etiquette", "Cause probable"), el("p", v.cause)) : null,
      actions, etat));
    pret();
  })();
})();
