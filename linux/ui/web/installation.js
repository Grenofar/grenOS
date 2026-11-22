// Installer grenOS (22 novembre), toutes ses pages : Bienvenue, Clavier et
// région, Disque, Compte, Résumé, Installation, Terminé. Grenofar : « refais
// le système de partition, des fois c'est pas cliquable, il fallait attendre
// avant de pouvoir cliquer — supprimer une partition, créer pour grenOS, et
// une taille en Gio qu'on change », puis « l'interface, mets-la pour toutes
// les pages de l'installation ».
//
// Les disques sont lus UNE fois, pendant la page de bienvenue. Ensuite chaque
// clic n'est qu'un calcul, ici, en mémoire : rien à attendre, et rien n'est
// écrit avant « Installer », au résumé. Le moteur (grenos-installation) refait
// le même calcul avant d'écrire ; linux/tools/essai-installation-ecran.py
// exige les mêmes nombres des deux.
"use strict";
(function () {
  // ------------------------------------------------------------- le calcul
  function trous(premier, fin, partitions, minimum) {
    const libres = [];
    let position = premier;
    for (const p of [...partitions].sort((a, b) => a.debut_mio - b.debut_mio)) {
      if (p.debut_mio - position >= minimum) libres.push({ debut_mio: position, taille_mio: p.debut_mio - position });
      position = Math.max(position, p.fin_mio);
    }
    if (fin - position >= minimum) libres.push({ debut_mio: position, taille_mio: fin - position });
    return libres;
  }

  // Ce que donnent les clics faits jusqu'ici, et le plan qui en sort.
  // `point` : un Mio dans l'espace où créer grenOS ; `taille` 0 : tout l'espace.
  function calculer(disque, choix, regles) {
    const effacer = !!choix.effacer && disque.etat !== "occupe";
    const modifiable = disque.etat === "gpt" || disque.etat === "vide";
    const partent = effacer ? disque.partitions : modifiable ? disque.partitions.filter((p) => choix.supprimer.includes(p.dev)) : [];
    const restent = disque.partitions.filter((p) => !partent.includes(p));
    const efi = effacer ? null : restent.find((p) => p.efi_utilisable) || null;
    const efiNeuve = efi ? 0 : regles.efi_mio;
    let bruts = [];
    if (effacer) bruts = [{ debut_mio: 1, taille_mio: disque.taille_mio - 2 }];
    else if (modifiable) bruts = trous(disque.premier_mio, disque.fin_mio, restent, regles.trou_minimum_mio);
    const zones = bruts.map((z) => {
      const maximum = z.taille_mio - efiNeuve;
      return { ...z, maximum, possible: maximum >= regles.minimum_mio };
    });
    let creation = null;
    if (choix.point != null) {
      const zone = zones.find((z) => z.possible && choix.point >= z.debut_mio && choix.point < z.debut_mio + z.taille_mio);
      if (zone) {
        const taille = choix.taille ? Math.max(regles.minimum_mio, Math.min(choix.taille, zone.maximum)) : zone.maximum;
        creation = { zone, efi, efi_neuve: efiNeuve, debut_mio: zone.debut_mio + efiNeuve, taille, reste: zone.maximum - taille };
      }
    }
    const plan = creation && {
      disque: disque.dev,
      empreinte: disque.empreinte,
      effacer,
      supprimer: effacer ? [] : partent.map((p) => p.dev),
      zone_mio: effacer ? null : creation.zone.debut_mio,
      taille_mio: creation.taille === creation.zone.maximum ? 0 : creation.taille,
    };
    return { effacer, partent, restent, zones, creation, plan };
  }

  // Le disque montré d'abord : un disque vide, sinon un disque sans Windows
  // ni fichiers — JAMAIS celui de Windows : il se choisit d'un clic voulu.
  function aGarder(disque) {
    return disque.partitions.some((p) => ["windows", "microsoft", "donnees", "autre"].includes(p.genre));
  }
  function parDefaut(disques) {
    const sansRien = disques.find((d) => d.etat === "vide");
    const sansWindows = disques.find((d) => d.etat === "gpt" && !aGarder(d));
    return (sansRien || sansWindows || {}).dev || null;
  }

  if (typeof module !== "undefined" && module.exports) {
    module.exports = { trous, calculer, parDefaut };
    return;
  }

  // ------------------------------------------------------------ les pages
  const { api, el, svg, habiller, aurore, pret, toast } = window.grenos;

  const ETAPES = [
    ["bienvenue", "Bienvenue", "etoile"],
    ["clavier", "Clavier et région", "clavier"],
    ["disque", "Disque", "disque"],
    ["compte", "Compte", "compte"],
    ["resume", "Résumé", "bouclier"],
    ["installation", "Installation", "paquet"],
    ["termine", "Terminé", "coche"],
  ];
  const SOUS = {
    bienvenue: "Quelques pages, puis c'est installé.",
    clavier: "Le clavier change tout de suite : essaie-le.",
    disque: "Supprime, crée, choisis la taille. Rien n'est écrit avant le résumé.",
    compte: "Choisi ici, il ne sera pas redemandé au premier démarrage.",
    resume: "Relis : l'installation commence au prochain clic.",
    installation: "grenOS se copie sur le disque.",
    termine: "Il ne reste qu'à redémarrer.",
  };

  const D = {
    etape: "bienvenue",
    infos: null,
    // les disques
    lecture: "charge", regles: null, disques: [], choisi: null, choix: new Map(), souciDisques: "",
    // clavier et région
    clavier: 0, fuseau: "", fuseauVoulu: false, cherche: "",
    // le compte
    nom: "", login: "", protege: false, mot: "", confirme: "",
    // la fin
    compris: false, resumeDepuis: 0, avance: { part: 0, texte: "" }, echec: "",
  };
  const N = {};

  function gio(mio) {
    if (mio < 1024) return `${Math.round(mio)} Mio`;
    return `${(Math.floor(mio / 102.4) / 10).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} Gio`;
  }

  const rangDe = (etape) => ETAPES.findIndex(([cle]) => cle === etape);
  const enPreparation = () => rangDe(D.etape) <= rangDe("resume");
  const disqueChoisi = () => D.disques.find((d) => d.dev === D.choisi) || null;
  const clavierChoisi = () => D.infos.claviers[D.clavier] || D.infos.claviers[0];

  function choixDe(disque) {
    if (!D.choix.has(disque.dev)) D.choix.set(disque.dev, { effacer: false, supprimer: [], point: null, taille: 0 });
    return D.choix.get(disque.dev);
  }

  const calculDe = (disque) => calculer(disque, choixDe(disque), D.regles);

  function changer(disque, geste) {
    geste(choixDe(disque));
    D.compris = false;
    dessiner();
  }

  // ---- des briques
  function message(titre, texte, ...plus) {
    return el("div.in-centre", el("h2", titre), el("p.in-etape", texte), plus);
  }
  function attente(texte) {
    return el("div.in-centre", el("div.in-roue"), el("p.in-etape", texte));
  }

  // ---------------------------------------------------------- 1. bienvenue
  function pageBienvenue() {
    if (!D.infos) return { corps: attente("Préparation…"), suite: null };
    if (!D.infos.cle) {
      return { corps: message("grenOS s'installe depuis sa clé", "Démarre le PC sur la clé de grenOS, puis rouvre « Installer grenOS »."), suite: null };
    }
    const point = (icone, titre, texte) =>
      el("div.carte.in-point-cle", el("span.in-pastille", svg(icone)), el("div", el("b", titre), el("p", texte)));
    const avis = D.infos.efi ? null
      : el("p.in-avis", "Ce PC a démarré en mode BIOS. Redémarre sur la clé en choisissant l'entrée « UEFI ».");
    return {
      corps: el("div.in-defile.in-accueil",
        el("div.in-grand-logo", svg("marque")),
        el("h2.in-bonjour", "Bienvenue dans ", el("span.degrade", "grenOS")),
        el("p.in-etape", "On choisit le clavier, le disque et ton compte. Tu relis, puis grenOS s'installe."),
        [avis],
        el("div.in-points",
          point("disque", "Rien n'est écrit avant le résumé", "Tu peux revenir en arrière à chaque page, sans rien perdre."),
          point("cadenas", "Secure Boot", D.infos.secure_boot_phrase),
          point("eclair", "Garde le PC branché", "Et laisse la clé en place jusqu'au redémarrage."))),
      suite: { texte: "Commencer", possible: D.infos.efi },
    };
  }

  // -------------------------------------------------- 2. clavier et région
  function pageClavier() {
    const recherche = el("input.champ", { type: "search", placeholder: "Rechercher un pays…", value: D.cherche, "data-entree": "1" });
    const liste = el("div.in-claviers", { role: "radiogroup", "aria-label": "Disposition du clavier" });
    const essai = el("input.champ", { placeholder: "Tape ici : é à ç ù @ 1 2 3", autocomplete: "off", spellcheck: "false" });
    const fuseau = el("select.champ", { "aria-label": "Fuseau horaire" },
      D.infos.fuseaux.map((f) => el("option", { value: f, selected: f === D.fuseau }, f.replace(/_/g, " "))));
    fuseau.addEventListener("change", () => { D.fuseau = fuseau.value; D.fuseauVoulu = true; });

    function remplir() {
      const q = D.cherche.trim().toLowerCase();
      const visibles = D.infos.claviers.map((c, i) => [c, i]).filter(([c]) => !q || c.nom.toLowerCase().includes(q));
      liste.replaceChildren(...visibles.map(([c, i]) => {
        const tuile = el("button.in-clavier", {
          role: "radio", "aria-checked": String(i === D.clavier),
          onclick: () => {
            D.clavier = i;
            if (!D.fuseauVoulu) { D.fuseau = c.fuseau; fuseau.value = c.fuseau; }
            for (const autre of liste.children) autre.setAttribute("aria-checked", String(autre === tuile));
            api.appeler("premier_clavier", { index: i });
          },
        }, el("kbd", c.disposition.toUpperCase()), el("b", c.nom), svg("coche"));
        return tuile;
      }));
      if (!visibles.length) liste.replaceChildren(el("p.note", "Aucun clavier ne porte ce nom."));
    }
    recherche.addEventListener("input", () => { D.cherche = recherche.value; remplir(); });
    remplir();

    return {
      corps: el("div.in-deux",
        el("section.in-pile.in-pile-liste", el("h3", "Clavier"), recherche, liste),
        el("section.in-pile",
          el("h3", "Essaie-le"), essai,
          el("h3", "Fuseau horaire"), fuseau,
          el("p.note", "Il suit le pays du clavier, sauf si tu en choisis un autre."),
          el("h3", "Langue"),
          el("p.note", "grenOS est en français."))),
      suite: { texte: "Continuer", possible: true },
    };
  }

  // ------------------------------------------------------------- 3. disque
  function segment(genre, mio, texte, clic) {
    const s = el(clic ? "button.in-segment" : "div.in-segment", { title: `${texte} — ${gio(mio)}`, "data-genre": genre },
      el("span", texte), el("small", gio(mio)));
    s.style.flexGrow = String(Math.max(mio, 1));
    if (clic) s.addEventListener("click", clic);
    return s;
  }

  function segments(disque, c) {
    const morceaux = c.restent.map((p) => ({ debut: p.debut_mio, n: segment(p.genre, p.taille_mio, p.nom) }));
    for (const z of c.zones) {
      if (c.creation && c.creation.zone === z) {
        const k = c.creation;
        if (k.efi_neuve) morceaux.push({ debut: z.debut_mio, n: segment("efi", k.efi_neuve, "Démarrage") });
        morceaux.push({ debut: k.debut_mio, n: segment("nouveau", k.taille, "grenOS") });
        if (k.reste >= 1) morceaux.push({ debut: k.debut_mio + k.taille, n: segment("libre", k.reste, "Espace libre") });
      } else {
        const clic = z.possible ? () => changer(disque, (x) => { x.point = z.debut_mio; x.taille = 0; }) : null;
        morceaux.push({ debut: z.debut_mio, n: segment("libre", z.taille_mio, "Espace libre", clic) });
      }
    }
    if (!morceaux.length) morceaux.push({ debut: 0, n: segment("autre", disque.taille_mio, "Données") });
    return morceaux.sort((a, b) => a.debut - b.debut).map((m) => m.n);
  }

  function lignePartition(disque, p, c) {
    const part = c.partent.includes(p);
    let geste = null;
    if (part && !c.effacer) {
      geste = el("button.bouton.petit", { onclick: () => changer(disque, (x) => { x.supprimer = x.supprimer.filter((d) => d !== p.dev); }) }, "Garder");
    } else if (!part && (disque.etat === "gpt" || disque.etat === "vide")) {
      geste = el("button.bouton.petit.danger", { onclick: () => changer(disque, (x) => { x.supprimer = [...x.supprimer, p.dev]; }) }, "Supprimer");
    }
    return el("div.in-ligne", { "data-part": part, "data-ligne": p.dev },
      el("i.in-point", { "data-genre": p.genre }),
      el("div.in-nom", el("b", p.nom), el("small", part ? "sera supprimée" : p.dev.replace("/dev/", ""))),
      el("span.in-taille", gio(p.taille_mio)),
      el("div.in-geste", [geste]));
  }

  function ligneLibre(disque, z) {
    const bouton = el("button.bouton.petit.principal", { disabled: !z.possible, onclick: () => changer(disque, (x) => { x.point = z.debut_mio; x.taille = 0; }) }, "Créer pour grenOS");
    const note = z.possible ? "rien dessus" : `trop petit : grenOS demande ${gio(D.regles.minimum_mio + z.taille_mio - z.maximum)}`;
    return el("div.in-ligne.in-libre",
      el("i.in-point", { "data-genre": "libre" }),
      el("div.in-nom", el("b", "Espace libre"), el("small", note)),
      el("span.in-taille", gio(z.taille_mio)),
      el("div.in-geste", bouton));
  }

  function ligneCreation(disque, c) {
    const k = c.creation;
    const bas = Math.ceil(D.regles.minimum_mio / 1024);
    const haut = Math.max(bas, Math.floor(k.zone.maximum / 1024));
    const valeur = k.reste === 0 ? haut : Math.min(haut, Math.round(k.taille / 1024));
    const glissiere = el("input.in-glissiere", { type: "range", min: bas, max: haut, step: 1, value: valeur, "aria-label": "Taille de grenOS en Gio", disabled: haut === bas });
    const champ = el("input.champ.in-nombre", { type: "number", min: bas, max: haut, step: 1, value: valeur, "aria-label": "Taille de grenOS en Gio" });
    const total = el("span.in-taille", gio(k.taille));
    const detail = el("small");

    function direDetail(x) {
      const bouts = [];
      bouts.push(x.efi_neuve ? `+ ${gio(x.efi_neuve)} pour le démarrage` : "démarre par la partition EFI de ce disque");
      bouts.push(x.reste ? `${gio(x.reste)} restent libres` : "tout l'espace");
      detail.textContent = bouts.join(" · ");
    }

    // Pendant qu'on glisse ou qu'on tape, seuls les nombres et la barre
    // bougent : redessiner la page ferait perdre le champ sous le doigt.
    function regler(n, source) {
      const borne = Math.max(bas, Math.min(haut, Math.round(n)));
      choixDe(disque).taille = borne >= haut ? 0 : borne * 1024;
      D.compris = false;
      const apres = calculDe(disque);
      if (source !== glissiere) glissiere.value = borne;
      if (source !== champ) champ.value = borne;
      total.textContent = gio(apres.creation.taille);
      direDetail(apres.creation);
      N.barre.replaceChildren(...segments(disque, apres));
      N.etat.textContent = etatDisque(disque, apres);
    }

    glissiere.addEventListener("input", () => regler(Number(glissiere.value), glissiere));
    champ.addEventListener("input", () => {
      const n = Number(champ.value);
      if (champ.value !== "" && n >= bas && n <= haut) regler(n, champ);
    });
    champ.addEventListener("change", () => regler(champ.value === "" ? haut : Number(champ.value), null));
    direDetail(k);

    return el("div.in-creation", { "data-ligne": "grenos" },
      el("div.in-ligne",
        el("i.in-point", { "data-genre": "nouveau" }),
        el("div.in-nom", el("b", "grenOS (sera créée)"), detail),
        total,
        el("div.in-geste", el("button.bouton.petit", { onclick: () => changer(disque, (x) => { x.point = null; x.taille = 0; x.effacer = false; }) }, "Annuler"))),
      el("div.in-reglage",
        el("span", "Taille"), glissiere, champ, el("span", "Gio"),
        el("button.bouton.petit.discret", { onclick: () => regler(haut, null) }, "Tout l'espace")));
  }

  function lignes(disque, c) {
    const toutes = disque.partitions.map((p) => ({ debut: p.debut_mio, rang: 0, n: lignePartition(disque, p, c) }));
    for (const z of c.zones) {
      const n = c.creation && c.creation.zone === z ? ligneCreation(disque, c) : ligneLibre(disque, z);
      // L'espace rendu par une suppression se range après ce qui part.
      const dedans = c.partent.filter((p) => p.debut_mio >= z.debut_mio - 1 && p.debut_mio < z.debut_mio + z.taille_mio);
      const debut = c.effacer ? disque.taille_mio : Math.max(z.debut_mio, ...dedans.map((p) => p.debut_mio));
      toutes.push({ debut, rang: 1, n });
    }
    return toutes.sort((a, b) => a.debut - b.debut || a.rang - b.rang).map((t) => t.n);
  }

  function carteDisque(d) {
    const contient = d.partitions.some((p) => p.genre === "windows") ? "Windows ou fichiers"
      : d.partitions.length ? `${d.partitions.length} partition${d.partitions.length > 1 ? "s" : ""}`
      : d.etat === "vide" ? "Vide" : "Données";
    return el("button.in-disque", { "aria-pressed": String(d.dev === D.choisi), onclick: () => { D.choisi = d.dev; D.compris = false; dessiner(); } },
      svg("disque"),
      el("span", el("b", d.modele), el("small", `${gio(d.taille_mio)} · ${d.sorte}`), el("small", contient)));
  }

  function detailDisque(disque) {
    const c = calculDe(disque);
    let tout = null;
    if (disque.etat !== "occupe") {
      if (c.effacer) {
        tout = el("button.bouton.petit", { onclick: () => changer(disque, (x) => { x.effacer = false; x.point = null; x.taille = 0; }) }, "Ne plus tout effacer");
      } else {
        const assez = disque.taille_mio - 2 - D.regles.efi_mio >= D.regles.minimum_mio;
        const mot = disque.etat === "vide" ? "Utiliser tout le disque" : "Effacer tout le disque";
        tout = el("button.bouton.petit", { disabled: !assez, onclick: () => changer(disque, (x) => { x.effacer = true; x.point = 1; x.taille = 0; }) }, mot);
        if (disque.etat !== "vide") tout.classList.add("danger");
      }
    }
    N.barre = el("div.in-barre", {}, segments(disque, c));
    const raison = disque.raison && !c.effacer ? el("p.in-avis", disque.raison) : null;
    return el("section.in-detail",
      el("div.in-detail-tete",
        el("div.in-titre", el("h2", disque.modele), el("p.note", `${gio(disque.taille_mio)} · ${disque.sorte}`)),
        [tout]),
      N.barre,
      [raison],
      el("div.in-liste", {}, lignes(disque, c)));
  }

  function etatDisque(disque, c) {
    if (!disque) return "Choisis le disque où installer grenOS.";
    if (!c.creation) {
      if (c.zones.some((z) => z.possible)) return "Clique « Créer pour grenOS » sur l'espace libre.";
      // Jamais « supprime » devant Windows : ce moteur ne sait pas réduire
      // une partition, et une personne qui veut le garder l'effacerait.
      if (aGarder(disque)) return "Pour garder ce qui est sur ce disque, choisis-en un autre : grenOS ne sait pas encore réduire une partition.";
      return disque.etat === "gpt" ? "Pas assez de place : supprime une partition, ou efface tout le disque." : "Efface tout le disque pour y installer grenOS.";
    }
    const n = c.partent.length;
    const perdu = n ? ` · ${n} partition${n > 1 ? "s" : ""} à supprimer` : "";
    return `grenOS : ${gio(c.creation.taille)}${perdu}. Rien n'est encore écrit.`;
  }

  function pageDisque() {
    const relire = () => el("button.bouton.petit.discret", { onclick: lireDisques }, svg("maj"), "Actualiser");
    if (D.lecture === "charge") return { corps: attente("Lecture des disques…"), suite: { texte: "Continuer", possible: false } };
    if (D.lecture === "illisible") {
      return { corps: message("Les disques n'ont pas pu être lus", D.souciDisques, relire()), suite: { texte: "Continuer", possible: false } };
    }
    if (!D.disques.length) {
      return {
        corps: message("Aucun disque pour grenOS", "Aucun disque de ce PC ne peut recevoir grenOS. La clé de grenOS n'est jamais proposée.", relire()),
        suite: { texte: "Continuer", possible: false },
      };
    }
    const disque = disqueChoisi();
    const c = disque && calculDe(disque);
    return {
      corps: el("div.in-disques",
        el("nav.in-colonne", {}, D.disques.map(carteDisque), relire()),
        disque ? detailDisque(disque) : el("div.in-aucun", el("p.in-etape", "Choisis un disque à gauche."))),
      etat: etatDisque(disque, c),
      suite: { texte: "Continuer", possible: !!(c && c.plan) },
    };
  }

  // ------------------------------------------------------------- 4. compte
  function pageCompte() {
    const lettre = () => D.nom.trim().charAt(0).toUpperCase() || "?";
    const initiale = el("div.in-initiale", lettre());
    const adresse = el("div.mono.in-adresse", D.login ? `${D.login}@grenos` : "ton-nom@grenos");
    const nom = el("input.champ", { id: "in-nom", placeholder: "Ton nom", value: D.nom, maxlength: 60, autocomplete: "off", spellcheck: "false" });
    let jeton = 0;
    nom.addEventListener("input", async () => {
      D.nom = nom.value;
      initiale.textContent = lettre();
      const moi = ++jeton;
      const r = await api.appeler("installation_nom", { texte: D.nom });
      if (moi !== jeton || D.etape !== "compte") return;
      D.login = r.probleme ? "" : r.login;
      adresse.textContent = r.login ? `${r.login}@grenos` : "ton-nom@grenos";
      souci(D.nom.trim() && r.probleme ? r.probleme : "");
    });

    const secret = (texte, champ) => el("input.champ", {
      type: "password", placeholder: texte, "aria-label": texte, value: D[champ], maxlength: 256, autocomplete: "new-password",
      oninput: (e) => { D[champ] = e.target.value; souci(""); },
    });
    const secrets = el("div.in-secrets", { hidden: !D.protege }, secret("Mot de passe", "mot"), secret("Confirme-le", "confirme"));
    const bascule = el("button.interrupteur", {
      role: "switch", type: "button", "aria-checked": String(D.protege), "aria-label": "Protéger par un mot de passe",
      onclick: () => {
        D.protege = !D.protege;
        bascule.setAttribute("aria-checked", String(D.protege));
        secrets.hidden = !D.protege;
        souci("");
        if (D.protege) secrets.querySelector("input").focus();
      },
    });

    return {
      corps: el("div.in-defile",
        el("div.carte.in-compte",
          el("div.in-identite", initiale, el("div", el("b", "Ton compte"), adresse)),
          nom,
          el("div.in-option",
            el("div", el("b", "Protéger par un mot de passe"), el("p.note", "Facultatif. Sans lui, ta session s'ouvre directement.")),
            bascule),
          secrets)),
      suite: { texte: "Continuer", possible: true },
      apres: () => nom.focus(),
    };
  }

  async function verifierCompte() {
    if (!D.nom.trim()) return "Il faut un nom pour créer ton compte.";
    const r = await api.appeler("installation_nom", { texte: D.nom });
    if (r.probleme) return r.probleme;
    D.login = r.login;
    if (D.protege) {
      if (!D.mot) return "Tape un mot de passe, ou coupe la protection.";
      if (D.mot !== D.confirme) return "Les deux mots de passe ne sont pas identiques.";
    }
    return "";
  }

  // ------------------------------------------------------------- 5. résumé
  function pageResume() {
    const disque = disqueChoisi();
    const c = disque && D.regles && calculDe(disque);
    if (!c || !c.plan) {
      return { corps: message("Le disque est à rechoisir", "Reviens à la page Disque."), suite: { texte: "Installer grenOS", possible: false } };
    }
    const perte = c.partent.length > 0 || (c.effacer && disque.etat !== "vide");
    const bloc = (sorte, titre, elements) =>
      el("div.carte.in-bloc", { "data-sorte": sorte }, el("h3", titre), el("ul", {}, elements.map((t) => el("li", t))));

    let part = c.partent.map((p) => `${p.nom} — ${gio(p.taille_mio)}`);
    if (c.effacer && !part.length && disque.etat !== "vide") part = ["Tout ce que contient ce disque"];
    const cree = [`grenOS — ${gio(c.creation.taille)}`];
    if (c.creation.efi_neuve) cree.push(`Démarrage (EFI) — ${gio(c.creation.efi_neuve)}`);
    const reste = [
      ...c.restent.map((p) => `${p.nom} — ${gio(p.taille_mio)}`),
      ...D.disques.filter((d) => d !== disque).map((d) => `${d.modele}, en entier`),
      "La clé de grenOS",
    ];
    const toi = [
      `${D.nom.trim()} (${D.login}@grenos)`,
      D.protege ? "Protégé par un mot de passe" : "Sans mot de passe",
      clavierChoisi().nom,
      D.fuseau.replace(/_/g, " "),
    ];

    const blocs = [];
    if (part.length) blocs.push(bloc("part", "Supprimé pour de bon", part));
    blocs.push(bloc("cree", "Créé", cree), bloc("reste", "Pas touché", reste), bloc("toi", "Toi", toi));

    let accord = null;
    if (perte) {
      const caseAccord = el("input", {
        type: "checkbox", checked: D.compris,
        onchange: (e) => { D.compris = e.target.checked; N.suite.disabled = !D.compris; },
      });
      accord = el("label.in-accord", caseAccord, el("span", "J'ai compris : ce qui est supprimé est perdu pour de bon."));
    }
    return {
      corps: el("div.in-confirme",
        el("div.in-titre", el("h2", `Installer grenOS sur ${disque.modele} ?`)),
        el("div.in-blocs", {}, blocs),
        [accord]),
      suite: { texte: "Installer grenOS", possible: !perte || D.compris },
    };
  }

  // ------------------------------------------------------- 6. installation
  function pageInstallation() {
    if (D.echec) {
      return { corps: message("L'installation s'est arrêtée", D.echec), suite: { texte: "Revenir aux disques", possible: true } };
    }
    N.part = el("div.in-part", `${D.avance.part} %`);
    N.phrase = el("p.in-etape", D.avance.texte || "Préparation");
    N.rempli = el("i");
    N.rempli.style.width = `${D.avance.part}%`;
    return {
      // La roue : GRUB et l'image de démarrage durent une minute au même
      // pourcentage, et un écran immobile se lit « figé ».
      corps: el("div.in-centre", el("div.in-roue"), N.part, N.phrase, el("div.progression", N.rempli),
        el("p.note", "N'éteins pas le PC et ne retire pas la clé.")),
      suite: null,
    };
  }

  // ------------------------------------------------------------ 7. terminé
  function pageTermine() {
    return {
      corps: el("div.in-centre", el("div.in-reussi", svg("coche")), el("h2", "grenOS est installé"),
        el("p.in-etape", "Le PC repartira sur le disque installé. Retire la clé seulement quand le logo de la carte mère réapparaît.")),
      suite: { texte: "Redémarrer maintenant", possible: true },
    };
  }

  const PAGES = {
    bienvenue: pageBienvenue, clavier: pageClavier, disque: pageDisque, compte: pageCompte,
    resume: pageResume, installation: pageInstallation, termine: pageTermine,
  };

  // --------------------------------------------------------- la mécanique
  function souci(texte) {
    N.etat.textContent = texte;
    N.etat.classList.toggle("in-souci", !!texte);
  }

  function dessinerFil() {
    const ici = rangDe(D.etape);
    N.fil.replaceChildren(...ETAPES.map(([cle, nom, icone], i) => {
      const attributs = { "data-etat": i < ici ? "fait" : i === ici ? "ici" : "apres" };
      const revenir = i < ici && enPreparation();
      const contenu = [el("span.in-puce", svg(i < ici ? "coche" : icone)), el("span.in-fil-nom", nom)];
      return el("li", attributs, revenir
        ? el("button", { onclick: () => aller(cle), title: nom }, contenu)
        : el("div", { title: nom }, contenu));
    }));
  }

  function dessiner() {
    const page = PAGES[D.etape]();
    const avant = N.corps.querySelector(".in-liste");
    const defile = avant ? avant.scrollTop : 0;
    N.corps.replaceChildren(page.corps);
    N.sous.textContent = SOUS[D.etape];
    dessinerFil();

    const retour = rangDe(D.etape) > 0 && enPreparation();
    N.retour.style.visibility = retour ? "visible" : "hidden";
    souci("");
    if (page.etat) N.etat.textContent = page.etat;
    N.suite.hidden = !page.suite;
    if (page.suite) {
      N.suiteTexte.textContent = page.suite.texte;
      N.suite.disabled = !page.suite.possible;
    }
    N.pied.hidden = !page.suite && !retour;

    const apres = N.corps.querySelector(".in-liste");
    if (apres) {
      apres.scrollTop = defile;
      // La taille qu'on vient de faire apparaître ne reste pas sous le bord.
      apres.querySelector('[data-ligne="grenos"]')?.scrollIntoView({ block: "nearest" });
    }
    if (page.apres) page.apres();
  }

  function aller(etape) {
    if (etape === "resume") {
      D.compris = false;
      D.resumeDepuis = Date.now();
    }
    D.etape = etape;
    dessiner();
    // « Continuer » et « Installer » sont le MÊME bouton : s'il gardait le
    // focus, Entrée au résumé effacerait le disque (reproduit le 22 novembre).
    if (!N.corps.contains(document.activeElement)) N.corps.focus({ preventScroll: true });
    // Le rapport de la clé dit ainsi jusqu'où la personne est allée.
    api.appeler("premier_dire", { texte: `installation : page ${etape}` });
  }

  function reculer() {
    const i = rangDe(D.etape);
    if (i > 0 && enPreparation()) aller(ETAPES[i - 1][0]);
  }

  let enGeste = false;
  async function avancer() {
    if (enGeste || N.suite.hidden || N.suite.disabled) return;
    enGeste = true;
    try {
      if (D.etape === "compte") {
        const probleme = await verifierCompte();
        if (D.etape !== "compte") return;
        if (probleme) { souci(probleme); return; }
      }
      // Le second clic d'un double-clic lent : personne n'a relu si vite.
      if (D.etape === "resume") {
        if (Date.now() - D.resumeDepuis >= 800) await lancer();
        return;
      }
      if (D.etape === "installation") {
        D.echec = "";
        aller("disque");
        lireDisques();
        return;
      }
      if (D.etape === "termine") {
        N.suite.disabled = true;
        try { await api.appeler("installation_redemarrer"); } catch (e) { N.suite.disabled = false; toast(String(e)); }
        return;
      }
      aller(ETAPES[rangDe(D.etape) + 1][0]);
    } finally {
      enGeste = false;
    }
  }

  async function lireDisques() {
    D.lecture = "charge";
    if (D.etape === "disque") dessiner();
    try {
      const r = await api.appeler("installation_lire");
      if (r.en_cours) {
        D.avance = r.en_cours;
        aller("installation");
        return;
      }
      const avant = new Map(D.disques.map((d) => [d.dev, d.empreinte]));
      D.regles = r;
      D.disques = r.disques;
      // Un disque qui a changé repart sans aucun geste en attente.
      for (const dev of [...D.choix.keys()]) {
        const d = D.disques.find((x) => x.dev === dev);
        if (!d || avant.get(dev) !== d.empreinte) D.choix.delete(dev);
      }
      if (!disqueChoisi()) D.choisi = parDefaut(D.disques);
      D.lecture = "pret";
    } catch (e) {
      D.souciDisques = String(e);
      D.lecture = "illisible";
    }
    // Pendant qu'on tape son nom ou qu'on essaie son clavier : ne rien redessiner.
    if (D.etape === "disque" || D.etape === "resume") dessiner();
  }

  async function lancer() {
    const c = clavierChoisi();
    const plan = {
      ...calculDe(disqueChoisi()).plan,
      compte: { login: D.login, nom: D.nom.trim(), mot_de_passe: D.protege ? D.mot : null },
      clavier: { disposition: c.disposition, variante: c.variante },
      fuseau: D.fuseau,
    };
    D.avance = { part: 0, texte: "Vérification du plan" };
    D.echec = "";
    aller("installation");
    try {
      await api.appeler("installation_lancer", { plan });
    } catch (e) {
      D.echec = String(e);
      dessiner();
    }
  }

  async function demarrer() {
    dessiner();
    pret();
    try {
      D.infos = await api.appeler("installation_infos");
    } catch (e) {
      N.corps.replaceChildren(message("L'installateur n'a pas pu s'ouvrir", String(e)));
      return;
    }
    const i = D.infos;
    let n = i.claviers.findIndex((c) => c.disposition === i.clavier && c.variante === i.variante && c.fuseau === i.fuseau);
    if (n < 0) n = i.claviers.findIndex((c) => c.disposition === i.clavier && c.variante === i.variante);
    D.clavier = Math.max(0, n);
    D.fuseau = i.fuseaux.includes(i.fuseau) ? i.fuseau : clavierChoisi().fuseau;
    D.fuseauVoulu = D.fuseau !== clavierChoisi().fuseau;
    D.nom = i.nom || "";
    if (D.nom) {
      const r = await api.appeler("installation_nom", { texte: D.nom });
      D.login = r.probleme ? "" : r.login;
    }
    if (i.en_cours) {
      D.avance = i.en_cours;
      aller("installation");
      return;
    }
    dessiner();
    // Lus dès la bienvenue : la page Disque n'a rien à faire attendre.
    if (i.cle) lireDisques();
  }

  N.sous = el("p.note");
  N.fil = el("ol.in-fil", { "aria-label": "Étapes de l'installation" });
  N.corps = el("main.in-corps", { tabindex: "-1" });
  N.retour = el("button.bouton.discret", { onclick: reculer }, svg("retour"), "Retour");
  N.etat = el("p.in-resume", { role: "status" });
  N.suiteTexte = el("span");
  N.suite = el("button.bouton.principal", {
    onclick: avancer,
    // Au résumé, la souris ne lui donne jamais le focus : le second appui
    // d'un double-clic le lui rendait, et Entrée installait. Tab, lui, reste.
    onmousedown: (e) => { if (D.etape === "resume") e.preventDefault(); },
  }, N.suiteTexte, svg("fleche"));
  N.pied = el("footer.in-pied", N.retour, N.etat, N.suite);
  document.body.append(aurore(), el("div.in-page",
    el("header.tete", el("span.in-logo", svg("marque")), el("div", el("h1", "Installer grenOS"), N.sous)),
    N.fil, N.corps, N.pied));

  // UN DOUBLE-CLIC NE VAUT QU'UN CLIC, partout (22 novembre, reproduit). Chaque
  // geste redessine : sous le pointeur, « Supprimer » devient « Garder » et
  // le second clic annulait le premier (un bouton qui semble mort) ; au
  // compte, il tombait sur « Installer ». Le temps et la place comptent aussi,
  // au cas où le moteur web recompterait à 1 sur le bouton redessiné.
  let dernierClic = { quand: -1e9, x: 0, y: 0 };
  document.addEventListener("click", (e) => {
    if (!e.detail || !(e.clientX || e.clientY) || !e.target.closest?.("button")) return;
    const avant = dernierClic;
    dernierClic = { quand: e.timeStamp, x: e.clientX, y: e.clientY };
    const proche = Math.abs(e.clientX - avant.x) < 6 && Math.abs(e.clientY - avant.y) < 6;
    if (e.detail > 1 || (proche && e.timeStamp - avant.quand < 350)) {
      e.preventDefault();
      e.stopPropagation();
    }
  }, true);

  // Entrée continue — jamais au résumé : on n'efface pas un disque d'une
  // touche tenue trop longtemps.
  document.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" || e.repeat || ["resume", "installation", "termine"].includes(D.etape)) return;
    const cible = e.target;
    if (cible instanceof HTMLButtonElement || cible instanceof HTMLSelectElement) return;
    if (cible instanceof HTMLInputElement && (cible.dataset.entree || cible.type === "number")) { cible.blur(); return; }
    e.preventDefault();
    avancer();
  });

  api.ecouter("installation-avance", (a) => {
    D.avance = a;
    if (D.etape !== "installation" || D.echec || !N.part) return;
    N.part.textContent = `${a.part} %`;
    N.phrase.textContent = a.texte;
    N.rempli.style.width = `${a.part}%`;
  });
  api.ecouter("installation-fin", (f) => {
    if (f.bien) {
      D.mot = "";
      D.confirme = "";
      aller("termine");
    } else {
      D.echec = f.texte || "L'installation n'a pas pu aller au bout.";
      aller("installation");
    }
  });

  habiller();
  Promise.resolve(api.appeler("apparence")).then((a) => a && habiller(a)).catch(() => {});
  demarrer();
})();
