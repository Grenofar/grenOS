// grenPerf : la mémoire, le processeur, et ce qui les mange — avec de quoi agir.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, mesurerFluidite } = window.grenos;
  const D = {};
  let dernier = null;
  // Ctrl+Alt+Échap ouvre directement le mode jeu (grenos-ui jeu).
  let onglet = window.GRENOS_ONGLET || "processus";
  let filtre = "";
  let systeme = false;
  const aFermer = new Set();
  let jeuVu = false;
  // Ce qui fait tourner le jeu lui-même : jamais proposé à la fermeture.
  const POUR_JOUER = /steam|proton|wine|heroic|lutris|gamemode|sober|bottles/i;
  const nombre = (v, d = 1) => v.toFixed(d).replace(".", ",");

  function cadre() {
    document.body.append(aurore());
    D.tuiles = el("section.tuiles");
    D.corps = el("section.carte.verre.corps-perf");
    D.onglets = el("nav.onglets", { role: "tablist" });
    D.jeu = el("button.bouton.mode-jeu", { onclick: () => basculerJeu() });
    document.body.append(el("div.page-perf",
      el("header.tete",
        el("div.logo", svg("eclair")),
        el("div", el("h1", "grenPerf"), el("small.note", "Mémoire, processeur et ce qui les mange")),
        el("div.espace"), D.jeu),
      D.tuiles, D.onglets, D.corps));
    for (const [cle, titre] of [["processus", "Processus"], ["demarrage", "Démarrage"], ["jeu", "Mode jeu"]]) {
      D.onglets.append(el("button.onglet", { role: "tab", "data-cle": cle, onclick: () => { onglet = cle; dessiner(); } }, titre));
    }
  }

  function tuile(titre, valeur, detail, part, genre = "") {
    return el(`div.carte.verre.tuile-chiffre${genre ? "." + genre : ""}`,
      el("div.etiquette", titre),
      el("div.chiffre", valeur),
      el("div.jauge", { role: "meter", "aria-valuenow": Math.round(part * 100), "aria-valuemin": 0, "aria-valuemax": 100, "aria-label": titre },
        el("i", { style: `width:${Math.max(1, Math.min(100, part * 100))}%` })),
      el("div.note", detail));
  }

  function tuiles(s) {
    const utilisee = s.memoire_totale_go - s.memoire_dispo_go;
    const partRam = utilisee / Math.max(0.01, s.memoire_totale_go);
    const echange = s.echange_total_go > 0 ? s.echange_utilise_go / s.echange_total_go : 0;
    const liberer = el("button.bouton.petit", { onclick: liberer_ }, svg("eclair"), "Libérer la RAM");
    D.tuiles.replaceChildren(
      tuile("Processeur", `${Math.round(s.cpu)} %`, `${s.coeurs} cœur(s) · charge ${s.charge}`, s.cpu / 100, s.cpu > 85 ? "chaud" : ""),
      el("div.tuile-ram",
        tuile("Mémoire", `${nombre(utilisee)} Go`, `sur ${nombre(s.memoire_totale_go)} Go · ${nombre(s.memoire_cache_go)} Go de cache rendu à la demande`, partRam, partRam > 0.88 ? "chaud" : ""),
        liberer),
      tuile("Échange", s.echange_total_go > 0 ? `${nombre(s.echange_utilise_go)} Go` : "aucun", s.echange_total_go > 0 ? `sur ${nombre(s.echange_total_go)} Go` : "pas de mémoire d'échange", echange),
      tuile("Applications", String(s.groupes.filter((g) => g.a_moi).length), `${s.groupes.reduce((n, g) => n + g.nombre, 0)} processus au total`, 0));
  }

  function ligneProcessus(g, maxCpu) {
    const fermer = el("button.bouton.petit.danger", { disabled: g.protege || !g.a_moi, title: g.protege ? "Fait tourner le bureau : protégé" : "" },
      g.protege ? svg("cadenas") : svg("retour"), g.protege ? "Protégé" : "Fermer");
    fermer.addEventListener("click", async () => {
      if (!fermer.dataset.confirme) { fermer.dataset.confirme = "1"; fermer.lastChild.textContent = "Confirmer ?"; setTimeout(() => { delete fermer.dataset.confirme; fermer.lastChild.textContent = "Fermer"; }, 3000); return; }
      fermer.disabled = true;
      try { const n = await api.appeler("perf_fermer", { nom: g.nom }); toast(`${g.description || g.nom} fermé (${n} processus).`); }
      catch (e) { toast(String(e)); }
      rafraichir();
    });
    const prio = el("select.choix-prio", { disabled: g.protege || !g.a_moi, "aria-label": "Priorité" },
      ...[["haute", "Priorité haute"], ["normale", "Normale"], ["basse", "Basse"]].map(([v, t]) =>
        el("option", { value: v, selected: (v === "haute" && g.priorite < 0) || (v === "basse" && g.priorite > 0) || (v === "normale" && g.priorite === 0) }, t)));
    prio.addEventListener("change", async () => {
      try { await api.appeler("perf_priorite", { nom: g.nom, niveau: prio.value }); toast("Priorité changée."); }
      catch (e) { toast(String(e)); }
    });
    return el("div.processus",
      el("div.qui",
        el("b", g.description || g.nom),
        el("small.note", `${g.nom}${g.nombre > 1 ? ` · ${g.nombre} processus` : ""}${g.a_moi ? "" : " · système"}`)),
      el("div.mesure", el("span.mono", `${nombre(g.cpu)} %`), el("div.mini", el("i", { style: `width:${Math.min(100, (g.cpu / Math.max(1, maxCpu)) * 100)}%` }))),
      el("div.mesure", el("span.mono", g.memoire_mo >= 1024 ? `${nombre(g.memoire_mo / 1024)} Go` : `${Math.round(g.memoire_mo)} Mo`)),
      prio, fermer);
  }

  function processus(s) {
    const recherche = el("input.champ.recherche", { type: "search", placeholder: "Chercher une application…", value: filtre,
      oninput: (e) => { filtre = e.target.value; liste(); } });
    const bascule = el("button.interrupteur", { role: "switch", "aria-checked": String(systeme), type: "button",
      onclick: () => { systeme = !systeme; bascule.setAttribute("aria-checked", String(systeme)); liste(); } });
    const conteneur = el("div.liste-processus");
    function liste() {
      const q = filtre.trim().toLowerCase();
      const g = s.groupes.filter((x) => (systeme || x.a_moi) && (!q || x.nom.toLowerCase().includes(q) || x.description.toLowerCase().includes(q)));
      const maxCpu = Math.max(...g.map((x) => x.cpu), 1);
      conteneur.replaceChildren(
        el("div.processus.entete-liste", el("span", "Application"), el("span", "Processeur"), el("span", "Mémoire"), el("span", "Priorité"), el("span", "")),
        ...g.slice(0, 80).map((x) => ligneProcessus(x, maxCpu)));
    }
    liste();
    return [el("div.barre-outils", recherche, el("label.ligne-option", el("span.note", "Afficher le système"), bascule)), conteneur];
  }

  async function demarrage() {
    const items = await api.appeler("perf_demarrage");
    const zone = el("div.lignes");
    for (const d of items) {
      const b = el("button.interrupteur", { role: "switch", type: "button", "aria-checked": String(d.actif),
        onclick: async () => {
          const voulu = b.getAttribute("aria-checked") !== "true";
          try { await api.appeler("perf_regler_demarrage", { id: d.id, actif: voulu }); b.setAttribute("aria-checked", String(voulu)); }
          catch (e) { toast(String(e)); }
        } });
      zone.append(el("div.ligne", el("span.icone", svg("fusee")), el("div", el("div.nom", d.nom), el("div.detail", d.commentaire || d.id)), el("span.valeur", b)));
    }
    if (!items.length) zone.append(el("p.note", "Rien ne démarre avec ta session en plus de grenOS."));
    return [el("p.sous", "Ce qui s'ouvre tout seul à chaque session. Couper ici ne désinstalle rien : l'application reste là quand tu l'ouvres toi-même."), zone];
  }

  function modeJeu(s) {
    const candidats = s.groupes.filter((g) => g.a_moi && !g.protege && !POUR_JOUER.test(g.nom) && (g.en_trop || g.memoire_mo > 150));
    if (!jeuVu) { candidats.filter((g) => g.en_trop).forEach((g) => aFermer.add(g.nom)); jeuVu = true; }
    const choix = el("div.choix.liste");
    for (const g of candidats) {
      const t = el("button.tuile.compacte", { role: "checkbox", type: "button", "aria-checked": String(aFermer.has(g.nom)),
        onclick: () => { aFermer.has(g.nom) ? aFermer.delete(g.nom) : aFermer.add(g.nom); t.setAttribute("aria-checked", String(aFermer.has(g.nom))); } },
        el("span.pastille", svg(g.en_trop ? "sablier" : "paquet")),
        el("span", el("b", g.description || g.nom), el("small", `${Math.round(g.memoire_mo)} Mo${g.en_trop ? " · inutile pendant une partie" : ""}`)),
        el("span.coche", svg("coche")));
      choix.append(t);
    }
    return [
      el("p.sous", "Pour une partie : ce qui est coché se ferme, GrenAI et son modèle local aussi, tout le reste de tes applications passe en arrière-plan (priorité basse). Steam, Proton et le jeu gardent la priorité. Couper le mode jeu rend à chacun sa priorité — rien n'est définitif."),
      candidats.length ? el("div.etiquette", "À fermer pendant la partie") : el("p.note", "Rien de lourd ni d'inutile ne tourne en ce moment."),
      choix,
      el("button.bouton.principal.grand", { onclick: () => basculerJeu() }, svg("manette"), s.mode_jeu ? "Couper le mode jeu" : "Activer le mode jeu")];
  }

  async function basculerJeu() {
    const actif = !(dernier && dernier.mode_jeu);
    try {
      const b = await api.appeler("perf_mode_jeu", { actif, fermer: actif ? [...aFermer] : [], garder: [] });
      toast(b.message);
    } catch (e) { toast(String(e)); }
    rafraichir(true);
  }

  async function liberer_() {
    toast("Libération de la mémoire…");
    try { const r = await api.appeler("perf_liberer"); toast(r.message); } catch (e) { toast(String(e)); }
    rafraichir();
  }

  function toast(texte) {
    document.querySelectorAll(".toast").forEach((t) => t.remove());
    const t = el("div.toast", texte);
    document.body.append(t);
    setTimeout(() => t.remove(), 4200);
  }

  async function dessiner() {
    if (!dernier) return;
    D.onglets.querySelectorAll(".onglet").forEach((o) => o.setAttribute("aria-selected", String(o.dataset.cle === onglet)));
    D.jeu.replaceChildren(svg("manette"), dernier.mode_jeu ? "Mode jeu actif" : "Mode jeu");
    D.jeu.classList.toggle("actif", dernier.mode_jeu);
    const contenu = onglet === "processus" ? processus(dernier) : onglet === "demarrage" ? await demarrage() : modeJeu(dernier);
    D.corps.replaceChildren(...contenu);
  }

  async function rafraichir(force) {
    dernier = await api.appeler("perf_instantane");
    tuiles(dernier);
    // On ne redessine pas la liste sous la souris de quelqu'un qui est en
    // train de choisir : seulement les processus, et pas pendant une saisie.
    const occupe = document.activeElement && ["SELECT", "INPUT"].includes(document.activeElement.tagName) && onglet === "processus";
    if (force || (onglet === "processus" && !occupe) || !D.corps.childElementCount) dessiner();
    else D.jeu.classList.toggle("actif", dernier.mode_jeu);
  }

  // Deux secondes quand la fenêtre est vue, rien quand elle est cachée : un
  // moniteur ne doit pas être ce qui consomme.
  function boucle() {
    if (!document.hidden) rafraichir();
    setTimeout(boucle, 2000);
  }

  window.grenosOnglet = (cle) => { onglet = cle; dessiner(); };

  (async function () {
    habiller(await api.appeler("apparence"));
    cadre();
    window.grenos.pret();
    await api.appeler("perf_instantane"); // premier relevé : la base des pourcentages
    setTimeout(boucle, 600);
  })();
})();
