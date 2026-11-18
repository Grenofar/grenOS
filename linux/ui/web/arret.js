// Fin de session : quatre ou cinq gestes, aucun déclenché par erreur.
//
// Échap ou « Annuler » ferment. Éteindre est le seul bouton rouge. La page ne
// connaît que des CLÉS ; c'est Rust (arret.rs) qui sait ce qu'elles lancent,
// et qui refuse un geste qu'il n'a pas proposé.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, toast } = window.grenos;

  async function faire(geste, bouton) {
    for (const b of document.querySelectorAll(".geste")) b.disabled = true;
    bouton.classList.add("choisi");
    try {
      await api.appeler("arret_faire", { cle: geste.cle });
    } catch (e) {
      for (const b of document.querySelectorAll(".geste")) b.disabled = false;
      bouton.classList.remove("choisi");
      toast(String(e));
    }
  }

  function fermer() { api.appeler("fermer"); }

  (async function () {
    habiller(await api.appeler("apparence"));
    const vue = await api.appeler("arret_vue");
    document.body.append(aurore());
    const liste = el("div.gestes");
    for (const g of vue.gestes) {
      const b = el(`button.geste${g.genre ? "." + g.genre : ""}`,
        el("span.pastille", svg(g.icone)),
        el("span.texte", el("strong", g.titre), el("small", g.note)));
      b.addEventListener("click", () => faire(g, b));
      liste.append(b);
    }
    document.body.append(el("div.page-arret",
      el("header.tete", el("div.logo", svg("eteindre")),
        el("div", el("h1", "Fin de session"), el("small.note", vue.adresse))),
      liste,
      el("button.bouton.discret.annuler", { onclick: fermer }, "Annuler")));
    // Le focus sur le premier geste NON dangereux : Entrée par réflexe ne
    // doit jamais éteindre la machine.
    liste.querySelector(".geste:not(.danger)")?.focus();
    window.grenos.pret();
  })();

  document.addEventListener("keydown", (e) => { if (e.key === "Escape") fermer(); });
})();
