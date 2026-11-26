// Les Réglages : douze pages dans un volet, et rien qui demande de connaître
// Linux. Toutes se remplissent dès l'ouverture, une par une (la page visible
// d'abord) : la fenêtre s'affiche tout de suite, et les lignes que la CI
// attend de chaque page arrivent quand même.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, ouvrirProgramme, toast } = window.grenos;
  const PAGES = [
    ["apparence", "Apparence", "palette"], ["ecran", "Écran", "bureau"], ["son", "Son", "son"],
    ["reseau", "Réseau", "wifi"], ["compte", "Compte", "compte"], ["heure", "Heure et date", "sablier"],
    ["applications", "Applications", "paquet"], ["maj", "Mises à jour", "maj"], ["composants", "Mise à jour composant", "cpu"],
    ["securite", "Sécurité", "bouclier"], ["materiel", "Matériel", "ram"], ["partitions", "Partitions", "disque"],
  ];
  const D = { pages: {} };
  // « applications:gimp.desktop » : la page, puis ce qu'elle vise (le clic
  // droit « Désinstaller… » du menu Démarrer). Les autres n'ont que la page.
  const lireOnglet = (o) => {
    const t = String(o || ""); const i = t.indexOf(":");
    return i < 0 ? [t, ""] : [t.slice(0, i), t.slice(i + 1)];
  };
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

  // LE NAVIGATEUR PRINCIPAL (23 novembre). Grenofar : « si je veux me
  // connecter à Google, je choisis juste mon compte, celui enregistré dans
  // mon navigateur principal ». Tous les liens — et les « Se connecter avec
  // Google » de Steam, Heroic ou d'une application — s'ouvrent dans celui-ci.
  async function carteNavigateur() {
    const v = await R("reglages_navigateurs").catch(() => ({ navigateurs: [] }));
    const liste = v.navigateurs || [];
    if (!liste.length) return carte(ligne("Navigateur principal", el("span.note", "aucun navigateur installé")));
    const dire = el("p.note");
    const actuel = liste.find((n) => n.actuel);
    const options = actuel ? [] : [["", "Choisir…"]];
    const s = choix([...options, ...liste.map((n) => [n.id, n.nom])], actuel ? actuel.id : "", async (id) => {
      if (!id) return;
      const r = await R("reglages_navigateur_choisir", { identifiant: id });
      const nom = (liste.find((n) => n.id === id) || {}).nom || id;
      dire.textContent = r.souci || `${nom} ouvre maintenant tous les liens, et les connexions avec Google s'y font avec les comptes que tu y as enregistrés.`;
    });
    return carte(ligne("Navigateur principal", s), dire,
      note("Les liens et les pages de connexion (« Se connecter avec Google », Steam, Heroic…) s'ouvrent dans ce navigateur, là où tes comptes sont déjà enregistrés."));
  }

  // ---- Les pages ---------------------------------------------------------------
  const CONSTRUIRE = {
    partitions: () => pagePartitions(),
    applications: () => pageApplications(),
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
      const v = await R("reglages_ecran", { nom: D.ecranChoisi || "" });
      if (!v.ecran) return [titre("Écran"), affichage, carte(note("Aucun écran interrogeable ici."))].filter(Boolean);
      const e = v.ecran;
      // PLUSIEURS ÉCRANS (21 novembre). Grenofar : « fais qu'on puisse choisir
      // l'écran auquel changer les paramètres, ça ne fait que pour un seul ».
      const nomEcran = (x) => `${x.reference || x.nom}${x.principal ? " (principal)" : ""} — ${x.actuelle ? x.actuelle.replace("x", " × ") : "éteint"}${x.taux ? ` à ${Math.round(parseFloat(x.taux))} Hz` : ""}`;
      const choixEcran = (v.ecrans || []).length > 1
        ? ligne("Écran à régler", choix(v.ecrans.map((x) => [x.nom, nomEcran(x)]), e.nom, async (val) => {
          D.ecranChoisi = val;
          D.pages.ecran.replaceChildren(...(await CONSTRUIRE.ecran()).filter(Boolean));
        }))
        : null;
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
      return [titre("Écran", `${e.reference ? e.reference + " (" + e.nom + ")" : "Écran " + e.nom}, actuellement ${e.actuelle.replace("x", " × ") || "inconnu"} à ${e.taux ? Math.round(parseFloat(e.taux)) : "?"} images par seconde.`),
        affichage,
        carte(choixEcran,
          // SES LIMITES (21 novembre) : « la référence de l'écran, son nombre
          // d'images et sa résolution max, donc pas plus, pas possible ».
          e.max_frequence ? ligne("Ses limites", el("span.note", `${(e.max_definition || "").replace("x", " × ")} · jusqu'à ${e.max_frequence} images par seconde`)) : null,
          ligne("Définition", tailles), ligne("Images par seconde", taux), bouton("Appliquer", "principal", appliquer), confirmer, dire,
          // La disposition faite à la souris est GARDÉE en sortant (21
          // novembre) : avant, elle se perdait à la session suivante.
          ligne("Plusieurs écrans", bouton("Disposer les écrans", "", () => ouvrirProgramme("grenos-disposer-ecrans"))),
          v.virtuelle
            ? note(`grenOS tourne dans une machine virtuelle (${v.virtuelle}) : son écran est virtuel, et c'est ${v.virtuelle === "oracle" ? "VirtualBox" : "l'hyperviseur"} qui le recopie sur ton vrai écran, à sa propre cadence. Choisir une fréquence plus haute ici n'y changerait rien ; sur un vrai PC, toutes celles de ton écran sont proposées.`)
            : note("grenOS ne propose que ce que ton écran annonce : rien au-delà de ses limites. Après « Appliquer », rien n'est gardé tant que tu n'as pas cliqué « Garder » : si l'image disparaît, attends 15 secondes, l'ancien réglage revient tout seul."))].filter(Boolean);
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
        await carteNavigateur(),
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

  // ---- Partitions (23 novembre) ---------------------------------------------
  // Grenofar : « fais un système de réglage des partitions ». Chaque disque en
  // barre à l'échelle (partitions et espace libre), et les gestes sans
  // connaître Linux. Ce qui efface demande une seconde confirmation, et
  // l'outil administrateur (grenos-partitions) refuse de toute façon ce qui
  // fait tourner grenOS.
  const octets = (n) => {
    if (!n && n !== 0) return "?";
    const u = ["o", "Ko", "Mo", "Go", "To"];
    let i = 0; let v = n;
    while (v >= 1000 && i < u.length - 1) { v /= 1000; i += 1; }
    return `${v.toLocaleString("fr-FR", { maximumFractionDigits: v < 10 ? 1 : 0 })} ${u[i]}`;
  };
  const SYSTEMES = [["ext4", "ext4 — pour grenOS et Linux"], ["exfat", "exFAT — partagé avec Windows et Mac"], ["ntfs", "NTFS — pour Windows"], ["fat32", "FAT32 — vieux appareils (4 Go par fichier au plus)"]];
  const nomPartition = (p) => p.etiquette || (p.windows ? "Windows" : p.demarrage ? "Démarrage (EFI)" : p.nom);

  function confirmerPuis(texte, genre, action) {
    // Un premier clic arme, le second agit (et rien après 6 s).
    const b = bouton(texte, genre, async () => {
      if (!b.dataset.confirme) {
        b.dataset.confirme = "1";
        b.textContent = "Sûr ? Tout sera effacé — cliquer encore";
        setTimeout(() => { delete b.dataset.confirme; b.textContent = texte; }, 6000);
        return;
      }
      delete b.dataset.confirme;
      b.disabled = true;
      await action();
      b.disabled = false;
      b.textContent = texte;
    });
    return b;
  }

  async function pagePartitions(etatInitial) {
    const dire = el("p.note");
    const v = etatInitial || await R("reglages_partitions");
    const redessiner = async (r) => {
      D.pages.partitions.replaceChildren(...(await pagePartitions(r)).filter(Boolean));
      const d = D.pages.partitions.querySelector(".rg-dire-partitions");
      if (d) d.textContent = r.souci ? `Refusé : ${r.souci}` : r.fait || "";
    };
    const agir = async (geste, peripherique, extra) => {
      dire.textContent = "En cours… (le mot de passe administrateur peut être demandé)";
      try { await redessiner(await api.appeler("reglages_partitions_agir", { geste, peripherique, ...(extra || {}) })); }
      catch (e) { dire.textContent = String(e); }
    };
    const monter = async (chemin, oui) => {
      dire.textContent = oui ? "Montage…" : "Démontage…";
      try {
        const r = await R("reglages_partitions_monter", { chemin, monter: oui });
        await redessiner({ ...r, fait: r.souci ? "" : (oui ? "Montée." : "Démontée : tu peux la retirer sans risque.") });
      } catch (e) { dire.textContent = String(e); }
    };
    // Un petit formulaire déplié sous la ligne : système, nom, (taille).
    const formulaire = (titreGeste, avecTaille, max, envoyer) => {
      const fs = choix(SYSTEMES, "ext4", () => {});
      const nom = el("input.champ.rg-choix", { placeholder: "Nom (facultatif)", maxlength: 16 });
      const taille = avecTaille ? el("input.champ.rg-choix", { type: "number", min: 16, max: Math.floor(max / 1048576), placeholder: `Taille en Mo (vide = tout, ${octets(max)})` }) : null;
      const go = confirmerPuis(titreGeste, "danger", () => envoyer(fs.value, nom.value.trim(), taille ? taille.value.trim() : ""));
      return el("div.rg-part-form", ligne("Système de fichiers", fs), ligne("Nom", nom), taille ? ligne("Taille", taille) : null, go);
    };

    const carteDisque = (d) => {
      const barre = el("div.rg-part-barre");
      const morceaux = [...d.partitions.map((p) => ({ p, debut: p.debut, taille: p.taille })), ...d.libres.map((l) => ({ libre: l, debut: l.debut, taille: l.taille }))].sort((a, b) => a.debut - b.debut);
      for (const m of morceaux) {
        const part = el(`div.rg-part-morceau${m.libre ? ".libre" : ""}${m.p && m.p.systeme ? ".systeme" : ""}`,
          { style: `flex-grow:${Math.max(m.taille / d.taille, 0.012)}`, title: m.libre ? `Espace libre — ${octets(m.taille)}` : `${nomPartition(m.p)} — ${octets(m.taille)}` });
        if (m.p && m.p.utilise != null && m.p.taille) part.append(el("span.rg-part-rempli", { style: `width:${Math.min(100, 100 * m.p.utilise / m.p.taille)}%` }));
        barre.append(part);
      }
      const lignes = d.partitions.map((p) => {
        const details = el("div.rg-part-details");
        details.hidden = true;
        const ouvrirDetails = (contenu) => { details.replaceChildren(contenu); details.hidden = false; };
        const etiquettes = [p.systeme_fichiers || "non formatée", octets(p.taille)];
        if (p.utilise != null) etiquettes.push(`${octets(p.utilise)} utilisés`);
        if (p.montee) etiquettes.push(`montée sur ${p.montee}`);
        const gestes = el("div.rg-rangee");
        if (p.systeme) {
          gestes.append(el("span.badge", "grenOS tourne dessus — protégée"));
        } else {
          if (p.montee) {
            gestes.append(bouton("Ouvrir", "", () => api.appeler("fichiers_nouvelle_fenetre", { dossier: p.montee }).catch((e) => toast(String(e)))));
            gestes.append(bouton("Démonter", "", () => monter(p.chemin, false)));
          } else if (p.systeme_fichiers && !p.chiffree && !p.echange) {
            gestes.append(bouton("Monter", "", () => monter(p.chemin, true)));
          }
          if (p.systeme_fichiers && ["ext4", "ext3", "ext2", "exfat", "ntfs", "vfat"].includes(p.systeme_fichiers)) {
            gestes.append(bouton("Renommer", "discret", () => {
              const champ = el("input.champ.rg-choix", { value: p.etiquette, maxlength: 16 });
              ouvrirDetails(el("div.rg-part-form", ligne("Nouveau nom", champ), bouton("Renommer", "principal", () => agir("renommer", p.chemin, { nom: champ.value.trim() }))));
            }));
          }
          gestes.append(bouton("Formater…", "discret", () => ouvrirDetails(formulaire("Formater (efface tout)", false, 0, (systeme, nom) => agir("formater", p.chemin, { systeme, nom })))));
          gestes.append(confirmerPuis("Supprimer", "danger", () => agir("supprimer", p.chemin)));
        }
        const avertir = p.windows ? note("Partition de Windows : la formater ou la supprimer empêche Windows de démarrer.")
          : (p.demarrage && !p.systeme) ? note("Partition de démarrage d'un autre système : y toucher peut l'empêcher de démarrer.") : null;
        return el("div.rg-part-ligne",
          el("div.rg-part-titre", el("b", nomPartition(p)), el("span.note", ` ${p.chemin} · ${etiquettes.join(" · ")}`)),
          avertir, gestes, details);
      });
      const libres = d.libres.map((l) => {
        const details = el("div.rg-part-details");
        details.hidden = true;
        return el("div.rg-part-ligne.libre",
          el("div.rg-part-titre", el("b", "Espace libre"), el("span.note", ` ${octets(l.taille)}`)),
          el("div.rg-rangee", bouton("Créer une partition…", "", () => {
            details.replaceChildren(formulaire("Créer la partition", true, l.taille, (systeme, nom, taille) => agir("creer", d.chemin, { systeme, nom, taille })));
            details.hidden = false;
          })), details);
      });
      // Une clé formatée d'un bloc, sans table : le disque est la partition.
      const entier = d.entier && !d.systeme ? el("div.rg-part-ligne",
        el("div.rg-part-titre", el("b", "Disque entier"), el("span.note", ` ${d.systeme_fichiers} · ${octets(d.taille)}`)),
        el("div.rg-rangee", bouton("Formater…", "discret", (ev) => {
          const zone = ev.target.closest(".rg-part-ligne");
          zone.append(formulaire("Formater (efface tout)", false, 0, (systeme, nom) => agir("formater", d.chemin, { systeme, nom })));
        }))) : null;
      return carte(
        ligne(`${d.modele}${d.liaison ? " · " + d.liaison : ""}`, el("span.note", `${d.chemin} · ${octets(d.taille)}${d.table ? " · " + d.table.toUpperCase().replace("DOS", "MBR") : ""}${d.systeme ? " · disque de grenOS" : ""}${d.amovible ? " · amovible" : ""}`)),
        barre, entier, ...lignes, ...libres,
        !d.partitions.length && !d.libres.length && !d.entier ? note("Ce disque n'a ni partition ni espace libre lisible.") : null);
    };

    return [titre("Partitions", "Tes disques, ce qu'ils contiennent, et l'espace libre."),
      carte(el("p.note.rg-dire-partitions"), dire,
        el("div.rg-rangee",
          bouton("Actualiser", "", async () => redessiner(await R("reglages_partitions"))),
          bouton("Redimensionner (GParted)", "discret", () => ouvrirProgramme("gparted")),
          bouton("Disques (santé, images)", "discret", () => ouvrirProgramme("gnome-disks"))),
        note("Formater et supprimer effacent tout ce que la partition contient. Ce qui fait tourner grenOS est protégé. Pour agrandir ou réduire une partition, GParted le fait sans rien perdre.")),
      ...(v.disques || []).map(carteDisque),
      (v.disques || []).length ? null : carte(note("Aucun disque lisible ici."))];
  }

  // ---- Applications (26 novembre) ---------------------------------------------
  // Grenofar : « fais qu'on puisse désinstaller des apps, depuis paramètres ou
  // clic droit sur le dock quand on appuie sur Windows ». Ce que la personne a
  // ajouté (desinstaller.rs : les Flatpak, et les paquets Debian qui ont une
  // entrée dans le menu), un bouton par application, et une confirmation qui
  // dit ce qui partira avec. Ce qui fait grenOS est rangé à part, sans bouton.
  // Le clic droit du menu Démarrer ouvre cette page sur l'application, sa
  // confirmation déjà dépliée.
  async function pageApplications() {
    const v = await api.appeler("reglages_applications");
    const cible = D.cibleAppli || "";
    D.cibleAppli = "";
    const dire = el("p.note.rg-note");
    const compte = el("span.note");
    const liste = el("div.rg-apps");
    const lignes = [];
    const compter = () => {
      // Dans la liste, pas « dans la page » : à la construction, rien ne l'est encore.
      const n = lignes.filter((l) => l.bloc.parentNode === liste).length;
      compte.textContent = n ? `${n} application${n > 1 ? "s" : ""}` : "";
      if (!n) liste.replaceChildren(note("Rien d'installé en plus de grenOS pour l'instant : ce que tu ajoutes depuis GrenPlace apparaîtra ici."));
    };

    const ligneAppli = (a) => {
      const zone = el("div.rg-app-confirmer");
      zone.hidden = true;
      const meta = [a.source === "flatpak" ? "Flatpak" : "Paquet Debian", a.taille, a.version ? `version ${a.version}` : ""].filter(Boolean).join(" · ");
      const bloc = el("div.rg-app-bloc");
      const deplier = async () => {
        zone.hidden = false;
        zone.replaceChildren(note("Ce qui partirait avec…"));
        const ap = await api.appeler("reglages_desinstaller_apercu", { source: a.source, identifiant: a.identifiant, installation: a.installation })
          .catch((e) => ({ refus: String(e) }));
        if (ap.refus) { zone.replaceChildren(note(ap.refus)); return; }
        const donnees = ap.donnees ? el("input", { type: "checkbox" }) : null;
        const direIci = el("p.note.rg-note");
        const go = bouton(`Désinstaller ${a.nom}`, "danger", async () => {
          go.disabled = annuler.disabled = true;
          direIci.textContent = a.source === "apt" ? "Désinstallation… le mot de passe administrateur peut être demandé." : "Désinstallation…";
          try {
            const r = await api.appeler("reglages_desinstaller", {
              source: a.source, identifiant: a.identifiant, installation: a.installation, donnees: !!(donnees && donnees.checked),
            });
            if (r.fait) {
              bloc.remove();
              dire.textContent = r.fait + (r.raccourcis ? " Son raccourci sur le bureau aussi." : "");
              toast(r.fait);
              compter();
              return;
            }
            direIci.textContent = r.souci;
            zone.querySelector("details")?.remove();
            if (r.detail) zone.append(el("details", el("summary", "Voir le détail"), el("pre.journal.rg-journal", r.detail)));
          } catch (e) { direIci.textContent = String(e); }
          go.disabled = annuler.disabled = false;
        });
        const annuler = bouton("Annuler", "discret", () => { zone.hidden = true; });
        zone.replaceChildren(...[
          el("p.rg-note", el("b", `Désinstaller ${a.nom} ?`), " ",
            a.source === "flatpak" ? "L'application part, avec les bibliothèques que plus rien n'utilise." : "Le programme part ; tes documents restent."),
          ap.avec && ap.avec.length ? note(`Partira avec : ${ap.avec.join(", ")}.`) : null,
          donnees ? el("label.rg-app-case", donnees,
            el("span", `Supprimer aussi ses données${ap.taille_donnees ? ` (${ap.taille_donnees})` : ""} : ses réglages, les comptes qui y sont connectés, et ce qu'elle a téléchargé.`)) : null,
          el("div.rg-rangee", go, annuler), direIci].filter(Boolean));
      };
      const icone = a.icone ? el("img.rg-app-icone", { src: a.icone, alt: "" }) : el("span.rg-app-icone.vide", svg("paquet"));
      bloc.append(
        el("div.rg-app", icone,
          el("div.rg-app-texte", el("b", a.nom), a.description ? el("span.note", a.description) : null, el("span.note.rg-app-meta", meta)),
          bouton("Désinstaller", "", () => (zone.hidden ? deplier() : (zone.hidden = true)))),
        zone);
      lignes.push({ a, bloc, deplier });
      return bloc;
    };

    liste.append(...(v.applications || []).map(ligneAppli));
    compter();
    const recherche = el("input.champ.rg-choix", { type: "search", placeholder: "Rechercher une application" });
    recherche.addEventListener("input", () => {
      const f = recherche.value.trim().toLowerCase();
      for (const l of lignes) l.bloc.hidden = !!f && !`${l.a.nom} ${l.a.description} ${l.a.identifiant}`.toLowerCase().includes(f);
    });

    if (cible) {
      const l = lignes.find((x) => (x.a.entrees || []).includes(cible));
      if (l) {
        l.bloc.classList.add("vise");
        setTimeout(() => l.bloc.scrollIntoView({ block: "center" }), 60);
        l.deplier();
      } else {
        dire.textContent = (v.livrees_entrees || []).includes(cible)
          ? "Cette application fait partie de grenOS : elle ne se désinstalle pas, le bureau en dépend."
          : "Cette application ne se désinstalle pas d'ici : elle n'a été installée ni par GrenPlace, ni par Flatpak, ni par apt.";
      }
    }
    const livrees = v.livrees || [];
    return [titre("Applications", "Ce que tu as installé, et de quoi le désinstaller."),
      carte(el("div.rg-ligne", recherche, compte), dire, liste),
      livrees.length ? carte(el("details", el("summary", `Livrées avec grenOS (${livrees.length})`), el("p.rg-diag", livrees.join(" · "))),
        note("Elles font partie de grenOS : le bureau, la barre ou l'installation en dépendent. Elles se mettent à jour avec le système, mais ne se désinstallent pas.")) : null,
      carte(ligne("Installer d'autres applications", bouton("Ouvrir GrenPlace", "", () => ouvrirProgramme("grenplace"))),
        note("Un clic droit sur une application du menu Démarrer propose aussi « Désinstaller… »."))];
  }

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

  window.grenosOnglet = (onglet) => {
    const [cle, cible] = lireOnglet(onglet);
    if (!D.pages[cle]) return;
    // Fenêtre déjà ouverte, une autre application visée : la page se refait.
    if (cible) { D.cibleAppli = cible; delete D.pages[cle].dataset.garnie; }
    montrer(cle);
  };

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
    const [demande, cible] = lireOnglet(window.GRENOS_ONGLET);
    if (cible) D.cibleAppli = cible;
    const depart = PROPOSER_MAX ? "ecran"
      : PAGES.some((p) => p[0] === demande) ? demande : "apparence";
    montrer(depart);
    pret();
    api.appeler("reglages_ouvert");
    // Les autres, une par une, quand rien ne presse.
    for (const [cle] of PAGES) await garnir(cle);
  })();
})();
