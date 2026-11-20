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
  // Ouverte par la session pour proposer la fréquence maximale de l'écran.
  const PROPOSER_MAX = window.GRENOS_ONGLET === "ecran-max";
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

  // QUEL AFFICHAGE TOURNE (20 novembre), pour les RTX 50 : le pilote libre
  // `nouveau`, ou l'image du firmware sur laquelle grenOS s'est replié quand
  // l'écran n'était pas lisible (grenos-affichage-garde). On le DIT, et on
  // permet de réessayer — sinon une machine repliée une fois le resterait
  // sans que personne sache pourquoi. Pendant un réessai, c'est ici que la
  // personne confirme qu'elle voit l'écran.
  function carteAffichage(a) {
    if (!a || !a.rtx50) return null;
    const dire = el("p.note");
    if (a.essai) {
      const oui = bouton("Oui, garder l'affichage complet", "principal", async () => {
        const r = await R("reglages_affichage_confirmer");
        dire.textContent = r.souci ? `Pas gardé : ${r.souci}` : "Gardé : ta carte reste sur l'affichage complet.";
        oui.disabled = !r.souci;
      });
      return carte(el("p", el("b", "Tu vois bien cet écran ? "),
        "grenOS essaie l'affichage complet de ta carte. Sans réponse dans les trois minutes qui suivent le démarrage, il revient tout seul à l'image sûre."), oui, dire);
    }
    if (a.mode === "firmware" && a.peut_reessayer) {
      const essayer = bouton("Réessayer l'affichage complet", "principal", async () => {
        const r = await R("reglages_affichage_reessayer");
        if (r.souci) { dire.textContent = `Impossible : ${r.souci}`; return; }
        essayer.disabled = true;
        dire.replaceChildren("Noté pour le prochain démarrage. ", bouton("Redémarrer maintenant", "", () => ouvrirProgramme("grenos-arret")));
      });
      return carte(ligne("Affichage", el("span.note", "image sûre du firmware — un écran, 60 Hz")),
        note("grenOS s'y est replié parce que l'écran n'était pas lisible avec le pilote libre de ta carte. Tu peux réessayer : si l'image n'est pas bonne, il reviendra tout seul ici trois minutes après le démarrage."),
        essayer, dire);
    }
    if (a.mode === "nouveau") return carte(ligne("Affichage", el("span.note", "pilote libre de ta carte — plusieurs écrans, haute fréquence")));
    if (a.mode === "nvidia") return carte(ligne("Affichage", el("span.note", "pilote officiel NVIDIA")));
    return null;
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
        if (!liste.length) fonds.append(el("p.note", "Aucune image dans tes dossiers Images, Bureau et Téléchargements : « Parcourir… » va la chercher ailleurs."));
      };
      // Le vrai sélecteur de fichiers, pour une image rangée n'importe où.
      const parcourir = async () => {
        try { if (await api.appeler("reglages_parcourir_fond")) poserFonds(); } catch (e) { toast(String(e)); }
      };
      poserFonds();
      return [titre("Apparence", "Le même bureau, deux lumières."),
        carte(ligne("Thème", el("div.rg-rangee", bouton("Nuit", "", () => theme("nuit")), bouton("Jour", "", () => theme("jour")))),
          ligne("Couleur d'accent", reglage("accent", [["bleu", "Bleu"], ["violet", "Violet"], ["vert", "Vert"], ["orange", "Orange"], ["rose", "Rose"], ["cyan", "Cyan"]])),
          ligne("Effets", reglage("effets", [["complets", "Complets"], ["equilibres", "Équilibrés"], ["performance", "Performance"]])),
          ligne("Taille de l'affichage", choix([96, 108, 120, 144, 168, 192].map((d) => [String(d), `${Math.round(d * 100 / 96)} %`]),
            String(await api.appeler("reglages_dpi_lire")),
            async (v) => { try { dire.textContent = await api.appeler("reglages_dpi", { dpi: Number(v) }); } catch (e) { dire.textContent = String(e); } })),
          ligne("Barre des tâches", choix([["barre", "Barre"], ["dock", "Dock"]], await api.appeler("reglages_barre_lire"),
            async (v) => { try { dire.textContent = await api.appeler("reglages_barre", { disposition: v }); } catch (e) { dire.textContent = String(e); } })),
          ligne("Dispositions en glissant une fenêtre", interrupteur(await api.appeler("reglages_dispositions_lire"),
            async (v) => { try { dire.textContent = await api.appeler("reglages_dispositions", { actif: v }); } catch (e) { dire.textContent = String(e); } })),
          dire,
          note("Meta+T fait la bascule sans ouvrir cette fenêtre. Le fond d'écran, la barre, les menus et les applications suivent ensemble.")),
        carte(ligne("Fond d'écran", bouton("Parcourir…", "principal", parcourir)), fonds,
          note("Une image à toi, posée sur le bureau : choisis-la ci-dessous parmi tes images récentes, ou va la chercher avec « Parcourir… ». Sans choix, grenOS garde le sien."))];
    },

    async ecran() {
      const affichage = carteAffichage(await R("reglages_affichage").catch(() => null));
      const v = await R("reglages_ecran");
      if (!v.ecran) return [titre("Écran"), affichage, carte(note("Aucun écran interrogeable ici."))].filter(Boolean);
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
      // GARDER OU REVENIR (20 novembre), comme Windows : rien n'est noté
      // tant qu'on n'a pas dit « Garder », et sans réponse en 15 s — écran
      // noir, plus de signal — on revient de soi-même à l'ancien réglage.
      const confirmer = el("div.rg-confirmer");
      confirmer.hidden = true;
      let minuteur = null;
      const appliquer = async () => {
        const t = v.modes[mode].taux[Number(taux.value)];
        if (!t) { dire.textContent = "Choisis une fréquence."; return; }
        const taille = v.modes[mode].taille;
        const r = await R("reglages_ecran_appliquer", { nom: e.nom, taille, brut: t.brut, annoncee: t.annoncee });
        if (r.souci) { dire.textContent = r.souci; return; }
        dire.textContent = "";
        let reste = 15;
        const compte = el("strong", String(reste));
        const finir = () => { clearInterval(minuteur); minuteur = null; confirmer.hidden = true; };
        const revenir = async (texte) => {
          finir();
          if (r.precedent) await R("reglages_ecran_revenir", { nom: e.nom, taille: r.precedent.taille, brut: r.precedent.brut });
          dire.textContent = texte;
        };
        confirmer.replaceChildren(
          el("p", `${taille.replace("x", " × ")} à ${t.arrondi} images par seconde. Garder ce réglage ? Retour à l'ancien dans `, compte, " s."),
          el("div.rg-rangee",
            bouton("Garder", "principal", async () => {
              finir();
              await R("reglages_ecran_garder", { nom: e.nom, mode: r.mode, taux: r.taux });
              dire.textContent = `Gardé : ${taille.replace("x", " × ")} à ${t.arrondi} images par seconde.`;
            }),
            bouton("Revenir", "", () => revenir("Ancien réglage remis."))));
        confirmer.hidden = false;
        clearInterval(minuteur);
        minuteur = setInterval(() => {
          reste -= 1;
          compte.textContent = String(reste);
          if (reste <= 0) revenir("Pas de réponse en 15 secondes : l'ancien réglage est revenu. L'écran n'a sans doute pas accepté cette fréquence.");
        }, 1000);
      };
      // LA FRÉQUENCE MAXIMALE, PROPOSÉE (20 novembre). Grenofar : « mon
      // écran est à 300 Hz » — grenOS démarrait à 60. La session ouvre cette
      // page (« ecran-max ») une fois par écran : on y essaie la plus haute
      // fréquence que l'écran annonce, avec le même « Garder ? » de 15 s ;
      // un écran qui n'affiche rien revient seul à l'ancien réglage.
      if (PROPOSER_MAX && !D.propose) {
        D.propose = true;
        setTimeout(async () => {
          const p = (await R("reglages_ecran_proposition").catch(() => ({}))).proposition;
          if (!p || p.nom !== e.nom) return;
          const m = v.modes.findIndex((x) => x.taille === p.taille);
          if (m < 0) return;
          mode = m; tailles.value = String(m); remplirTaux();
          const i = v.modes[m].taux.findIndex((t) => t.brut === p.brut);
          if (i < 0) return;
          taux.value = String(i);
          await appliquer();
        }, 900);
      }
      return [titre("Écran", `Écran ${e.nom}, actuellement ${e.actuelle.replace("x", " × ") || "inconnu"} à ${e.taux || "?"} images par seconde.`),
        affichage,
        carte(ligne("Définition", tailles), ligne("Images par seconde", taux), bouton("Appliquer", "principal", appliquer), confirmer, dire,
          ligne("Plusieurs écrans", bouton("Disposer les écrans", "", () => ouvrirProgramme("arandr"))),
          v.virtuelle
            ? note(`grenOS tourne dans une machine virtuelle (${v.virtuelle}) : son écran est virtuel, et c'est ${v.virtuelle === "oracle" ? "VirtualBox" : "l'hyperviseur"} qui le recopie sur ton vrai écran, à sa propre cadence. Choisir une fréquence plus haute ici n'y changerait rien ; sur un vrai PC, toutes celles de ton écran sont proposées.`)
            : note("Les fréquences marquées « à essayer » ne sont pas annoncées par l'écran, mais restent sous son maximum. Après « Appliquer », rien n'est gardé tant que tu n'as pas cliqué « Garder » : si l'image disparaît, attends 15 secondes, l'ancien réglage revient tout seul."))].filter(Boolean);
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
          note("Un mot de passe protège aussi l'installation de logiciels et les réglages du système.")),
        await carteCle()];
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

    // La page complète, comme l'ancienne fenêtre : l'installation automatique,
    // « Vérifier maintenant », et la préparation avec sa barre et son détail.
    // Mêmes commandes que la fenêtre Mises à jour (maj.rs) : une seule façon
    // de mettre à jour la machine. Et rien ne s'installe pendant la session :
    // la préparation télécharge, l'installation se fait au redémarrage.
    async maj() {
      const v = await R("reglages_maj");
      let e = await api.appeler("maj_etat");
      const resume = note(v.resume);
      const direAuto = el("p.note");
      const phrase = el("p.rg-diag", "Prêt.");
      const barre = el("i", { style: "width:0%" });
      const journal = el("pre.journal.rg-journal");
      const details = el("details", el("summary", "Voir le détail"), journal);
      const principal = el("button.bouton.petit.principal");
      const relire = async () => {
        e = await api.appeler("maj_etat");
        const n = e.disponibles;
        resume.textContent = n == null ? "Aucune vérification depuis le démarrage, ou pas de réseau."
          : n === 0 ? `Rien à installer. Dernière vérification : ${e.date} UTC.`
          : `${n} paquet${n > 1 ? "s" : ""} à installer. Vérifié le ${e.date} UTC.`;
        principal.textContent = e.arme ? "Redémarrer maintenant" : e.en_cours ? "En cours…" : "Vérifier et préparer";
        principal.disabled = e.en_cours;
        return n;
      };
      principal.addEventListener("click", async () => {
        if (e.arme) { api.appeler("maj_redemarrer").catch((x) => { phrase.textContent = String(x); }); return; }
        principal.disabled = true; principal.textContent = "En cours…"; journal.textContent = "";
        try { await api.appeler("maj_preparer"); } catch (x) { phrase.textContent = String(x); relire(); }
      });
      api.ecouter("maj-avance", (a) => { barre.style.width = `${Math.max(2, Math.min(100, a.part))}%`; phrase.textContent = a.texte; });
      api.ecouter("maj-ligne", (l) => { journal.textContent += `${l}\n`; journal.scrollTop = journal.scrollHeight; });
      api.ecouter("maj-fin", async (f) => { barre.style.width = "100%"; phrase.textContent = f.texte; if (!f.bien) details.open = true; await relire(); });
      const auto = interrupteur(e.mode === "automatiques", async (actif) => {
        try {
          await api.appeler("maj_mode", { mode: actif ? "automatiques" : "demander" });
          direAuto.textContent = actif ? "Les mises à jour s'installeront toutes seules, chaque jour — au redémarrage, avant le bureau." : "Les mises à jour ne s'installeront plus toutes seules.";
        } catch (x) { direAuto.textContent = String(x); auto.checked = !actif; }
      });
      const verifier = async () => {
        direAuto.textContent = "Vérification en cours…";
        try {
          await api.appeler("maj_verifier");
          const n = await relire();
          direAuto.textContent = n ? `${n} paquet${n > 1 ? "s" : ""} à installer.` : "Rien à installer : tout est à jour.";
        } catch (x) { direAuto.textContent = String(x); }
      };
      await relire();
      return [titre("Mises à jour", v.version),
        carte(resume,
          ligne("Installer automatiquement, chaque jour", auto),
          bouton("Vérifier maintenant", "discret", verifier),
          direAuto,
          note("La vérification quotidienne ne fait que regarder : rien ne s'installe sans que tu le demandes, sauf si tu actives l'installation automatique ci-dessus. Et rien ne s'installe pendant la session : tout se pose au redémarrage, avant le bureau.")),
        carte(phrase, el("div.progression", barre), principal, details,
          note("Les nouveautés de grenOS et les correctifs de sécurité de Debian arrivent ensemble, sans jamais retélécharger l'image. Les applications Flatpak se mettent à jour tout de suite, le système au prochain redémarrage.")),
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
        carte(...v.lignes.map(([n, t]) => ligne(n, el("span.note", t))),
          ligne("Disques et clés USB", bouton("Ouvrir Disques", "", () => ouvrirProgramme("gnome-disks")))),
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

  // LA CLÉ DE SÉCURITÉ (19 novembre) : une clé FIDO2/U2F (YubiKey…) qu'on
  // touche au lieu de taper son mot de passe. Facultative, jamais
  // obligatoire : le mot de passe marche toujours.
  async function carteCle() {
    const etat = await api.appeler("reglages_cle_etat");
    const dire = el("p.note");
    const zone = el("div.rg-rangee");
    const redessiner = async () => {
      const e = await api.appeler("reglages_cle_etat");
      statut.textContent = e.cles ? `${e.cles} clé${e.cles > 1 ? "s" : ""} enregistrée${e.cles > 1 ? "s" : ""}.` : "Aucune clé enregistrée.";
      retirer.hidden = !e.cles;
    };
    const statut = el("span");
    const ajouter = bouton("Ajouter une clé", "principal", async () => {
      ajouter.disabled = true;
      dire.textContent = "Branche ta clé, puis touche-la quand elle clignote (30 secondes)…";
      try { dire.textContent = await api.appeler("reglages_cle_ajouter"); } catch (e) { dire.textContent = String(e); }
      ajouter.disabled = false;
      redessiner();
    });
    const retirer = bouton("Retirer mes clés", "discret", async () => {
      try { dire.textContent = await api.appeler("reglages_cle_retirer"); } catch (e) { dire.textContent = String(e); }
      redessiner();
    });
    zone.append(ajouter, retirer);
    await redessiner();
    return carte(ligne("Clé de sécurité", statut),
      etat.outil ? zone : note("L'outil des clés de sécurité n'est pas encore installé : il arrive avec la prochaine mise à jour."),
      dire,
      note("Une clé FIDO2 ou U2F (YubiKey, SoloKey, Titan…) : la toucher ouvre ta session et valide les gestes d'administrateur. Le mot de passe marche toujours — perdre la clé ne t'enferme pas dehors."));
  }

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
    const depart = PROPOSER_MAX ? "ecran"
      : PAGES.some((p) => p[0] === window.GRENOS_ONGLET) ? window.GRENOS_ONGLET : "apparence";
    montrer(depart);
    pret();
    api.appeler("reglages_ouvert");
    // Les autres, une par une, quand rien ne presse.
    for (const [cle] of PAGES) await garnir(cle);
  })();
})();
