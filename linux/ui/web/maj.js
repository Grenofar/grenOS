// Mises à jour : un état lisible d'un coup d'œil, un seul bouton qui fait la
// bonne chose, et le détail pour qui veut le lire.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, notesMaj } = window.grenos;
  let etat = null;
  let enCours = false;
  const D = {};

  function cadre() {
    document.body.append(aurore());
    D.hero = el("section.hero.carte.verre");
    D.avance = el("section.carte.verre.avance", { hidden: true });
    D.barre = el("i");
    D.phrase = el("div.phrase", "…");
    D.avance.append(el("div.etiquette", "En cours"), D.phrase, el("div.progression", D.barre));
    D.journal = el("pre.journal");
    D.modes = el("div");
    D.paquets = el("section.carte.verre.paquets");
    D.nouveautes = el("section.carte.verre.nouveautes", { hidden: true });
    D.bandeau = el("div");
    document.body.append(el("div.page-maj",
      el("header.tete", el("div.logo", svg("maj")), el("div", el("h1", "Mises à jour"), el("small.note", "grenOS, Debian et tes applications Flatpak"))),
      D.bandeau, D.hero, D.nouveautes, D.avance,
      el("section.carte.verre", el("div.etiquette", "Mode"), D.modes),
      D.paquets,
      el("details.carte.verre", el("summary", "Voir le détail technique"), D.journal)));
  }

  function bandeau() {
    const r = etat.dernier_resultat;
    if (r.startsWith("echec")) {
      D.bandeau.replaceChildren(el("div.bandeau.danger", svg("bouclier"), "La dernière installation au démarrage a échoué. Le système a démarré normalement ; tu peux réessayer."));
    } else if (r.startsWith("ok")) {
      const n = r.split(/\s+/)[1] || "0";
      D.bandeau.replaceChildren(el("div.bandeau.ok", svg("coche"), `${n} paquet(s) installé(s) au dernier démarrage.`));
    } else D.bandeau.replaceChildren();
  }

  function hero() {
    let icone = "coche", genre = "ok", titre = "grenOS est à jour", detail = "", actions = [];
    const n = etat.disponibles;
    const fp = etat.flatpak;
    if (etat.arme) {
      icone = "redemarrer"; genre = "alerte";
      titre = "Prêt à installer";
      detail = "Les mises à jour sont téléchargées. Elles s'installent au redémarrage, avant le bureau — sans écran noir et sans risque pour ta session.";
      actions.push(el("button.bouton.principal.grand", { onclick: redemarrer }, svg("redemarrer"), "Redémarrer et installer"));
    } else if (n == null) {
      icone = "wifi"; genre = "neutre";
      titre = "État inconnu";
      detail = "La dernière vérification n'a pas pu joindre les dépôts. « Rien » n'est pas « zéro » : vérifie quand tu es en ligne.";
      actions.push(el("button.bouton.principal.grand", { onclick: preparer }, svg("maj"), "Chercher et préparer"));
    } else if (n > 0 || (fp || 0) > 0) {
      icone = "paquet"; genre = "accent";
      titre = n > 0 ? `${n} mise${n > 1 ? "s" : ""} à jour disponible${n > 1 ? "s" : ""}` : `${fp} application${fp > 1 ? "s" : ""} à mettre à jour`;
      detail = (etat.telechargee ? "Déjà téléchargées en arrière-plan. " : "") + "Prépare-les maintenant : les applications se mettent à jour tout de suite, le système au prochain redémarrage.";
      actions.push(el("button.bouton.principal.grand", { onclick: preparer }, svg("fleche"), "Préparer les mises à jour"));
    } else {
      detail = "Rien à installer. Les correctifs de sécurité arrivent ici dès qu'ils sont publiés.";
    }
    actions.push(el("button.bouton", { onclick: verifier }, svg("maj"), "Vérifier maintenant"));
    D.hero.className = `hero carte verre ${genre}`;
    D.hero.replaceChildren(
      el("div.halo", svg(icone)),
      el("div.corps",
        el("h2", titre),
        el("p.sous", detail),
        el("div.meta",
          el("span.badge", svg("sablier"), etat.date ? `Vérifié le ${etat.date} UTC` : "Jamais vérifié depuis le démarrage"),
          fp != null ? el("span.badge", svg("paquet"), `${fp} application(s) Flatpak`) : null),
        el("div.actions", ...actions)));
    D.hero.querySelectorAll("button").forEach((b) => (b.disabled = enCours));
  }

  function modes() {
    const options = [
      ["automatiques", "eclair", "Automatiques", "Téléchargées en fond, installées au redémarrage. Applications à jour toutes seules."],
      ["demander", "oeil", "Me demander", "Téléchargées et annoncées ; rien ne s'installe sans toi."],
      ["manuelles", "reglages", "Manuelles", "Compter, rien de plus."],
    ];
    const g = el("div.choix.trois", { role: "radiogroup" });
    for (const [valeur, i, titre, note] of options) {
      const t = el("button.tuile", { role: "radio", type: "button", "aria-checked": String(etat.mode === valeur),
        onclick: async () => {
          const avant = etat.mode;
          etat.mode = valeur; modes();
          try { await api.appeler("maj_mode", { mode: valeur }); }
          catch (e) { etat.mode = avant; modes(); toast(String(e)); }
        } }, el("span.pastille", svg(i)), el("span", el("b", titre), el("small", note)), el("span.coche", svg("coche")));
      g.append(t);
    }
    D.modes.replaceChildren(g);
  }

  // Ce qu'apporte la mise à jour qui attend, en phrases ; sans mise à jour,
  // ce que la version installée a apporté. Jamais de carte vide.
  function nouveautes() {
    const attend = (etat.disponibles || 0) > 0 || etat.arme;
    const venir = attend ? (etat.nouveautes_a_venir || []) : [];
    const liste = venir.length ? venir : (etat.nouveautes || []);
    D.nouveautes.hidden = !liste.length;
    D.nouveautes.replaceChildren(
      el("div.etiquette", venir.length ? "Ce qu'apporte la mise à jour" : "Nouveautés de ta version"),
      notesMaj(liste));
  }

  function paquets() {
    const p = etat.paquets;
    D.paquets.hidden = !p.length;
    D.paquets.replaceChildren(el("div.etiquette", `Ce qui va changer — ${p.length} paquet(s)`),
      el("div.puces", ...p.slice(0, 120).map((n) => el("span.puce-paquet.mono", n)),
        p.length > 120 ? el("span.note", ` et ${p.length - 120} de plus`) : null));
  }

  function toast(texte) {
    const t = el("div.toast", texte);
    document.body.append(t);
    setTimeout(() => t.remove(), 4200);
  }

  async function rafraichir() {
    etat = await api.appeler("maj_etat");
    enCours = etat.en_cours;
    bandeau(); hero(); nouveautes(); modes(); paquets();
  }

  async function verifier() {
    toast("Vérification en cours…");
    try { await api.appeler("maj_verifier"); } catch (e) { toast(String(e)); }
    rafraichir();
  }

  async function preparer() {
    enCours = true;
    D.journal.textContent = "";
    D.avance.hidden = false;
    D.barre.style.width = "2%";
    D.phrase.textContent = "Démarrage…";
    hero();
    try { await api.appeler("maj_preparer"); }
    catch (e) { enCours = false; D.avance.hidden = true; toast(String(e)); hero(); }
  }

  async function redemarrer() {
    try { await api.appeler("maj_redemarrer"); } catch (e) { toast(String(e)); }
  }

  api.ecouter("maj-avance", (a) => { D.barre.style.width = `${Math.max(2, Math.min(100, a.part))}%`; D.phrase.textContent = a.texte; });
  api.ecouter("maj-ligne", (l) => { D.journal.textContent += l + "\n"; D.journal.scrollTop = D.journal.scrollHeight; });
  api.ecouter("maj-fin", (f) => {
    enCours = false;
    D.barre.style.width = "100%";
    D.phrase.textContent = f.texte;
    setTimeout(() => { D.avance.hidden = true; }, f.bien ? 1600 : 6000);
    if (!f.bien) { D.journal.parentElement.open = true; }
    toast(f.texte);
    rafraichir();
  });

  (async function () {
    habiller(await api.appeler("apparence"));
    cadre();
    window.grenos.pret();
    rafraichir();
  })();
})();
