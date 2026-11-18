// Jeux : la carte graphique, GameMode, les outils pour jouer, et les jeux
// demandés avec la vérité sur chacun. Promettre qu'un jeu tourne alors que son
// éditeur refuse Linux ferait perdre une soirée à quelqu'un.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, installer } = window.grenos;
  const D = {};
  let v = null;
  const nombre = (x, d = 1) => Number(x).toFixed(d).replace(".", ",");

  function dire(t) { D.dire.textContent = t || ""; }
  const ouvrir = (cle) => api.appeler("jeux_ouvrir", { cle }).catch((e) => dire(String(e)));

  async function poser(nom, source, identifiant) {
    await installer({ nom, source, identifiant });
    charger();
  }

  function ligne(nom, valeur) { return el("div.jx-ligne", el("span", nom), el("span.note", valeur)); }
  function section(t) { return el("div.etiquette.jx-section", t); }
  function bouton(texte, genre, rappel) { return el(`button.bouton.petit${genre ? "." + genre : ""}`, { onclick: rappel }, texte); }

  function carteGraphique() {
    const c = el("section.carte.verre.jx-bloc",
      ligne("Carte", v.carte), ligne("Pilote en cours", v.pilote),
      ligne("3D (Vulkan)", v.vulkan ? v.vulkan_detail : "aucune"));
    if (v.pilote_conseil) c.append(el("p.note", v.pilote_conseil));
    if (v.pilote_paquet) c.append(bouton("Installer le pilote NVIDIA", "principal", async () => {
      poser("Pilote NVIDIA", "apt", await api.appeler("jeux_paquet_nvidia"));
    }));
    return c;
  }

  function carteGamemode() {
    const c = el("section.carte.verre.jx-bloc",
      ligne("GameMode", v.gamemode_pose ? "installé" : "absent"),
      ligne("Priorité accordée à ce compte", v.gamemode_arme ? "oui" : (v.gamemode_pose ? "pas encore" : "—")));
    let note;
    if (v.gamemode_pose && v.gamemode_arme) note = "Un jeu lancé par GameMode passe devant le reste : priorité processeur, priorité disque, et l'écran ne s'éteint plus pendant qu'on joue à la manette.";
    else if (v.gamemode_pose) note = "Tout est en place sauf une chose : ce compte vient d'être ajouté au groupe « gamemode », et cela ne prend effet qu'au prochain démarrage. Jusque-là GameMode tournera sans pouvoir changer la priorité.";
    else note = "GameMode n'est pas sur cette machine. Sans lui, un jeu est traité comme n'importe quel programme.";
    c.append(el("p.note", note));
    if (v.gamemode_pose) {
      // L'option vit dans le compte Steam de la personne : on l'écrit, elle la colle.
      c.append(el("p.note", "Dans Steam : clic droit sur un jeu → Propriétés → Options de lancement, et colle ceci"),
        el("input.champ.mono.jx-option", { value: v.option_steam, readonly: true, onfocus: (e) => e.target.select() }));
    }
    return c;
  }

  function outil(o) {
    return el("section.carte.verre.jx-outil",
      el("div", el("strong", o.nom), el("p.note", o.note)),
      o.installe ? el("span.jx-marche", "Installé") : bouton("Installer", "principal", () => poser(o.nom, o.source, o.identifiant)));
  }

  function carteJeu(j) {
    const c = el("section.carte.verre.jx-jeu",
      el("div.jx-entete", el("strong", j.nom),
        el(`span.${j.etat === "marche" ? "jx-marche" : "jx-nuage"}`, j.etat === "marche" ? "fonctionne" : "en nuage")),
      el("p.note", j.note));
    const rangee = (...e) => el("div.jx-rangee", ...e);
    if (j.action === "steam") {
      c.append(v.heroic_pose
        ? rangee(bouton("Ouvrir Epic (gratuit)", "principal", () => ouvrir("heroic")),
          el("span.note", "Heroic est installé. Dedans : connexion Epic, puis Rocket League s'ajoute à ta bibliothèque — il est gratuit."))
        : rangee(bouton("Installer Epic et jouer", "principal", () => {
          dire("Heroic arrive : c'est lui qui ouvre Epic sous Linux. Connecte-toi ensuite avec ton compte Epic — Rocket League y est gratuit.");
          poser("Heroic", "flatpak", v.heroic);
        }), bouton("Voir la fiche Epic", "discret", () => ouvrir("epic"))));
      c.append(el("ol.jx-pas",
        el("li", "Ouvre Heroic et connecte-toi avec ton compte Epic."),
        el("li", "Récupère Rocket League : il est gratuit, un clic suffit."),
        el("li", "Au premier lancement, laisse Heroic télécharger son moteur (Proton-GE) — c'est lui qui fait tourner le jeu, et l'anti-triche l'accepte sous Linux depuis avril 2026.")));
      c.append(el("p.note", "Si tu l'avais déjà sur Steam avant 2020"));
      c.append(v.rocket_prete
        ? rangee(bouton("Jouer", "principal", () => ouvrir("rocket")), el("span.note", "Proton Experimental est réglé pour ce jeu."))
        : rangee(bouton("Préparer et jouer", "principal", preparerRocket), bouton("Ouvrir la fiche", "discret", () => ouvrir("rocket"))));
    } else if (j.action === "flatpak") {
      if (!v.vulkan) c.append(el("p.note", "Cette machine n'a pas Vulkan : Sober retombera de lui-même sur OpenGL, et Roblox sera plus lent."));
      c.append(v.roblox_pose
        ? rangee(bouton("Jouer", "principal", () => ouvrir("roblox")), el("span.note", "Sober est installé."))
        : rangee(bouton("Installer et jouer", "principal", () => poser(j.nom, "flatpak", j.identifiant)),
          bouton("Ouvrir le site", "discret", () => ouvrir("roblox-site"))));
    } else {
      c.append(v.fortnite_pose
        ? rangee(bouton("Ouvrir Fortnite", "principal", () => ouvrir("fortnite")))
        : rangee(bouton("Installer Fortnite en nuage", "principal", poserFortnite)));
    }
    return c;
  }

  // Chaque refus dit quoi faire ensuite : « ça n'a pas marché » n'est pas une réponse.
  async function preparerRocket() {
    const manque = await api.appeler("jeux_preparer_rocket");
    if (manque === "steam") {
      dire("Steam n'est pas là : il arrive, puis relance « Préparer et jouer ».");
      poser("Steam", "flatpak", "com.valvesoftware.Steam");
    } else if (manque === "jamais") {
      dire("Lance Steam une fois et connecte-toi : il écrit sa configuration à ce moment-là, et c'est elle qu'il faut régler. Reviens ensuite ici.");
    } else if (manque === "ouvert") {
      dire("Ferme Steam d'abord : il réécrit sa configuration en se fermant, et effacerait le réglage.");
    } else if (manque === "illisible") {
      dire("La configuration de Steam n'a pas la forme attendue. Rien n'a été modifié. Dans Steam : Rocket League → Propriétés → Compatibilité → Proton Experimental.");
    } else {
      dire("Proton Experimental est réglé pour Rocket League. Steam le téléchargera au premier lancement.");
      charger();
    }
  }

  async function poserFortnite() {
    const manque = await api.appeler("jeux_poser_fortnite");
    if (manque === "chromium") {
      dire("Il faut d'abord un navigateur Chromium : le service de Microsoft refuse Firefox. Installation en cours…");
      poser("Chromium", "apt", "chromium");
      return;
    }
    dire("Fortnite est posé sur le bureau et dans le menu. Il s'ouvre comme une application, sans barre d'adresse.");
    charger();
  }

  async function charger() {
    try { v = await api.appeler("jeux_vue"); } catch (e) { D.corps.replaceChildren(el("p.note", `La machine n'a pas pu être lue : ${e}`)); return; }
    if (!v) return;
    D.sous.textContent = `${v.coeurs} cœurs, ${nombre(v.memoire)} Go de mémoire.`;
    D.corps.replaceChildren(
      v.virtuelle ? el("section.jx-avertissement", el("div.etiquette", "À lire avant de commencer"),
        el("p", "Cette machine est virtuelle : elle n'a pas de vraie carte graphique, et aucun jeu récent n'y tournera. Pour jouer, installe grenOS sur le disque de ton ordinateur — c'est la seule façon d'avoir la 3D.")) : null,
      section("Carte graphique"), carteGraphique(),
      section("La priorité donnée au jeu"), carteGamemode(),
      section("Pour jouer"), ...v.outils.map(outil),
      section("Les jeux que tu as demandés"), ...v.jeux.map(carteJeu));
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.sous = el("small.note", "Lecture de la machine…");
    D.corps = el("div.jx-corps", el("p.note", "On interroge la carte graphique — quelques secondes au plus."));
    D.dire = el("p.jx-dire");
    document.body.append(el("div.page-jeux",
      el("header.tete", el("div.logo", svg("manette")), el("div", el("h1", "Jeux"), D.sous)),
      D.dire, D.corps));
    pret();
    charger();
  })();
})();
