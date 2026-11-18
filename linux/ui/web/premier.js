// L'assistant de première configuration.
//
// Ce que la machine d'intégration fait, et qu'il faut tenir : elle appuie sur
// Entrée jusqu'à l'écran du nom, tape « ci », Entrée ; puis Entrée jusqu'au
// bout, avec UNE flèche vers le bas sur l'écran Apparence (Automatique →
// Sombre). Elle lit tout sur le port série. Donc : Entrée avance partout, les
// flèches changent le choix principal, le champ du nom a le clavier dès qu'il
// paraît, et chaque écran s'annonce avec la même phrase qu'en Python.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, mesurerFluidite } = window.grenos;

  // Les écrans qu'on sait dessiner. `premier.json` en décrit vingt et un ; les
  // autres sont sautés plutôt que montrés vides — une page vide serait une
  // promesse à l'écran sans rien derrière.
  const CONSTRUITS = ["bienvenue", "langue", "clavier", "compte", "parcours", "reseau",
    "apparence", "usage", "confidentialite", "securite", "majs", "applications",
    "sauvegardes", "optimisation", "systeme", "pare-feu", "journaux", "final"];
  const ICONES = { bienvenue: "etoile", langue: "langue", clavier: "clavier", compte: "compte",
    parcours: "fusee", reseau: "wifi", apparence: "palette", usage: "bureau",
    confidentialite: "oeil", securite: "bouclier", majs: "maj", applications: "paquet",
    optimisation: "eclair", sauvegardes: "paquet", systeme: "reglages", "pare-feu": "bouclier",
    journaux: "oeil", final: "coche" };
  const COURTS = { bienvenue: "Bienvenue", langue: "Langue", clavier: "Clavier", compte: "Compte",
    parcours: "Parcours", reseau: "Internet", apparence: "Apparence", usage: "Usage",
    confidentialite: "Confidentialité", securite: "Sécurité", majs: "Mises à jour",
    applications: "Applications", optimisation: "Matériel", sauvegardes: "Sauvegardes",
    systeme: "Système", "pare-feu": "Pare-feu", journaux: "Journaux", final: "Terminé" };

  const R = {
    langue: "fr_FR", clavier: "fr", clavier_index: "0", parcours: "personnalise",
    apparence: "auto", accent: "bleu", effets: "equilibres", usage: "bureautique",
    telemetrie: "aucune", majs: "automatiques", applications: "", optimiser: "non",
    compte: "", nom_affiche: "",
    sauvegardes: "non", parefeu: "actif", journaux: "mois", echange: "zram", swappiness: "equilibre",
  };
  let infos = null;
  let cle = null;
  let fini = false;
  let lent = false;
  const mdp = { actif: false, mot: "", confirme: "" };
  const ecrans = {};

  // ------------------------------------------------------------ la mécanique
  function liste() {
    const p = R.parcours;
    return infos.donnees.ecrans
      .filter((e) => e.parcours.includes(p) && CONSTRUITS.includes(e.cle))
      .map((e) => e.cle);
  }
  const titreDe = (c) => (infos.donnees.ecrans.find((e) => e.cle === c) || {}).titre || "";

  const D = {};
  function dessinerCadre() {
    document.body.append(aurore());
    D.rail = el("ol.etapes");
    D.barre = el("i");
    D.ecran = el("section.ecran");
    D.erreur = el("div.erreur", { role: "status" });
    D.retour = el("button.bouton.discret", { onclick: reculer }, svg("retour"), "Retour");
    D.suite = el("button.bouton.principal.grand", { onclick: avancer }, el("span"), svg("fleche"));
    D.aide = el("div.note.aide", el("kbd", "Entrée"), " pour continuer");
    document.body.append(
      el("div.oobe",
        el("aside.rail.verre",
          el("div.marque", el("div.logo", svg("marque")), el("div", el("b", "grenOS"), el("small", "Première configuration"))),
          D.rail,
          el("div.note", "Tout se change plus tard dans les Réglages.")),
        el("main.scene",
          el("div.progression.mobile", D.barre),
          el("div.cadre", D.ecran),
          el("footer.pied", D.retour, D.erreur, D.aide, D.suite))));
  }

  function majRail() {
    const l = liste();
    const rang = l.indexOf(cle);
    D.rail.replaceChildren(...l.map((c, i) =>
      el("li", { class: i < rang ? "fait" : i === rang ? "ici" : "" },
        el("span.puce", i < rang ? svg("coche") : svg(ICONES[c] || "etoile")),
        el("span", COURTS[c] || c))));
    D.barre.style.width = `${(rang / Math.max(1, l.length - 1)) * 100}%`;
  }

  function aller(nouvelle, sens = 1) {
    cle = nouvelle;
    const l = liste();
    const rang = l.indexOf(cle) + 1;
    D.erreur.textContent = "";
    D.retour.style.visibility = rang > 1 ? "visible" : "hidden";
    D.suite.querySelector("span").textContent =
      cle === "bienvenue" ? "Start" : cle === "final" ? "Démarrer" : "Continuer";
    D.suite.disabled = false;
    D.aide.style.display = cle === "final" ? "none" : "";
    D.ecran.className = "ecran apparait " + (sens > 0 ? "vers-avant" : "vers-arriere");
    D.ecran.dataset.cle = cle;
    D.ecran.replaceChildren(...[].concat(ecrans[cle]()));
    majRail();
    api.appeler("premier_dire", { texte: `premier : ecran ${cle} (${rang}/${l.length})` });
    if (cle === "compte") api.appeler("premier_dire", { texte: "premier ecran affiche, il demande un nom" });
  }

  async function peutContinuer() {
    if (cle === "compte") {
      const r = await api.appeler("premier_nom", { texte: R.nom_affiche || "" });
      if (r.probleme) {
        R.compte = "";
        return r.probleme === "Un nom, avec au moins une lettre." ? "Il faut un nom pour créer votre compte." : r.probleme;
      }
      R.compte = r.login;
      if (mdp.actif && mdp.mot) {
        if (mdp.mot.length < 4) return "Le mot de passe doit faire au moins 4 caractères.";
        if (mdp.mot !== mdp.confirme) return "Les deux mots de passe ne sont pas identiques.";
      }
    }
    if (cle === "parcours" && !["rapide", "personnalise", "dev"].includes(R.parcours)) return "Choisissez comment continuer.";
    return "";
  }

  let enCours = false;
  async function avancer() {
    if (fini || enCours) return;
    enCours = true;
    try {
      const souci = await peutContinuer();
      if (souci) { D.erreur.textContent = souci; secouer(); return; }
      if (cle === "final") return terminer();
      const l = liste();
      const i = l.indexOf(cle);
      if (i + 1 < l.length) aller(l[i + 1], 1);
    } finally { enCours = false; }
  }
  function reculer() {
    if (fini) return;
    const l = liste();
    const i = l.indexOf(cle);
    if (i > 0) aller(l[i - 1], -1);
  }
  function secouer() {
    D.ecran.classList.remove("secoue");
    void D.ecran.offsetWidth;
    D.ecran.classList.add("secoue");
  }

  // ------------------------------------------------------------- le clavier
  document.addEventListener("keydown", (e) => {
    if (fini) return;
    const champ = e.target instanceof HTMLInputElement;
    if (e.key === "Enter") {
      if (champ && e.target.dataset.entree) return; // le champ a son propre geste
      e.preventDefault();
      avancer();
      return;
    }
    if (e.key === "Escape") { reculer(); return; }
    if (!champ && ["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].includes(e.key)) {
      const groupe = D.ecran.querySelector('[role="radiogroup"][data-principal]') ||
                     D.ecran.querySelector('[role="radiogroup"]');
      if (!groupe) return;
      const tuiles = [...groupe.querySelectorAll('[role="radio"]')];
      const i = tuiles.findIndex((t) => t.getAttribute("aria-checked") === "true");
      const pas = e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : -1;
      const suivante = tuiles[Math.min(tuiles.length - 1, Math.max(0, i + pas))];
      if (suivante) { e.preventDefault(); suivante.click(); suivante.focus(); }
      return;
    }
    // Sur l'écran du nom, une lettre tapée ailleurs va dans le champ : la
    // personne ne doit jamais taper dans le vide.
    if (cle === "compte" && !champ && e.key.length === 1) {
      const f = document.getElementById("nom");
      if (f) f.focus();
    }
  });

  // ------------------------------------------------------- briques d'écran
  function entete(etiquette, titre, sous) {
    return el("header.entete",
      el("div.etiquette", etiquette),
      el("h1.titre", titre),
      sous ? el("p.sous", sous) : null);
  }

  // Un groupe de tuiles, une seule cochée. `rappel` après chaque choix.
  function groupe(cleR, options, { principal = true, colonnes = "", rappel } = {}) {
    const g = el(`div.choix${colonnes ? "." + colonnes : ""}`, { role: "radiogroup", "data-principal": principal || null });
    for (const o of options) {
      const t = el("button.tuile", {
        role: "radio", type: "button", "aria-checked": String(R[cleR] === o.valeur),
        onclick: () => {
          R[cleR] = o.valeur;
          g.querySelectorAll('[role="radio"]').forEach((x) => x.setAttribute("aria-checked", String(x === t)));
          if (rappel) rappel(o.valeur);
        },
      },
      o.dessin || (o.icone ? el("span.pastille", svg(o.icone)) : null),
      el("span", el("b", o.titre), o.note ? el("small", o.note) : null),
      el("span.coche", svg("coche")));
      g.append(t);
    }
    return g;
  }

  // ---------------------------------------------------------------- écrans
  const SALUTS = ["Welcome", "Bienvenue", "Bienvenido", "Willkommen", "Benvenuto", "Bem-vindo", "مرحبا", "Welkom", "ようこそ", "Witaj"];
  ecrans.bienvenue = () => {
    // La seule page en anglais : quelqu'un qui ne lit pas le français doit
    // pouvoir atteindre l'écran où l'on choisit la langue.
    const salut = el("span.salut.degrade", SALUTS[0]);
    let i = 0;
    const minuterie = setInterval(() => {
      if (cle !== "bienvenue") return clearInterval(minuterie);
      i = (i + 1) % SALUTS.length;
      salut.classList.remove("change"); void salut.offsetWidth; salut.classList.add("change");
      salut.textContent = SALUTS[i];
    }, 1800);
    return el("div.accueil",
      el("div.embleme", el("div.anneau"), el("div.anneau.a2"), el("div.logo.geant", svg("marque"))),
      el("h1.titre.enorme", salut),
      el("p.sous.centre", "to ", el("b", "grenOS"), " — let's set up your computer in a few steps."),
      el("div.puces-accueil",
        el("span.badge", svg("eclair"), "Fast"), el("span.badge", svg("cadenas"), "Private by default"),
        el("span.badge", svg("palette"), "Yours to customize")));
  };

  ecrans.langue = () => [
    entete("Étape de base", "Langue et région", "Elle décide de la langue des menus, du format des dates et de l'heure."),
    groupe("langue", [
      { valeur: "fr_FR", titre: "Français", note: "France — 17/11/2026, 14:30", dessin: el("span.drapeau", "FR") },
      { valeur: "en_US", titre: "English", note: "United States — 11/17/2026, 2:30 PM", dessin: el("span.drapeau", "EN") },
    ]),
    el("p.note", "Deux langues pour l'instant, et on le dit plutôt que d'en promettre cent."),
  ];

  ecrans.clavier = () => {
    const recherche = el("input.champ", { placeholder: "Rechercher un pays…", "data-entree": "1", type: "search" });
    const g = el("div.choix.claviers", { role: "radiogroup", "data-principal": true });
    const essai = el("input.champ.essai", { placeholder: "Tape ici pour essayer : é à ç ù @ 1 2 3", "data-entree": "1" });
    const remplir = () => {
      const q = recherche.value.trim().toLowerCase();
      g.replaceChildren(...infos.donnees.claviers.map((c, i) => [c, i])
        .filter(([c]) => !q || c.nom.toLowerCase().includes(q))
        .map(([c, i]) => {
          const t = el("button.tuile.compacte", {
            role: "radio", type: "button", "aria-checked": String(R.clavier_index === String(i)),
            onclick: () => {
              R.clavier_index = String(i); R.clavier = c.disposition;
              g.querySelectorAll('[role="radio"]').forEach((x) => x.setAttribute("aria-checked", String(x === t)));
              api.appeler("premier_clavier", { index: i });
            },
          }, el("span.touche", c.disposition.toUpperCase()), el("span", el("b", c.nom), el("small", c.fuseau.replace("_", " "))), el("span.coche", svg("coche")));
          return t;
        }));
    };
    recherche.addEventListener("input", remplir);
    recherche.addEventListener("keydown", (e) => { if (e.key === "Enter") { const p = g.querySelector('[role="radio"]'); if (p) p.click(); essai.focus(); } });
    essai.addEventListener("keydown", (e) => { if (e.key === "Enter") avancer(); });
    remplir();
    return [entete("Étape de base", "Clavier", "Choisis ta disposition — elle change tout de suite. Essaie-la : ce que tu tapes doit correspondre."),
      el("div.deux", el("div.pile", recherche, el("div.defile", g)), el("div.pile", el("div.carte.verre.clavier-essai", el("div.etiquette", "Zone d'essai"), essai, el("div.mini-clavier", ..."AZERTYUIOP".split("").map((l) => el("kbd", l))))))];
  };

  ecrans.compte = () => {
    const initiale = el("div.avatar", (R.nom_affiche || "?").trim().charAt(0).toUpperCase() || "?");
    const adresse = el("div.adresse.mono", R.compte ? `${R.compte}@${infos.machine}` : "ton-nom@" + infos.machine);
    const champ = el("input.champ.grand-champ", { id: "nom", placeholder: "Ton nom", autocomplete: "off", spellcheck: "false", value: R.nom_affiche, "data-entree": "1" });
    let jeton = 0;
    champ.addEventListener("input", async () => {
      R.nom_affiche = champ.value;
      initiale.textContent = champ.value.trim().charAt(0).toUpperCase() || "?";
      const moi = ++jeton;
      const r = await api.appeler("premier_nom", { texte: champ.value });
      if (moi !== jeton) return;
      adresse.textContent = r.adresse || "ton-nom@" + infos.machine;
      adresse.classList.toggle("vide", !r.adresse);
      R.compte = r.probleme ? "" : r.login;
      D.erreur.textContent = r.probleme && champ.value.trim() ? r.probleme : "";
    });
    champ.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); avancer(); } });

    const champs = el("div.mdp", { hidden: !mdp.actif },
      el("input.champ", { type: "password", placeholder: "Mot de passe", value: mdp.mot, "data-entree": "1", oninput: (e) => { mdp.mot = e.target.value; } }),
      el("input.champ", { type: "password", placeholder: "Confirmer", value: mdp.confirme, "data-entree": "1", oninput: (e) => { mdp.confirme = e.target.value; } }));
    const bascule = el("button.interrupteur", { role: "switch", "aria-checked": String(mdp.actif), type: "button",
      onclick: () => { mdp.actif = !mdp.actif; bascule.setAttribute("aria-checked", String(mdp.actif)); champs.hidden = !mdp.actif; } });

    setTimeout(() => champ.focus(), 60);
    return [entete("Étape de base", "Votre compte", "Ton nom devient ton compte, ta maison et ton adresse sur cette machine."),
      el("div.carte.verre.compte",
        el("div.identite", initiale, el("div", el("div.etiquette", "Ton adresse"), adresse)),
        champ,
        el("div.ligne-option", el("div", el("b", "Protéger par un mot de passe"), el("small.note", " — facultatif. Sans lui, la session s'ouvre directement.")), bascule),
        champs)];
  };

  ecrans.parcours = () => [
    entete("Ton rythme", "Comment veux-tu continuer ?", "Tu pourras tout changer plus tard dans les Réglages."),
    groupe("parcours", [
      { valeur: "rapide", icone: "eclair", titre: "Rapide", note: "Le moins d'écrans possible. On démarre tout de suite." },
      { valeur: "personnalise", icone: "reglages", titre: "Personnalisé (recommandé)", note: "Réseau, apparence, confidentialité, sécurité, mises à jour, applications." },
      { valeur: "dev", icone: "terminal", titre: "Dev", note: "Tout cela, plus les réglages avancés du système à mesure qu'ils arrivent." },
    ], { colonnes: "trois", rappel: majRail }),
  ];

  ecrans.reseau = () => {
    const etat = el("div.carte.verre.etat-reseau", el("span.badge", "Recherche…"));
    const listeR = el("div.choix.liste.reseaux");
    const rafraichir = async () => {
      const e = await api.appeler("reseau_etat");
      if (cle !== "reseau") return;
      D.suite.querySelector("span").textContent = e.connecte ? "Continuer" : "Continuer hors connexion";
      etat.replaceChildren(
        el("span.pastille", svg("wifi")),
        el("div", el("b", e.connecte ? "Connecté" : e.gestionnaire ? "Pas encore connecté" : "Réseau indisponible"),
          el("small.note", e.connecte ? ` — ${e.nom || "réseau filaire"}` : e.wifi ? " — choisis un réseau ci-dessous" : " — aucune carte Wi-Fi détectée ; un câble suffit")),
        el("span.badge" + (e.connecte ? ".ok" : ".alerte"), e.connecte ? "En ligne" : "Hors ligne"));
      if (!e.wifi) { listeR.replaceChildren(); return; }
      listeR.replaceChildren(el("div.note", "Recherche des réseaux…"));
      const reseaux = await api.appeler("reseau_liste");
      if (cle !== "reseau") return;
      listeR.replaceChildren(...reseaux.map((r) => tuileReseau(r, rafraichir)));
      if (!reseaux.length) listeR.replaceChildren(el("div.note", "Aucun réseau Wi-Fi à portée."));
    };
    rafraichir();
    return [entete("Connexion", "Connexion à Internet", "Pour l'heure exacte, les mises à jour et les applications. Tu peux aussi continuer hors connexion."),
      etat, el("div.defile.haut", listeR),
      el("button.bouton.discret", { onclick: rafraichir, type: "button" }, svg("maj"), "Actualiser")];
  };

  function tuileReseau(r, apres) {
    const barres = el("span.signal", ...[25, 50, 75].map((s) => el("i", { class: r.signal >= s ? "plein" : "" })), el("i", { class: r.signal >= 90 ? "plein" : "" }));
    const form = el("div.connexion", { hidden: true });
    const champ = el("input.champ", { type: "password", placeholder: "Mot de passe du réseau", "data-entree": "1" });
    const msg = el("div.erreur");
    const connecter = async () => {
      msg.textContent = "Connexion…";
      try { await api.appeler("reseau_connecter", { nom: r.nom, motDePasse: champ.value || null }); msg.textContent = ""; apres(); }
      catch (e) { msg.textContent = String(e); }
    };
    champ.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); connecter(); } });
    if (r.protege) form.append(champ);
    form.append(el("button.bouton.principal", { onclick: connecter, type: "button" }, "Se connecter"), msg);
    return el("div.tuile.reseau", { "aria-checked": String(r.actif) },
      el("div.rangee", { onclick: () => { form.hidden = !form.hidden; if (!form.hidden && r.protege) champ.focus(); } },
        barres, el("b", r.nom), r.protege ? el("span.cadenas", svg("cadenas")) : null,
        r.actif ? el("span.badge.ok", "Connecté") : null),
      form);
  }

  function apercuBureau(mode) {
    return el(`span.apercu-bureau.${mode}`,
      el("span.ap-fond"), el("span.ap-fenetre", el("i"), el("i"), el("i")), el("span.ap-barre", el("i"), el("i"), el("i"), el("i")));
  }

  ecrans.apparence = () => {
    const appliquer = () => habiller({ theme: R.apparence, accent: R.accent, effets: R.effets });
    const accents = el("div.accents", { role: "radiogroup" });
    const COULEURS = { bleu: "#3b82f6", violet: "#8b5cf6", vert: "#10b981", orange: "#f59e0b", rose: "#ec4899", cyan: "#06b6d4" };
    for (const [nom, c] of Object.entries(COULEURS)) {
      const p = el("button.accent-choix", { role: "radio", type: "button", title: nom, "aria-checked": String(R.accent === nom),
        style: `background: linear-gradient(135deg, ${c}, color-mix(in srgb, ${c} 50%, #8b5cf6))`,
        onclick: () => { R.accent = nom; accents.querySelectorAll("button").forEach((x) => x.setAttribute("aria-checked", String(x === p))); appliquer(); } });
      accents.append(p);
    }
    return [entete("Personnalisation", "Apparence", "Ce que tu choisis s'applique ici même, tout de suite. Tout se change plus tard dans Réglages → Apparence."),
      groupe("apparence", [
        { valeur: "auto", titre: "Automatique", note: "Sombre le soir, clair le jour.", dessin: apercuBureau("auto") },
        { valeur: "nuit", titre: "Sombre", note: "Le thème de grenOS, reposant pour les yeux.", dessin: apercuBureau("nuit") },
        { valeur: "jour", titre: "Clair", note: "Fond clair, texte sombre.", dessin: apercuBureau("jour") },
      ], { colonnes: "trois.apparences", rappel: appliquer }),
      el("div.deux.serre",
        el("div.carte.verre", el("div.etiquette", "Couleur d'accent"), accents),
        el("div.carte.verre", el("div.etiquette", "Effets visuels"),
          groupe("effets", [
            { valeur: "complets", titre: "Complets", note: "Flou, fond animé, toutes les transitions." },
            { valeur: "equilibres", titre: "Équilibrés", note: "Animations douces, sans flou." },
            { valeur: "performance", titre: "Performance max", note: "Aucune animation : chaque cycle va à tes applications." },
          ], { principal: false, colonnes: "liste.compact", rappel: appliquer }),
          lent ? el("p.note", "Cette machine dessine lentement : « Performance max » a été choisi pour toi.") : null)),
    ];
  };

  ecrans.usage = () => {
    const icones = { bureautique: "bureau", developpement: "code", jeu: "manette", montage: "film" };
    return [entete("Tes applications", "À quoi va servir cet ordinateur ?", "Les applications du profil choisi s'installeront au prochain démarrage, avant le bureau. Rien n'est définitif : GrenPlace sait en ajouter et en retirer."),
      groupe("usage", infos.donnees.usages.map((u) => ({ valeur: u.cle, icone: icones[u.cle] || "paquet", titre: u.titre, note: u.note })), { colonnes: "deux-col" })];
  };

  ecrans.confidentialite = () => [
    entete("Confidentialité", "Vos données vous appartiennent", "grenOS n'envoie rien que tu n'aies choisi d'envoyer. Aucune télémétrie n'est activée par défaut."),
    groupe("telemetrie", [
      { valeur: "aucune", icone: "cadenas", titre: "Aucune (par défaut)", note: "Rien ne quitte cette machine." },
      { valeur: "minimale", icone: "oeil", titre: "Minimale et anonyme", note: "La version de grenOS et le type de matériel, sans identifiant." },
      { valeur: "complete", icone: "bouclier", titre: "Complète", note: "En plus : les pannes rencontrées, pour les corriger." },
    ], { colonnes: "trois" }),
  ];

  ecrans.securite = () => {
    // L'état, et AUCUN interrupteur : « il ne faut pas que ça se retourne
    // contre nous ». Une case sans effet serait la faute qu'on chasse ici.
    const sb = infos.secure_boot;
    const lignes = [
      ["bouclier", "Secure Boot", sb, sb === "actif" ? "alerte" : ""],
      ["maj", "Mises à jour automatiques", R.majs === "automatiques" ? "activées" : "au choix", R.majs === "automatiques" ? "ok" : ""],
      ["oeil", "Antivirus", "ClamAV, installé", "ok"],
      ["cadenas", "Chiffrement du disque", "se décide à l'installation sur disque", ""],
    ];
    return [entete("Sécurité", "Protégeons votre ordinateur", infos.secure_boot_phrase),
      el("div.carte.verre.lignes", ...lignes.map(([i, n, v, genre]) =>
        el("div.ligne", el("span.icone", svg(i)), el("div.nom", n), el("span.valeur", el(`span.badge${genre ? "." + genre : ""}`, v)))))];
  };

  ecrans.majs = () => [
    entete("Mises à jour", "Garder grenOS à jour", "Les correctifs de sécurité comptent plus que tout le reste : c'est par là qu'une machine se fait prendre."),
    groupe("majs", [
      { valeur: "automatiques", icone: "eclair", titre: "Automatiques (recommandé)", note: "Téléchargées en fond, installées au redémarrage avant le bureau. Les applications se mettent à jour seules." },
      { valeur: "demander", icone: "oeil", titre: "Me demander", note: "grenOS télécharge et prévient ; rien ne s'installe sans ton accord." },
      { valeur: "manuelles", icone: "reglages", titre: "Manuelles", note: "grenOS compte les mises à jour, et c'est tout." },
    ], { colonnes: "trois" }),
    el("p.note", "Jamais pendant ta session : une installation interrompue en plein travail est la façon classique de casser un système."),
  ];

  ecrans.applications = () => {
    // Proposées, pas imposées : aucune case n'est cochée au départ.
    const choisies = () => new Set(R.applications.split(",").filter(Boolean));
    const options = [
      ["steam", "manette", "Steam", "Des milliers de jeux, dont beaucoup de jeux Windows."],
      ["vscode", "code", "Visual Studio Code", "Un éditeur de code, si cette machine sert à développer."],
      ["libreoffice", "bureau", "LibreOffice", "Traitement de texte, tableur, présentations."],
    ];
    return [entete("Tes applications", "Applications proposées", "Le navigateur, l'explorateur et le lecteur multimédia sont déjà là. Voici ce qu'on peut ajouter — rien n'est coché."),
      el("div.choix.trois", ...options.map(([c, i, t, n]) => {
        const tuile = el("button.tuile", { role: "checkbox", type: "button", "aria-checked": String(choisies().has(c)),
          onclick: () => {
            const s = choisies(); s.has(c) ? s.delete(c) : s.add(c);
            R.applications = [...s].sort().join(",");
            tuile.setAttribute("aria-checked", String(s.has(c)));
          } }, el("span.pastille", svg(i)), el("span", el("b", t), el("small", n)), el("span.coche", svg("coche")));
        return tuile;
      }))];
  };

  // I13 — les sauvegardes. Une vraie promesse derrière chaque tuile :
  // grenos-sauvegarde (rsync versionné, vérifié en relisant un fichier).
  ecrans.sauvegardes = () => [
    entete("Sauvegardes", "Garder une copie de tes fichiers", "Documents, Images, Musique, Vidéos et Bureau. Une copie par jour, les anciennes gardées, et chaque copie relue pour vérifier qu'elle se restaure."),
    groupe("sauvegardes", [
      { valeur: "externe", icone: "paquet", titre: "Sur un disque externe", note: "Chaque jour, dès qu'un disque USB est branché. Rien à régler : grenOS trouve le disque." },
      { valeur: "non", icone: "retour", titre: "Pas maintenant", note: "Tu pourras l'activer plus tard avec « gren sauvegarde activer »." },
    ], { colonnes: "deux-col" }),
    el("p.note", "Le réseau (NAS, serveur) et le nuage arrivent ; aujourd'hui grenOS ne propose que ce qu'il sait vraiment faire."),
  ];

  // I17 — le parcours Dev. Chaque choix est appliqué pour de vrai par
  // premier.rs ; ce qui ne peut pas l'être (partitionnement, Secure Boot,
  // bac à sable) reste hors de la liste plutôt que d'y faire semblant.
  ecrans.systeme = () => [
    entete("Système", "Mémoire d'échange", "Ce que la machine fait quand la mémoire vive se remplit."),
    groupe("echange", [
      { valeur: "zram", icone: "eclair", titre: "zram (recommandé)", note: "Échange en mémoire compressée : dix fois plus rapide qu'un disque." },
      { valeur: "aucun", icone: "retour", titre: "Pas de zram", note: "Seulement la mémoire d'échange sur disque, s'il y en a une." },
    ], { colonnes: "deux-col" }),
    el("div.etiquette", "Quand puiser dans l'échange"),
    groupe("swappiness", [
      { valeur: "reactif", icone: "eclair", titre: "Le plus tard possible", note: "vm.swappiness = 10 : les applications restent en mémoire vive." },
      { valeur: "equilibre", icone: "reglages", titre: "Équilibré", note: "vm.swappiness = 60, le réglage de Debian." },
    ], { colonnes: "deux-col", principal: false }),
  ];

  ecrans["pare-feu"] = () => [
    entete("Réseau", "Pare-feu", "ufw : les connexions sortantes passent, les entrantes sont refusées sauf celles que tu ouvres."),
    groupe("parefeu", [
      { valeur: "actif", icone: "bouclier", titre: "Activé (recommandé)", note: "Refuser tout ce qui arrive sans y avoir été invité. « sudo ufw allow 22 » ouvre un port." },
      { valeur: "non", icone: "retour", titre: "Désactivé", note: "Tout ce qui écoute sur la machine est joignable depuis le réseau." },
    ], { colonnes: "deux-col" }),
  ];

  ecrans.journaux = () => [
    entete("Journaux", "Combien de temps garder les journaux", "Les journaux du système restent sur la machine — jamais envoyés. Ils servent à comprendre une panne après coup."),
    groupe("journaux", [
      { valeur: "semaine", icone: "sablier", titre: "Une semaine", note: "50 Mo au plus." },
      { valeur: "mois", icone: "sablier", titre: "Un mois (recommandé)", note: "100 Mo au plus." },
      { valeur: "trimestre", icone: "sablier", titre: "Trois mois", note: "500 Mo au plus." },
    ], { colonnes: "trois" }),
  ];

  ecrans.optimisation = () => {
    const bascule = el("button.interrupteur", { role: "switch", type: "button", "aria-checked": String(R.optimiser === "oui"),
      onclick: () => { R.optimiser = R.optimiser === "oui" ? "non" : "oui"; bascule.setAttribute("aria-checked", String(R.optimiser === "oui")); } });
    return [entete("Matériel", "Optimisation automatique", infos.virtuelle ? `Voici ce que grenOS a trouvé — c'est une machine virtuelle (${infos.virtuelle}).` : "Voici ce que grenOS a trouvé sur cette machine."),
      el("div.carte.verre.lignes.materiel", ...infos.materiel.map((m) =>
        el("div.ligne", el("span.icone", svg(m.icone)), el("div", el("div.nom", m.nom), m.detail ? el("div.detail", m.detail) : null), el("span.valeur", m.valeur)))),
      el("div.carte.verre.ligne-option", el("div", el("b", "Régler la machine pour la performance"), el("small.note", " — au prochain démarrage, pas pendant que tu réponds.")), bascule)];
  };

  ecrans.final = () => {
    const c = infos.donnees.claviers[Number(R.clavier_index)] || infos.donnees.claviers[0];
    const usage = (infos.donnees.usages.find((u) => u.cle === R.usage) || {}).titre || R.usage;
    const MOTS = { auto: "Automatique", nuit: "Sombre", jour: "Clair", aucune: "Aucune", minimale: "Minimale", complete: "Complète",
      automatiques: "Automatiques", demander: "Me demander", manuelles: "Manuelles", rapide: "Rapide", personnalise: "Personnalisé", dev: "Dev",
      complets: "Complets", equilibres: "Équilibrés", performance: "Performance max" };
    const lignes = [
      ["compte", "Compte", `${R.compte}@${infos.machine}${mdp.actif && mdp.mot ? " · mot de passe" : ""}`],
      ["clavier", "Clavier", c.nom],
      ["palette", "Apparence", `${MOTS[R.apparence]} · ${R.accent} · effets ${MOTS[R.effets].toLowerCase()}`],
      ["bureau", "Usage", usage],
      ["oeil", "Télémétrie", MOTS[R.telemetrie]],
      ["maj", "Mises à jour", MOTS[R.majs]],
      ["paquet", "À installer", R.applications ? R.applications.split(",").join(", ") : "rien de plus"],
      ["paquet", "Sauvegardes", R.sauvegardes === "externe" ? "Chaque jour, disque externe" : "Pas maintenant"],
    ];
    if (R.parcours === "dev") {
      lignes.push(["bouclier", "Pare-feu", R.parefeu === "actif" ? "Activé" : "Désactivé"]);
      lignes.push(["reglages", "Échange", `${R.echange === "zram" ? "zram" : "sans zram"} · ${R.swappiness === "reactif" ? "swappiness 10" : "swappiness 60"}`]);
      lignes.push(["oeil", "Journaux", { semaine: "Une semaine", mois: "Un mois", trimestre: "Trois mois" }[R.journaux]]);
    }
    return [entete("Dernière étape", "Tout est prêt", "Vérifie, puis démarre. grenOS applique tes choix et ouvre ton bureau."),
      el("div.resume", ...lignes.map(([i, n, v]) => el("div.carte.verre.case-resume", el("span.icone", svg(i)), el("div.etiquette", n), el("div.valeur-resume", v))))];
  };

  // -------------------------------------------------------------- la fin
  async function terminer() {
    fini = true;
    D.suite.disabled = true;
    D.retour.disabled = true;
    const gestes = ["Création de ton compte", "Clavier et fuseau horaire", "Apparence", "Mises à jour", "Applications au prochain démarrage"];
    const items = gestes.map((g) => el("li", el("span.puce", svg("coche")), g));
    const voile = el("div.voile",
      el("div.fin",
        el("div.embleme", el("div.anneau.tourne"), el("div.logo.geant", svg("marque"))),
        el("h1.titre", "Mise en place…"),
        el("ol.gestes", ...items)));
    document.body.append(voile);
    let i = 0;
    const pas = setInterval(() => { if (i < items.length - 1) items[i++].classList.add("fait"); }, 420);
    let adresse = "";
    try {
      // Le premier arrivé de deux chemins : la réponse de la commande, ou
      // l'événement « premier-fini » (voir premier.rs, panne du 18 novembre).
      const parEvenement = new Promise((ok) => api.ecouter("premier-fini", ok));
      adresse = await Promise.race([
        api.appeler("premier_terminer", { reponses: R, motDePasse: mdp.actif && mdp.mot ? mdp.mot : null }),
        parEvenement,
      ]);
    } catch (e) {
      clearInterval(pas);
      voile.remove();
      fini = false;
      D.suite.disabled = false;
      D.retour.disabled = false;
      aller("compte");
      D.erreur.textContent = String(e);
      return;
    }
    clearInterval(pas);
    items.forEach((x) => x.classList.add("fait"));
    const prenom = (R.nom_affiche || R.compte).trim().split(/\s+/)[0];
    voile.querySelector(".fin").replaceChildren(
      el("div.embleme.pret", el("div.anneau"), el("div.logo.geant", svg("coche"))),
      el("h1.titre.enorme", el("span.degrade", "grenOS est prêt")),
      el("p.sous.centre", `Bienvenue chez toi, ${prenom}. `, el("span.mono", adresse)));
    // I15 : la ligne vient de la PAGE, une fois l'écran dessiné — la preuve
    // qu'on l'a vu, pas seulement que Rust a fini d'écrire.
    requestAnimationFrame(() => api.appeler("premier_dire", { texte: "premier : ecran grenOS est pret affiche" }).catch(() => {}));
    if (document.documentElement.dataset.effets !== "performance") confettis(voile);
    setTimeout(() => api.appeler("premier_quitter"), 2600);
  }

  function confettis(parent) {
    const toile = el("canvas.confettis");
    parent.append(toile);
    const ctx = toile.getContext("2d");
    toile.width = innerWidth; toile.height = innerHeight;
    const style = getComputedStyle(document.documentElement);
    const couleurs = [style.getPropertyValue("--accent"), style.getPropertyValue("--accent-2"), style.getPropertyValue("--accent-clair"), "#ffffff"];
    const bouts = Array.from({ length: 90 }, () => ({
      x: innerWidth / 2, y: innerHeight / 2.4, vx: (Math.random() - .5) * 16, vy: -Math.random() * 14 - 4,
      t: Math.random() * 6 + 4, c: couleurs[Math.floor(Math.random() * couleurs.length)], r: Math.random() * 6,
    }));
    const debut = performance.now();
    (function image() {
      ctx.clearRect(0, 0, toile.width, toile.height);
      for (const b of bouts) {
        b.vy += .35; b.x += b.vx; b.y += b.vy; b.vx *= .99; b.r += .1;
        ctx.save(); ctx.translate(b.x, b.y); ctx.rotate(b.r); ctx.fillStyle = b.c; ctx.fillRect(-b.t / 2, -b.t / 4, b.t, b.t / 2); ctx.restore();
      }
      if (performance.now() - debut < 2400) requestAnimationFrame(image);
    })();
  }

  // ------------------------------------------------------------ démarrage
  async function demarrer() {
    habiller({ theme: "nuit", accent: R.accent, effets: R.effets });
    dessinerCadre();
    infos = await api.appeler("premier_infos");
    aller("bienvenue");
    api.appeler("premier_pret");
    // Une seconde d'images : moins de 30, la machine dessine au processeur et
    // les effets lui coûteraient sa fluidité. On propose alors « Performance ».
    const images = await mesurerFluidite();
    if (images < 30) {
      lent = true;
      R.effets = "performance";
      habiller({ theme: "nuit", accent: R.accent, effets: R.effets });
      api.appeler("premier_dire", { texte: `premier : ${images} images par seconde, effets reduits` });
    }
  }
  demarrer();
})();
