// Connexions : un interrupteur en haut, ce qui existe en dessous, un bouton
// par ligne. Le mot de passe est demandé au moment où il est demandé.
//
// La fenêtre s'affiche AVANT de lire le matériel (nmcli, bluetoothctl) :
// l'état arrive ensuite, et une note « aucune carte » n'est écrite qu'une fois
// qu'on a regardé.
//
// 27 novembre (Grenofar) : le réseau Wi-Fi utilisé est dit même quand le
// câble est branché ; un mot de passe enregistré est essayé sans rien
// demander, et s'il échoue la page le REDEMANDE, tant que la personne ne
// renonce pas ; « Oublier » un réseau ou un appareil ; les appareils
// Bluetooth par leur nom et leur genre (enceinte, clavier…).
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret } = window.grenos;
  const D = {};
  let onglet = "wifi";

  function dire(t) { D.dire.textContent = t || ""; }

  function barres(force) {
    const n = Math.min(((Math.max(force, 0) / 25) | 0) + (force ? 1 : 0), 4);
    return el("span.cx-barres", [0, 1, 2, 3].map((i) => el(`i${i < n ? ".plein" : ""}`, { style: `height:${5 + i * 3}px` })));
  }

  function interrupteur(rappel) {
    const c = el("input.cx-inter", { type: "checkbox", role: "switch", disabled: true });
    c.addEventListener("change", () => rappel(c.checked));
    return c;
  }

  function ligne(titre, sous, connecte, droite) {
    return el("div.cx-ligne.carte.verre",
      el("div.cx-texte", el("strong", titre), el(`small${connecte ? ".connecte" : ""}`, sous)),
      ...droite);
  }

  function bouton(texte, genre, rappel) {
    const b = el(`button.bouton.petit${genre ? "." + genre : ""}`, texte);
    b.addEventListener("click", async () => { b.disabled = true; try { await rappel(); } finally { b.disabled = false; } });
    return b;
  }

  async function agir(commande, args, ensuite) {
    try { dire(await api.appeler(commande, args)); } catch (e) { dire(String(e)); }
    setTimeout(ensuite, 1000);
  }

  // ---- L'en-tête : par où la machine est reliée ----------------------------
  async function lireEtat() {
    const e = await api.appeler("connexions_etat");
    if (!e) return null;
    const wifi = e.wifi_nom ? `au Wi-Fi « ${e.wifi_nom} »` : "";
    const cable = e.lien.connecte && e.lien.par === "câble";
    D.sous.textContent = cable && wifi ? `Connecté par câble et ${wifi}.`
      : wifi ? `Connecté ${wifi}.`
      : e.lien.connecte ? `Connecté par ${e.lien.par}.`
      : "Pas encore connecté.";
    return e;
  }

  // ---- Wi-Fi --------------------------------------------------------------
  async function chercherWifi() {
    dire("Recherche des réseaux…");
    const [liste, enregistres] = await Promise.all([
      api.appeler("connexions_wifi_liste"),
      api.appeler("connexions_wifi_enregistres"),
    ]);
    const visibles = liste || [];
    const lignes = visibles.map((r) => ligne(r.nom,
      r.connecte ? "connecté — c'est ton réseau" : r.connu ? "enregistré" : r.protege ? "protégé par mot de passe" : "ouvert",
      r.connecte, [
        barres(r.force),
        r.connu ? bouton("Oublier", "discret", () => oublier(r.nom)) : null,
        r.connecte
          ? bouton("Se déconnecter", "discret", () => agir("connexions_wifi_deconnecter", { nom: r.nom }, rafraichir))
          : bouton("Se connecter", "principal", () => connecter(r)),
      ].filter(Boolean)));
    // Les réseaux enregistrés hors de portée : on peut quand même les oublier.
    const loin = (enregistres || []).filter((n) => !visibles.some((r) => r.nom === n));
    if (loin.length) {
      lignes.push(el("p.note.cx-titre", "Enregistrés, hors de portée"));
      for (const n of loin) lignes.push(ligne(n, "enregistré", false, [bouton("Oublier", "discret", () => oublier(n))]));
    }
    D.wifi.replaceChildren(...lignes);
    const n = visibles.length;
    dire(n ? `${n} réseau${n > 1 ? "x" : ""} trouvé${n > 1 ? "s" : ""}.` : "Aucun réseau trouvé.");
  }

  function rafraichir() { lireEtat(); chercherWifi(); }

  async function connecter(r) {
    let mot = "";
    let erreur = "";
    // Protégé et rien d'enregistré : le mot de passe d'abord.
    if (r.protege && !r.connu) {
      mot = await demanderMot(r.nom, "");
      if (mot == null) return;
    }
    for (;;) {
      dire(`Connexion à ${r.nom}…`);
      try {
        dire(await api.appeler("connexions_wifi_connecter", { nom: r.nom, motDePasse: mot }));
        break;
      } catch (e) {
        erreur = String(e);
        dire(erreur);
        // Même avec un mot de passe enregistré : on le redemande.
        if (!r.protege) break;
        mot = await demanderMot(r.nom, erreur);
        if (mot == null) break;
      }
    }
    setTimeout(rafraichir, 1000);
  }

  async function oublier(nom) {
    if (!(await confirmer(`Oublier ${nom} ?`, "Son mot de passe sera effacé : il faudra le retaper pour s'y reconnecter."))) return;
    await agir("connexions_wifi_oublier", { nom }, rafraichir);
  }

  function dialogue(contenu) {
    const fond = el("div.cx-fond");
    fond.append(contenu);
    document.body.append(fond);
    return fond;
  }

  function demanderMot(nom, erreur) {
    return new Promise((ok) => {
      const champ = el("input.champ", { type: "password", placeholder: "Mot de passe du réseau", autocomplete: "off" });
      const voir = el("input", { type: "checkbox" });
      voir.addEventListener("change", () => { champ.type = voir.checked ? "text" : "password"; });
      let fond = null;
      const fin = (v) => { fond.remove(); ok(v); };
      fond = dialogue(el("form.cx-dialogue.carte", { onsubmit: (e) => { e.preventDefault(); if (champ.value) fin(champ.value); } },
        el("h2", `Mot de passe de ${nom}`),
        erreur ? el("p.cx-erreur", `${erreur} Retape le mot de passe.`) : el("p.note", `Réseau : ${nom}`),
        champ,
        el("label.cx-voir", voir, "Afficher le mot de passe"),
        el("div.cx-boutons",
          el("button.bouton.discret", { type: "button", onclick: () => fin(null) }, "Annuler"),
          el("button.bouton.principal", { type: "submit" }, "Se connecter"))));
      fond.addEventListener("keydown", (e) => { if (e.key === "Escape") fin(null); });
      champ.focus();
    });
  }

  function confirmer(titre, texte) {
    return new Promise((ok) => {
      let fond = null;
      const fin = (v) => { fond.remove(); ok(v); };
      const oui = el("button.bouton.principal", { type: "button", onclick: () => fin(true) }, "Oublier");
      fond = dialogue(el("div.cx-dialogue.carte",
        el("h2", titre), el("p.note", texte),
        el("div.cx-boutons",
          el("button.bouton.discret", { type: "button", onclick: () => fin(false) }, "Annuler"), oui)));
      fond.addEventListener("keydown", (e) => { if (e.key === "Escape") fin(false); });
      oui.focus();
    });
  }

  // ---- Bluetooth -------------------------------------------------------------
  const GENRES = {
    audio: "enceinte ou casque", clavier: "clavier", souris: "souris", manette: "manette",
    telephone: "téléphone", ordinateur: "ordinateur",
  };

  function sousTitreBt(a) {
    const genre = GENRES[a.genre] || "";
    if (a.connecte) return genre ? `connecté — ${genre}` : "connecté";
    if (a.apparie) return genre ? `associé — ${genre}` : "associé";
    return genre ? `${genre} — à associer` : "à associer";
  }

  async function poserBt() {
    const liste = (await api.appeler("connexions_bt_liste")) || [];
    D.bt.replaceChildren(...liste.map((a) => ligne(a.nom, sousTitreBt(a), a.connecte, [
      a.apparie ? bouton("Oublier", "discret", async () => {
        if (await confirmer(`Oublier ${a.nom} ?`, "Il faudra l'associer de nouveau pour s'en servir."))
          await agir("connexions_bt_oublier", { adresse: a.adresse, nom: a.nom }, poserBt);
      }) : null,
      a.connecte
        ? bouton("Déconnecter", "discret", () => agir("connexions_bt_delier", { adresse: a.adresse, nom: a.nom }, poserBt))
        : bouton("Connecter", "principal", async () => {
          dire(a.apparie ? `Connexion à ${a.nom}…` : `Association avec ${a.nom}… (laisse-le en mode association)`);
          await agir("connexions_bt_lier", { adresse: a.adresse, nom: a.nom }, poserBt);
        }),
    ].filter(Boolean))));
    if (liste.length) dire(`${liste.length} appareil${liste.length > 1 ? "s" : ""}.`);
    else dire("Aucun appareil. Mets ton enceinte ou ton casque en mode association, puis « Rechercher ».");
  }

  async function chercherBt() {
    dire("Recherche des appareils (10 secondes)…");
    await api.appeler("connexions_bt_chercher");
    poserBt();
  }

  // ---- Cadre ----------------------------------------------------------------
  function page(cle, titre, inter, chercher, liste, note) {
    return el(`section.cx-page${cle === onglet ? "" : ".cache"}`, { "data-cle": cle },
      el("div.cx-haut", el("strong", titre),
        el("button.bouton.discret.petit", { onclick: chercher }, "Rechercher"), inter),
      liste, note);
  }

  function choisir(cle) {
    onglet = cle;
    for (const p of document.querySelectorAll(".cx-page")) p.classList.toggle("cache", p.dataset.cle !== cle);
    for (const b of document.querySelectorAll(".cx-onglets button")) b.classList.toggle("actif", b.dataset.cle === cle);
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.sous = el("small.note", "Lecture de l'état…");
    D.dire = el("p.cx-dire");
    D.wifi = el("div.cx-liste");
    D.bt = el("div.cx-liste");
    D.noteWifi = el("p.note.cx-note");
    D.noteBt = el("p.note.cx-note");
    D.interWifi = interrupteur(async (v) => { await api.appeler("connexions_wifi_radio", { actif: v }); setTimeout(rafraichir, 2000); });
    D.interBt = interrupteur(async (v) => { await api.appeler("connexions_bt_radio", { actif: v }); setTimeout(poserBt, 2000); });
    document.body.append(el("div.page-connexions",
      el("header.tete", el("div.logo", svg("wifi")), el("div", el("h1", "Connexions"), D.sous)),
      el("nav.cx-onglets",
        el("button.actif", { "data-cle": "wifi", onclick: () => choisir("wifi") }, svg("wifi"), "Wi-Fi"),
        el("button", { "data-cle": "bt", onclick: () => choisir("bt") }, svg("bt"), "Bluetooth")),
      page("wifi", "Wi-Fi", D.interWifi, chercherWifi, D.wifi, D.noteWifi),
      page("bt", "Bluetooth", D.interBt, chercherBt, D.bt, D.noteBt),
      D.dire));
    pret();
    // « grenos-ui connexions:bt » (le panneau du son) ouvre l'onglet Bluetooth,
    // aussi quand la fenêtre est déjà ouverte.
    window.grenosOnglet = (cle) => { if (cle === "bt" || cle === "wifi") choisir(cle); };
    if (window.GRENOS_ONGLET) window.grenosOnglet(window.GRENOS_ONGLET);
    chercherWifi();
    const e = await lireEtat();
    if (!e) return;
    D.interWifi.checked = e.wifi_on; D.interWifi.disabled = !e.wifi_la;
    D.interBt.checked = e.bt_on; D.interBt.disabled = !e.bt_la;
    if (!e.wifi_la) D.noteWifi.textContent = "Aucune carte Wi-Fi sur cette machine. Le câble, lui, fonctionne sans rien régler.";
    if (!e.bt_la) D.noteBt.textContent = "Aucun matériel Bluetooth sur cette machine. Dans une machine virtuelle, il faut d'abord le partager depuis l'hôte.";
    else poserBt();
  })();
})();
