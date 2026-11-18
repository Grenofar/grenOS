// Les Réglages : dix pages dans un volet, et rien qui demande de connaître
// Linux. Toutes se remplissent dès l'ouverture, une par une (la page visible
// d'abord) : la fenêtre s'affiche tout de suite, et les lignes que la CI
// attend de chaque page arrivent quand même.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, ouvrirProgramme, toast } = window.grenos;
  const PAGES = [
    ["apparence", "Apparence", "palette"], ["ecran", "Écran", "bureau"], ["son", "Son", "son"],
    ["reseau", "Réseau", "wifi"], ["compte", "Compte", "compte"], ["heure", "Heure et date", "sablier"],
    ["maj", "Mises à jour", "maj"], ["composants", "Mise à jour composant", "cpu"],
    ["securite", "Sécurité", "bouclier"], ["materiel", "Matériel", "ram"],
  ];
  const D = { pages: {} };
  const R = (fonction, arguments_) => api.appeler("reglages", { fonction, arguments: arguments_ || {} });
  const note = (t) => el("p.note.rg-note", t);
  const ligne = (t, w) => el("div.rg-ligne", el("span", t), w);
  const carte = (...e) => el("section.carte.verre.rg-carte", ...e);
  const titre = (t, s) => el("header.rg-titre", el("h2", t), s ? el("p.note", s) : null);
  const bouton = (t, genre, f) => el(`button.bouton.petit${genre ? "." + genre : ""}`, { onclick: f }, t);
  function interrupteur(actif, f) {
    const c = el("input.cx-inter", { type: "checkbox", role: "switch" });
    c.checked = !!actif;
    c.addEventListener("change", () => f(c.checked));
    return c;
  }
  function choix(options, actuel, f) {
    const s = el("select.champ.rg-choix", options.map(([v, t]) => el("option", { value: v, selected: v === actuel }, t)));
    s.addEventListener("change", () => f(s.value, s.selectedIndex));
    return s;
  }

  // ---- Les pages ---------------------------------------------------------------
  const CONSTRUIRE = {
    async apparence() {
      const a = await api.appeler("apparence");
      const dire = el("p.note");
      const theme = (m) => api.appeler("reglages_theme", { moment: m }).then(() => setTimeout(async () => habiller(await api.appeler("apparence")), 600));
      const reglage = (cle, valeurs) => choix(valeurs, a[cle], async (v) => {
        try { await api.appeler("regler_apparence", { cle, valeur: v }); habiller(await api.appeler("apparence")); } catch (e) { dire.textContent = String(e); }
      });
      const fonds = el("div.rg-fonds", el("p.note", "Lecture de tes images…"));
      const poserFonds = async () => {
        const liste = await api.appeler("reglages_fonds");
        fonds.replaceChildren(
          el("button.rg-fond", { onclick: async () => { await api.appeler("reglages_fond", { chemin: "" }); poserFonds(); } }, el("strong", "Le fond de grenOS"), el("small", "celui de l'image")),
          ...liste.map((f) => el(`button.rg-fond${f.actuel ? ".actif" : ""}`, {
            onclick: async () => { try { await api.appeler("reglages_fond", { chemin: f.chemin }); poserFonds(); } catch (e) { toast(String(e)); } },
          }, el("strong", f.nom.split("/").pop()), el("small", f.nom.split("/")[0]))));
        if (!liste.length) fonds.append(el("p.note", "Pose une image dans ton dossier Images pour la choisir ici."));
      };
      poserFonds();
      return [titre("Apparence", "Le même bureau, deux lumières."),
        carte(ligne("Thème", el("div.rg-rangee", bouton("Nuit", "", () => theme("nuit")), bouton("Jour", "", () => theme("jour")))),
          ligne("Couleur d'accent", reglage("accent", [["bleu", "Bleu"], ["violet", "Violet"], ["vert", "Vert"], ["orange", "Orange"], ["rose", "Rose"], ["cyan", "Cyan"]])),
          ligne("Effets", reglage("effets", [["complets", "Complets"], ["equilibres", "Équilibrés"], ["performance", "Performance"]])),
          dire,
          note("Meta+T fait la bascule sans ouvrir cette fenêtre. Le fond d'écran, la barre, les menus et les applications suivent ensemble.")),
        carte(el("div.etiquette", "Fond d'écran"), fonds,
          note("Une image à toi, posée sur le bureau : celles de tes dossiers Images, Bureau et Téléchargements. Sans choix, grenOS garde le sien."))];
    },

    async ecran() {
      const v = await R("reglages_ecran");
      if (!v.ecran) return [titre("Écran"), carte(note("Aucun écran interrogeable ici."))];
      const e = v.ecran;
      const dire = el("p.note");
      let mode = Math.max(0, v.modes.findIndex((m) => m.taille === e.actuelle));
      const taux = el("select.champ.rg-choix");
      const remplirTaux = () => {
        const liste = v.modes[mode].taux;
        const vise = Math.round(parseFloat(e.taux) || 0);
        taux.replaceChildren(...liste.map((t, i) => el("option", { value: i, selected: t.arrondi === vise }, t.annoncee ? `${t.arrondi} Hz` : `${t.arrondi} Hz — à essayer`)));
      };
      const tailles = choix(v.modes.map((m, i) => [String(i), m.taille.replace("x", " × ")]), String(mode), (val) => { mode = Number(val); remplirTaux(); });
      remplirTaux();
      const appliquer = async () => {
        const t = v.modes[mode].taux[Number(taux.value)];
        if (!t) { dire.textContent = "Choisis une fréquence."; return; }
        const r = await R("reglages_ecran_appliquer", { nom: e.nom, taille: v.modes[mode].taille, brut: t.brut, annoncee: t.annoncee });
        dire.textContent = r.souci || `Appliqué : ${v.modes[mode].taille.replace("x", " × ")} à ${t.arrondi} images par seconde.`;
      };
      return [titre("Écran", `Écran ${e.nom}, actuellement ${e.actuelle.replace("x", " × ") || "inconnu"} à ${e.taux || "?"} images par seconde.`),
        carte(ligne("Définition", tailles), ligne("Images par seconde", taux), bouton("Appliquer", "principal", appliquer), dire,
          note("Les fréquences marquées « à essayer » ne sont pas annoncées par le pilote : elles sont calculées puis proposées à l'écran. S'il refuse, rien ne change — et s'il accepte mais que l'image reste noire, attends dix secondes, rien n'est enregistré tant que l'essai n'a pas réussi."))];
    },

    async son() {
      const v = await R("reglages_son");
      const contenu = v.sorties.length ? [
        ligne("Sortie", choix(v.sorties, v.courante, (val) => R("reglages_son_regler", { geste: "sortie", valeur: val }))),
        ligne("Volume", (() => {
          const c = el("input.rg-curseur", { type: "range", min: 0, max: 100, value: v.volume });
          c.addEventListener("change", () => R("reglages_son_regler", { geste: "volume", valeur: c.value }));
          return c;
        })()),
        ligne("Silence", interrupteur(v.muet, () => R("reglages_son_regler", { geste: "muet" }))),
      ] : [note("Aucune sortie audio détectée. Dans une machine virtuelle, vérifie que le son est activé côté hôte.")];
      return [titre("Son", "Par où le son sort, et à quel volume."),
        carte(...contenu),
        carte(ligne("Réglages détaillés", bouton("Ouvrir le mélangeur", "", () => ouvrirProgramme("pavucontrol"))), note("Le mélangeur règle le volume application par application.")),
        carte(el("p.rg-diag", v.diagnostic), note("Ce que grenOS lit sur la machine, sans lancer aucun programme : c'est ce qui est là, pas ce qui devrait l'être."))];
    },

    async reseau() {
      const etat = await api.appeler("reglages_reseau");
      return [titre("Réseau", `État : ${etat}.`),
        carte(ligne("Connexions", el("div.rg-rangee",
          bouton("Wi-Fi et Bluetooth", "principal", () => ouvrirProgramme("grenos-connexions")),
          bouton("Réglages avancés", "", () => ouvrirProgramme("nm-connection-editor")))),
        note("Wi-Fi, câble, partage de connexion : tout se règle là. Dans une machine virtuelle, le réseau passe par l'hôte et fonctionne sans rien configurer."))];
    },

    async compte() {
      const c = await api.appeler("reglages_compte");
      const m1 = el("input.champ", { type: "password", placeholder: "Nouveau mot de passe", autocomplete: "new-password" });
      const m2 = el("input.champ", { type: "password", placeholder: "Le même, pour être sûr", autocomplete: "new-password" });
      const dire = el("p.note");
      const faire = async (commande, args) => {
        try { dire.textContent = await api.appeler(commande, args); m1.value = m2.value = ""; } catch (e) { dire.textContent = String(e); }
      };
      return [titre("Compte", `${c.adresse} — ${c.nom || "sans nom affiché"}`),
        carte(note(c.protege ? "Cette machine demande un mot de passe pour ouvrir la session." : "Cette machine s'ouvre sans mot de passe : un clic suffit."),
          m1, m2,
          el("div.rg-rangee", bouton("Enregistrer", "principal", () => faire("reglages_mot_de_passe", { premier: m1.value, second: m2.value })),
            c.protege ? bouton("Retirer le mot de passe", "discret", () => faire("reglages_retirer_mot_de_passe")) : null),
          dire,
          note("Un mot de passe protège aussi l'installation de logiciels et les réglages du système."))];
    },

    async heure() {
      const v = await R("reglages_heure");
      const dire = el("p.note");
      const regler = async (geste, valeur) => { dire.textContent = (await R("reglages_heure_regler", { geste, valeur })).texte; };
      const champ = el("input.champ.rg-choix", { placeholder: "2026-09-23 18:30:00" });
      return [titre("Heure et date", "Réglée par le réseau, ou à la main."),
        carte(ligne("Fuseau horaire", choix(v.fuseaux.map((f) => [f, f]), v.fuseau, (f) => regler("fuseau", f))),
          ligne("Mise à l'heure automatique", interrupteur(v.automatique, (a) => regler("automatique", a))),
          dire,
          note("Avec la mise à l'heure automatique, la machine demande l'heure au réseau et n'en dévie jamais. Sans réseau, désactive-la et règle-la toi-même.")),
        carte(ligne("Régler à la main", champ), bouton("Appliquer cette heure", "", () => regler("main", champ.value)),
          note("Format : année-mois-jour heures:minutes:secondes."))];
    },

    async maj() {
      const v = await R("reglages_maj");
      return [titre("Mises à jour", v.version),
        carte(note(v.resume),
          ligne("Vérifier, préparer, choisir le mode", bouton("Ouvrir Mises à jour", "principal", () => ouvrirProgramme("grenos-maj"))),
          note("La vérification quotidienne ne fait que regarder : rien ne s'installe sans que tu le demandes, sauf si tu choisis les mises à jour automatiques. Et rien ne s'installe pendant la session : tout se pose au redémarrage, avant le bureau.")),
        carte(ligne("Applications", bouton("Ouvrir GrenPlace", "", () => ouvrirProgramme("grenplace"))),
          note("Les applications installées depuis GrenPlace se mettent à jour avec le reste du système."))];
    },

    async composants() {
      const liste = el("p.rg-diag", "Lecture des composants…");
      const dire = el("p.note");
      R("reglages_composants").then((v) => {
        if (!v.present) liste.textContent = "fwupd n'est pas installé : aucun composant n'est interrogeable.";
        else if (!v.composants.length) liste.textContent = "Aucun composant interrogeable sur cette machine. C'est le cas ordinaire d'une machine virtuelle : elle n'a pas de micrologiciel à mettre à jour.";
        else liste.textContent = v.composants.map((c) => `${c.nom} — version ${c.version}${c.peut_etre_mis_a_jour ? "" : "  (non modifiable)"}`).join("\n");
      }).catch((e) => { liste.textContent = String(e); });
      const chercher = async () => { dire.textContent = "Lecture chez les constructeurs…"; dire.textContent = (await R("reglages_composants_chercher")).texte; };
      return [titre("Mise à jour composant", "Les micrologiciels de la machine : carte mère, contrôleurs, disques, cartes graphiques."),
        carte(liste, bouton("Chercher des mises à jour", "principal", chercher), dire),
        carte(note("Le microcode du processeur Intel ou AMD ne passe pas par ici : il arrive avec « Mettre à jour grenOS », comme le reste du système."),
          note("Le pilote NVIDIA non plus : la page Jeux le propose quand la machine a une carte NVIDIA."))];
    },

    async securite() {
      const v = await R("reglages_securite");
      const etat = el("p.rg-diag", v.resume);
      const dire = el("p.note");
      const telecharger = async () => {
        dire.textContent = "Téléchargement en cours, cela peut durer…";
        try { const r = await api.appeler("reglages_signatures"); etat.textContent = r.resume; dire.textContent = r.souci || r.resume; }
        catch (e) { dire.textContent = String(e); }
      };
      return [titre("Sécurité", "ClamAV, l'antivirus libre, et sa base de signatures."),
        carte(etat, bouton("Télécharger les signatures", "principal", telecharger), dire,
          note("La base pèse plusieurs centaines de mégaoctets et change tous les jours : elle n'est pas dans l'image, elle se télécharge ici.")),
        carte(note("Les logiciels malveillants qui visent un bureau Linux sont rares. ClamAV sert surtout à ne pas faire suivre un fichier infecté à une machine Windows — une clé USB, une pièce jointe, un fichier partagé."),
          note("Ni Malwarebytes ni Avast n'ont de produit Linux de bureau : Avast a retiré le sien en 2021, Malwarebytes n'en a jamais publié."))];
    },

    async materiel() {
      const v = await R("reglages_materiel");
      return [titre("Matériel", "Ce que la machine a, et ce que grenOS en voit."),
        carte(...v.lignes.map(([n, t]) => ligne(n, el("span.note", t)))),
        carte(el("p.rg-diag", v.explication))];
    },
  };

  async function garnir(cle) {
    if (D.pages[cle].dataset.garnie) return;
    D.pages[cle].dataset.garnie = "1";
    try { D.pages[cle].replaceChildren(...(await CONSTRUIRE[cle]()).filter(Boolean)); }
    catch (e) { D.pages[cle].replaceChildren(titre(PAGES.find((p) => p[0] === cle)[1]), carte(note(`Cette page n'a pas pu être lue : ${e}`))); }
  }

  function montrer(cle) {
    for (const [c] of PAGES) {
      D.pages[c].hidden = c !== cle;
      D.boutons[c].classList.toggle("actif", c === cle);
    }
    D.contenu.scrollTop = 0;
    garnir(cle);
  }

  window.grenosOnglet = (cle) => { if (D.pages[cle]) montrer(cle); };

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.boutons = {};
    const volet = el("nav.gp-volet.carte.verre.rg-volet");
    D.contenu = el("main.rg-contenu");
    for (const [cle, nom, icone] of PAGES) {
      D.boutons[cle] = el("button", { onclick: () => montrer(cle) }, el("span.rg-icone", svg(icone)), nom);
      volet.append(D.boutons[cle]);
      D.pages[cle] = el("div.rg-page", { hidden: true }, el("p.note", "…"));
      D.contenu.append(D.pages[cle]);
    }
    document.body.append(el("div.page-reglages", volet, D.contenu));
    const depart = PAGES.some((p) => p[0] === window.GRENOS_ONGLET) ? window.GRENOS_ONGLET : "apparence";
    montrer(depart);
    pret();
    api.appeler("reglages_ouvert");
    // Les autres, une par une, quand rien ne presse.
    for (const [cle] of PAGES) await garnir(cle);
  })();
})();
