// Connexions : un interrupteur en haut, ce qui existe en dessous, un bouton
// par ligne. Le mot de passe est demandé au moment où il est demandé.
//
// La fenêtre s'affiche AVANT de lire le matériel (nmcli, bluetoothctl) :
// l'état arrive ensuite, et une note « aucune carte » n'est écrite qu'une fois
// qu'on a regardé.
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

  // ---- Wi-Fi --------------------------------------------------------------
  async function chercherWifi() {
    dire("Recherche des réseaux…");
    const liste = (await api.appeler("connexions_wifi_liste")) || [];
    D.wifi.replaceChildren(...liste.map((r) => ligne(r.nom,
      r.connecte ? "connecté" : r.protege ? "protégé par mot de passe" : "ouvert", r.connecte, [
        barres(r.force),
        r.connecte
          ? bouton("Se déconnecter", "discret", () => agir("connexions_wifi_deconnecter", { nom: r.nom }, chercherWifi))
          : bouton("Se connecter", "principal", () => connecter(r)),
      ])));
    const n = liste.length;
    dire(n ? `${n} réseau${n > 1 ? "x" : ""} trouvé${n > 1 ? "s" : ""}.` : "Aucun réseau trouvé.");
  }

  async function connecter(r) {
    let mot = "";
    if (r.protege && !(await api.appeler("connexions_wifi_connu", { nom: r.nom }))) {
      mot = await demanderMot(r.nom);
      if (mot == null) return;
    }
    dire(`Connexion à ${r.nom}…`);
    await agir("connexions_wifi_connecter", { nom: r.nom, motDePasse: mot }, chercherWifi);
  }

  function demanderMot(nom) {
    return new Promise((ok) => {
      const champ = el("input.champ", { type: "password", placeholder: "Mot de passe du réseau", autocomplete: "off" });
      const voir = el("input", { type: "checkbox" });
      voir.addEventListener("change", () => { champ.type = voir.checked ? "text" : "password"; });
      const fond = el("div.cx-fond");
      const fin = (v) => { fond.remove(); ok(v); };
      fond.append(el("form.cx-dialogue.carte", { onsubmit: (e) => { e.preventDefault(); fin(champ.value); } },
        el("h2", `Mot de passe de ${nom}`),
        el("p.note", `Réseau : ${nom}`),
        champ,
        el("label.cx-voir", voir, "Afficher le mot de passe"),
        el("div.cx-boutons",
          el("button.bouton.discret", { type: "button", onclick: () => fin(null) }, "Annuler"),
          el("button.bouton.principal", { type: "submit" }, "Se connecter"))));
      fond.addEventListener("keydown", (e) => { if (e.key === "Escape") fin(null); });
      document.body.append(fond);
      champ.focus();
    });
  }

  // ---- Bluetooth -------------------------------------------------------------
  async function poserBt() {
    const liste = (await api.appeler("connexions_bt_liste")) || [];
    D.bt.replaceChildren(...liste.map((a) => ligne(a.nom, a.adresse, a.connecte, [
      a.connecte
        ? bouton("Déconnecter", "discret", () => agir("connexions_bt_delier", { adresse: a.adresse, nom: a.nom }, poserBt))
        : bouton("Connecter", "principal", async () => { dire(`Connexion à ${a.nom}…`); await agir("connexions_bt_lier", { adresse: a.adresse, nom: a.nom }, poserBt); }),
    ])));
    if (liste.length) dire(`${liste.length} appareil${liste.length > 1 ? "s" : ""}.`);
  }

  async function chercherBt() {
    dire("Recherche des appareils…");
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
    D.interWifi = interrupteur(async (v) => { await api.appeler("connexions_wifi_radio", { actif: v }); setTimeout(chercherWifi, 2000); });
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
    chercherWifi();
    const e = await api.appeler("connexions_etat");
    if (!e) return;
    D.sous.textContent = e.lien.connecte ? `Connecté en ${e.lien.par} à ${e.lien.nom}.` : "Pas encore connecté.";
    D.interWifi.checked = e.wifi_on; D.interWifi.disabled = !e.wifi_la;
    D.interBt.checked = e.bt_on; D.interBt.disabled = !e.bt_la;
    if (!e.wifi_la) D.noteWifi.textContent = "Aucune carte Wi-Fi sur cette machine. Le câble, lui, fonctionne sans rien régler.";
    if (!e.bt_la) D.noteBt.textContent = "Aucun matériel Bluetooth sur cette machine. Dans une machine virtuelle, il faut d'abord le partager depuis l'hôte.";
    else poserBt();
  })();
})();
