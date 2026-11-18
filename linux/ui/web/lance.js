// GrenLaunch : la bibliothèque (Steam, Heroic, Flathub), la découverte, les
// plateformes. Un jeu installé depuis Steam apparaît tout seul : la page
// relit les manifestes de Steam à chaque retour sur la fenêtre et toutes les
// dix secondes tant qu'elle est vue.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore } = window.grenos;
  const D = {};
  let vue = null;
  let onglet = "bibliotheque";
  let filtre = "tous";
  let recherche = "";
  let rayon = "installes"; // installes | compte
  const images = new Map();
  const enCours = new Map(); // identifiant flatpak → { part, texte }

  const ETATS = {
    natif: ["ok", "Natif Linux"],
    proton: ["", "Via Proton"],
    nuage: ["alerte", "En nuage"],
    indisponible: ["danger", "Non disponible sur Linux"],
  };

  function cadre() {
    document.body.append(aurore());
    D.onglets = el("nav.onglets", { role: "tablist" });
    for (const [cle, titre] of [["bibliotheque", "Ma bibliothèque"], ["decouvrir", "Découvrir"], ["plateformes", "Plateformes"]]) {
      D.onglets.append(el("button.onglet", { role: "tab", "data-cle": cle, onclick: () => { onglet = cle; dessiner(); } }, titre));
    }
    D.recherche = el("input.champ.recherche", { type: "search", placeholder: "Chercher un jeu…",
      oninput: (e) => { recherche = e.target.value.trim().toLowerCase(); dessiner(); } });
    D.une = el("section.une");
    D.corps = el("section.corps-lance");
    document.body.append(el("div.page-perf.page-lance",
      el("header.tete",
        el("div.logo", svg("manette")),
        el("div", el("h1", "GrenLaunch"), el("small.note", "Tous tes jeux au même endroit")),
        el("div.espace"), D.recherche),
      D.une, D.onglets, D.corps));
  }

  // Les images : le cache local de Steam d'abord, puis les serveurs officiels
  // (Steam pour ses jeux, Flathub pour les icônes). Une image qui ne vient pas
  // laisse la carte à ses deux couleurs, avec le nom du jeu en grand.
  const STEAM_CDN = "https://cdn.cloudflare.steamstatic.com/steam/apps/";
  const FLATHUB = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/";
  const couverture = (appid) => appid ? `${STEAM_CDN}${appid}/library_600x900.jpg` : null;
  const bandeau = (appid) => appid ? `${STEAM_CDN}${appid}/header.jpg` : null;
  const icone = (id) => id ? `${FLATHUB}${id}.png` : null;

  function jaquette(nom, couleurs, art = {}) {
    const j = el("div.jaquette", { style: `--c1:${couleurs[0]};--c2:${couleurs[1]}` });
    function repli() {
      j.classList.add("repli");
      j.replaceChildren(el("span.titre-repli", nom));
    }
    function logo() {
      const i = el("img.logo-jeu", { src: art.logo, alt: "", loading: "lazy" });
      i.addEventListener("error", repli);
      j.append(i, el("span.titre-jaquette", nom));
    }
    if (art.couverture) {
      const img = el("img.couverture", { src: art.couverture, alt: nom, loading: "lazy" });
      img.addEventListener("error", () => { img.remove(); art.logo ? logo() : repli(); });
      j.append(img);
    } else if (art.logo) logo();
    else repli();
    return j;
  }

  function artDuCatalogue(j, forme) {
    return {
      couverture: j.steam ? (forme === "portrait" ? couverture(j.steam) : bandeau(j.steam)) : null,
      logo: j.logo || (j.source === "flatpak" ? icone(j.identifiant) : null),
    };
  }

  function couleursDe(nom) {
    let h = 0;
    for (const c of nom) h = (h * 31 + c.charCodeAt(0)) % 360;
    return [`hsl(${h} 45% 16%)`, `hsl(${(h + 50) % 360} 70% 50%)`];
  }

  // --------------------------------------------------------- les actions
  async function jouer(origine, id) {
    try { await api.appeler("lance_jouer", { origine, id }); toast("Lancement…"); }
    catch (e) { toast(String(e)); }
  }

  async function installerFlatpak(identifiant) {
    enCours.set(identifiant, { part: 0, texte: "Préparation…" });
    dessiner();
    try { await api.appeler("lance_installer", { identifiant }); }
    catch (e) { enCours.delete(identifiant); toast(String(e)); dessiner(); }
  }

  function boutonFlatpak(identifiant, libelle) {
    const p = enCours.get(identifiant);
    if (p) return el("div.installation", el("div.progression", el("i", { style: `width:${Math.max(3, p.part)}%` })), el("small.note", p.texte));
    return el("button.bouton.principal.petit", { onclick: () => installerFlatpak(identifiant) }, svg("paquet"), libelle);
  }

  function action(j) {
    if (j.etat === "indisponible") return el("button.bouton.petit", { disabled: true }, svg("cadenas"), "Pas sur Linux");
    if (j.source === "nuage") return el("button.bouton.principal.petit", { onclick: () => jouer("nuage", j.adresse) }, svg("wifi"), "Jouer en nuage");
    if (j.installe) {
      if (j.source === "steam") return el("button.bouton.principal.petit", { onclick: () => jouer("steam", String(j.steam)) }, svg("fleche"), "Jouer");
      if (j.source === "flatpak") return el("button.bouton.principal.petit", { onclick: () => jouer("flatpak", j.identifiant) }, svg("fleche"), "Jouer");
      if (j.source === "heroic") {
        const parSteam = vue.bibliotheque.some((b) => b.origine === "steam" && b.id === String(j.steam));
        return el("button.bouton.principal.petit", { onclick: () => parSteam ? jouer("steam", String(j.steam)) : jouer("heroic-legendary", j.identifiant) }, svg("fleche"), "Jouer");
      }
    }
    if (j.source === "flatpak") return boutonFlatpak(j.identifiant, "Installer");
    if (j.source === "steam") {
      if (!vue.steam) return boutonFlatpak("com.valvesoftware.Steam", "Installer Steam d'abord");
      return el("button.bouton.principal.petit", { onclick: async () => {
        try { await api.appeler("lance_steam_installer", { appid: j.steam }); toast("Steam s'ouvre sur l'installation. Le jeu apparaîtra ici dès qu'il sera prêt."); }
        catch (e) { toast(String(e)); }
      } }, svg("paquet"), "Installer avec Steam");
    }
    if (j.source === "heroic") {
      if (!vue.heroic) return boutonFlatpak("com.heroicgameslauncher.hgl", "Installer Heroic d'abord");
      return el("button.bouton.principal.petit", { onclick: async () => {
        try { await api.appeler("lance_ouvrir", { plateforme: "heroic" }); toast("Dans Heroic : connecte ton compte Epic, puis installe le jeu. Il apparaîtra ici."); }
        catch (e) { toast(String(e)); }
      } }, svg("paquet"), "Obtenir dans Heroic");
    }
    return null;
  }

  // ---------------------------------------------------------- les écrans
  function aLaUne() {
    const j = vue.jeux.find((x) => x.cle === "rocket-league");
    if (!j || onglet === "plateformes") { D.une.replaceChildren(); return; }
    const [genre, texte] = ETATS[j.etat];
    const fond = j.steam ? `--art:url("${STEAM_CDN}${j.steam}/library_hero.jpg");` : "";
    D.une.replaceChildren(el("div.banniere", { style: `--c1:${j.couleurs[0]};--c2:${j.couleurs[1]};${fond}` },
      el("div.banniere-texte",
        el("span.etiquette", "À la une"),
        el("h2", j.nom),
        el("div.badges", el(`span.badge${genre ? "." + genre : ""}`, texte), el("span.badge", j.genre)),
        el("p", j.note),
        action(j)),
      el("div.banniere-art", svg("manette"))));
  }

  function carteBibliotheque(b) {
    const cat = vue.jeux.find((j) => (j.steam && String(j.steam) === b.id) || j.identifiant === b.id);
    const couleurs = cat ? cat.couleurs : couleursDe(b.nom);
    const ORIGINES = { steam: "Steam", "heroic-legendary": "Epic · Heroic", "heroic-gog": "GOG · Heroic", flatpak: "Flathub" };
    const art = cat ? artDuCatalogue(cat, "portrait") : {};
    if (b.origine === "steam") art.couverture = images.get(b.id) || b.image || couverture(b.id);
    if (b.origine === "flatpak") art.logo = art.logo || icone(b.id);
    return el("button.carte-jeu", { type: "button", title: `Jouer à ${b.nom}`, onclick: () => jouer(b.origine, b.id) },
      jaquette(b.nom, couleurs, art),
      el("div.survol", el("span.lecture", svg("fleche"))),
      el("div.legende", el("b", b.nom), el("small.note", ORIGINES[b.origine] + (b.taille_go > 0.05 ? ` · ${b.taille_go.toFixed(1).replace(".", ",")} Go` : ""))));
  }

  // Un jeu du compte Steam qui n'est pas installé ici : sa jaquette, grisée,
  // et un clic l'installe — c'est Steam qui télécharge, GrenLaunch le verra
  // arriver tout seul.
  function carteCompte(b) {
    const cat = vue.jeux.find((j) => j.steam && String(j.steam) === b.id);
    return el("button.carte-jeu.a-installer", { type: "button", title: `Installer ${b.nom}`, onclick: async () => {
      try { await api.appeler("lance_steam_installer", { appid: Number(b.id) }); toast(`Steam installe ${b.nom}. Il apparaîtra dans « Installés » dès qu'il sera prêt.`); }
      catch (e) { toast(String(e)); }
    } },
      jaquette(b.nom, cat ? cat.couleurs : couleursDe(b.nom), { couverture: couverture(b.id) }),
      el("div.survol", el("span.lecture", svg("paquet"))),
      el("div.legende", el("b", b.nom), el("small.note", "Steam · à installer")));
  }

  function rayons() {
    const n = vue.bibliotheque.length, m = vue.compte.length;
    return el("div.puces-filtre",
      el("button.puce-filtre", { type: "button", "aria-pressed": String(rayon === "installes"), onclick: () => { rayon = "installes"; dessiner(); } }, `Installés · ${n}`),
      el("button.puce-filtre", { type: "button", "aria-pressed": String(rayon === "compte"), onclick: () => { rayon = "compte"; dessiner(); } }, `Tous mes jeux Steam · ${n + m}`));
  }

  function bibliotheque() {
    const cherche = (b) => !recherche || b.nom.toLowerCase().includes(recherche);
    if (rayon === "compte") {
      const installes = vue.bibliotheque.filter((b) => b.origine === "steam" && cherche(b));
      const autres = vue.compte.filter(cherche);
      if (!installes.length && !autres.length) {
        return [rayons(), el("div.vide", el("div.halo-vide", svg("manette")), el("h2", "Aucun jeu Steam trouvé"),
          el("p.sous", vue.steam ? "Ouvre Steam une fois et connecte-toi : les jeux de ton compte apparaîtront ici." : "Installe Steam depuis l'onglet Plateformes, connecte-toi, et tes jeux apparaîtront ici."))];
      }
      return [rayons(), el("div.grille-jeux", ...installes.map(carteBibliotheque), ...autres.map(carteCompte))];
    }
    const liste = vue.bibliotheque.filter(cherche);
    if (!vue.bibliotheque.length && !vue.compte.length) {
      return [el("div.vide",
        el("div.halo-vide", svg("manette")),
        el("h2", "Ta bibliothèque est vide"),
        el("p.sous", "Installe un jeu depuis Découvrir, ou depuis Steam et Heroic : tout ce qu'ils installent apparaît ici tout seul."),
        el("div.actions", el("button.bouton.principal", { onclick: () => { onglet = "decouvrir"; dessiner(); } }, svg("etoile"), "Découvrir des jeux"),
          el("button.bouton", { onclick: () => { onglet = "plateformes"; dessiner(); } }, "Plateformes")))];
    }
    return [rayons(), el("div.grille-jeux", ...liste.map(carteBibliotheque))];
  }

  function decouvrir() {
    const FILTRES = [["tous", "Tous"], ["gratuit", "Gratuits"], ["natif", "Natifs"], ["proton", "Via Proton"], ["nuage", "En nuage"], ["indisponible", "Non disponibles"]];
    const puces = el("div.puces-filtre", ...FILTRES.map(([cle, t]) =>
      el("button.puce-filtre", { type: "button", "aria-pressed": String(filtre === cle), onclick: () => { filtre = cle; dessiner(); } }, t)));
    const jeux = vue.jeux.filter((j) =>
      (filtre === "tous" ? j.etat !== "indisponible" : filtre === "gratuit" ? /Gratuit|Libre/.test(j.genre) : j.etat === filtre) &&
      (!recherche || j.nom.toLowerCase().includes(recherche)));
    return [puces, el("div.grille-fiches", ...jeux.map((j) => {
      const [genre, texte] = ETATS[j.etat];
      return el(`article.fiche-jeu${j.etat === "indisponible" ? ".grise" : ""}`,
        jaquette(j.nom, j.couleurs, artDuCatalogue(j, "bandeau")),
        el("div.fiche-corps",
          el("div.badges", el(`span.badge${genre ? "." + genre : ""}`, texte), j.installe ? el("span.badge.ok", svg("coche"), "Installé") : null),
          el("b", j.nom), el("small.note", j.genre), el("p.note.resume-note", j.note),
          action(j)));
    }))];
  }

  function plateformes() {
    return [el("div.grille-fiches.plateformes", ...vue.plateformes.map((p) =>
      el("article.fiche-jeu",
        jaquette(p.nom, p.couleurs, { logo: icone(p.identifiant) }),
        el("div.fiche-corps",
          p.installe ? el("div.badges", el("span.badge.ok", svg("coche"), "Installé")) : null,
          el("b", p.nom), el("p.note.resume-note", p.note),
          p.installe
            ? el("button.bouton.principal.petit", { onclick: () => (p.cle === "steam" || p.cle === "heroic") ? api.appeler("lance_ouvrir", { plateforme: p.cle }).catch((e) => toast(String(e))) : jouer("flatpak", p.identifiant) }, svg("fleche"), "Ouvrir")
            : boutonFlatpak(p.identifiant, "Installer")))))];
  }

  function dessiner() {
    if (!vue) return;
    D.onglets.querySelectorAll(".onglet").forEach((o) => o.setAttribute("aria-selected", String(o.dataset.cle === onglet)));
    aLaUne();
    D.corps.replaceChildren(...(onglet === "bibliotheque" ? bibliotheque() : onglet === "decouvrir" ? decouvrir() : plateformes()));
  }

  function toast(texte) {
    document.querySelectorAll(".toast").forEach((t) => t.remove());
    const t = el("div.toast", texte);
    document.body.append(t);
    setTimeout(() => t.remove(), 5000);
  }

  // Les images ne sont lues que pour les jeux qu'on n'a pas encore vus : les
  // relire toutes les dix secondes coûterait de la mémoire pour rien.
  async function rafraichir() {
    const connus = vue ? new Set(vue.bibliotheque.map((b) => b.id)) : new Set();
    let v = await api.appeler("lance_vue", { images: false });
    const nouveaux = v.bibliotheque.filter((b) => b.origine === "steam" && !images.has(b.id));
    if (nouveaux.length) {
      const avec = await api.appeler("lance_vue", { images: true });
      avec.bibliotheque.forEach((b) => { if (b.image) images.set(b.id, b.image); });
      v = avec;
    }
    const arrives = vue ? v.bibliotheque.filter((b) => !connus.has(b.id)) : [];
    vue = v;
    if (arrives.length) toast(`${arrives.map((b) => b.nom).join(", ")} : prêt à jouer.`);
    const occupe = document.activeElement === D.recherche;
    if (!occupe || !D.corps.childElementCount) dessiner();
  }

  api.ecouter("lance-progres", (p) => { enCours.set(p.identifiant, p); dessiner(); });
  api.ecouter("lance-fin", (f) => { enCours.delete(f.identifiant); toast(f.texte); rafraichir(); });
  window.addEventListener("focus", () => rafraichir());

  function boucle() {
    if (!document.hidden) rafraichir();
    setTimeout(boucle, 10000);
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    cadre();
    boucle();
  })();
})();
