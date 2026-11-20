// Données factices pour regarder le dessin dans un navigateur, hors de Tauri.
// Ne sert JAMAIS dans grenos-ui : `window.__TAURI__` y existe, et ce fichier
// ne fait alors rien.
"use strict";
(function () {
  if (window.__TAURI__) return;
  const attendre = (ms) => new Promise((r) => setTimeout(r, ms));
  const DONNEES = {
    ecrans: ["bienvenue", "langue", "clavier", "compte", "parcours", "reseau", "apparence", "usage", "confidentialite", "securite", "majs", "applications", "sauvegardes", "optimisation", "systeme", "pare-feu", "journaux", "pilotes", "final"]
      .map((cle) => ({ cle, titre: cle, parcours: ["bienvenue", "langue", "clavier", "compte", "parcours", "pilotes", "final"].includes(cle) ? ["rapide", "personnalise", "dev"] : ["systeme", "pare-feu", "journaux"].includes(cle) ? ["dev"] : ["personnalise", "dev"] })),
    usages: [
      { cle: "bureautique", titre: "Bureautique et Internet", note: "Écrire, naviguer, regarder des vidéos.", paquets: [] },
      { cle: "developpement", titre: "Développement", note: "Visual Studio Code, Git, Python et Node.", paquets: ["vscode"] },
      { cle: "jeu", titre: "Jeux", note: "Steam, et Heroic pour les jeux Epic.", paquets: ["steam"] },
      { cle: "montage", titre: "Montage photo, vidéo et son", note: "Kdenlive, GIMP et Audacity.", paquets: ["gimp"] },
    ],
    claviers: [
      { nom: "France (AZERTY)", disposition: "fr", variante: "", fuseau: "Europe/Paris" },
      { nom: "Belgique (AZERTY)", disposition: "be", variante: "", fuseau: "Europe/Brussels" },
      { nom: "Canada — Québec", disposition: "ca", variante: "", fuseau: "America/Montreal" },
      { nom: "États-Unis (QWERTY)", disposition: "us", variante: "", fuseau: "America/New_York" },
    ],
  };
  window.APERCU = async function (commande, a) {
    switch (commande) {
      case "premier_infos": return { donnees: DONNEES, machine: "grenos", virtuelle: "", secure_boot: "actif", pilote_nvidia: true,
        secure_boot_phrase: "Secure Boot est désactivé. grenOS démarre aussi bien avec : tu peux l'activer dans le BIOS quand tu veux, rien ne cassera.",
        materiel: [
          { icone: "cpu", nom: "Processeur", valeur: "8 cœur(s)", detail: "AMD Ryzen 7 5800X" },
          { icone: "ram", nom: "Mémoire", valeur: "15,6 Go", detail: "" },
          { icone: "gpu", nom: "Carte graphique", valeur: "NVIDIA GeForce RTX 3060", detail: "nouveau" },
          { icone: "disque", nom: "Stockage", valeur: "SSD NVMe 1000 Go", detail: "" },
          { icone: "wifi", nom: "Wi-Fi", valeur: "présent", detail: "" },
          { icone: "bt", nom: "Bluetooth", valeur: "absent", detail: "" },
          { icone: "son", nom: "Son", valeur: "HDA Intel PCH", detail: "" }] };
      case "premier_nom": {
        const login = (a.texte || "").toLowerCase().normalize("NFKD").replace(/[̀-ͯ]/g, "").replace(/ /g, "-").replace(/[^a-z0-9-]/g, "").replace(/^-+|-+$/g, "");
        return { login, adresse: login ? login + "@grenos" : "", probleme: !login ? "Un nom, avec au moins une lettre." : /^[a-z]/.test(login) ? null : "Un nom commence par une lettre." };
      }
      case "reseau_etat": await attendre(300); return { connecte: false, wifi: true, gestionnaire: true, nom: "" };
      case "reseau_liste": await attendre(700); return [
        { nom: "Livebox-4F2A", signal: 86, protege: true, actif: false },
        { nom: "Freebox-Salon", signal: 61, protege: true, actif: false },
        { nom: "Café du coin", signal: 32, protege: false, actif: false }];
      case "premier_terminer": await attendre(2200); return a.reponses.compte + "@grenos";
      case "apparence": return { theme: "nuit", accent: "violet", effets: "equilibres" };
      case "maj_etat": return { disponibles: 37, flatpak: 3, date: "2026-11-17 08:12", telechargee: true, arme: false, mode: "automatiques", dernier_resultat: "ok 12", en_cours: false,
        paquets: ["linux-image-amd64", "firefox-esr", "libwebkit2gtk-4.1-0", "grenos-desktop", "mesa-vulkan-drivers", "openssl", "systemd", "pipewire"] };
      case "maj_preparer": {
        const e = window.APERCU_ECOUTE || {};
        (async () => {
          for (const [p, t] of [[5, "Lecture des dépôts…"], [25, "37 paquets à télécharger…"], [60, "Téléchargement : 20 sur 37"], [88, "Applications Flatpak…"], [96, "Préparation du redémarrage…"]]) {
            e["maj-avance"] && e["maj-avance"]({ part: p, texte: t });
            e["maj-ligne"] && e["maj-ligne"]("Get:" + p + " http://deb.debian.org/debian trixie/main amd64 paquet");
            await attendre(700);
          }
          e["maj-fin"] && e["maj-fin"]({ bien: true, texte: "37 paquets prêts. Redémarre pour les installer.", arme: true });
        })();
        return null;
      }
      case "perf_instantane": {
        const g = (nom, description, n, mo, cpu, a_moi = true, protege = false, en_trop = false) =>
          ({ nom, description, pids: [1], nombre: n, memoire_mo: mo, cpu: cpu * (0.7 + Math.random() * 0.6), a_moi, protege, en_trop, priorite: 0 });
        return { coeurs: 8, cpu: 18 + Math.random() * 20, memoire_totale_go: 15.6, memoire_dispo_go: 9.1, memoire_cache_go: 3.4,
          echange_total_go: 2, echange_utilise_go: 0.2, charge: "0.84 0.71 0.66", mode_jeu: !!window.MODE_JEU, groupes: [
            g("firefox-esr", "Navigateur Firefox", 14, 1840, 6.2), g("steam", "Steam", 6, 610, 1.1), g("Discord", "Discord", 5, 520, 0.8),
            g("tracker-miner-fs-3", "Indexation des fichiers", 1, 180, 3.5, true, false, true), g("code", "Visual Studio Code", 9, 960, 0.6),
            g("grenos-shell", "La barre de grenOS", 1, 64, 0.3, true, true), g("pipewire", "Le son", 1, 22, 0.4, true, true),
            g("Xorg", "Serveur graphique", 1, 140, 1.2, false, true), g("NetworkManager", "Le réseau", 1, 18, 0.1, false, true)] };
      }
      case "perf_demarrage": return [
        { id: "discord.desktop", nom: "Discord", commentaire: "Démarre réduit dans la barre", actif: true, systeme: false },
        { id: "xdg-user-dirs.desktop", nom: "Dossiers personnels", commentaire: "Met à jour les dossiers Bureau, Documents…", actif: true, systeme: true },
        { id: "blueman.desktop", nom: "Bluetooth", commentaire: "", actif: false, systeme: true }];
      case "perf_mode_jeu": window.MODE_JEU = a.actif; return { fermes: a.fermer, ralentis: 12, message: a.actif ? `Mode jeu actif : ${a.fermer.length} application(s) fermée(s), 12 processus passés en arrière-plan.` : "Mode jeu coupé : tout a retrouvé sa priorité." };
      case "perf_liberer": await attendre(800); return { avant_go: 9.1, apres_go: 9.6, message: "0,5 Go de plus immédiatement disponibles, mémoire compactée." };
      case "perf_fermer": await attendre(500); return 3;
      case "perf_priorite": return 3;
      case "lance_vue": {
        const J = (cle, nom, genre, etat, source, couleurs, extra = {}) => ({ cle, nom, genre, etat, source, couleurs, note: "Note de démonstration pour regarder la mise en page de la fiche.", installe: false, ...extra });
        return { steam: true, heroic: false,
          plateformes: [
            { cle: "steam", nom: "Steam", note: "La plus grande bibliothèque de jeux, et Proton.", identifiant: "com.valvesoftware.Steam", couleurs: ["#1b2838", "#2a475e"], installe: true },
            { cle: "heroic", nom: "Heroic", note: "Tes jeux Epic Games et GOG.", identifiant: "com.heroicgameslauncher.hgl", couleurs: ["#0b1220", "#7c3aed"], installe: false },
            { cle: "lutris", nom: "Lutris", note: "Battle.net, EA, Ubisoft…", identifiant: "net.lutris.Lutris", couleurs: ["#3d1d0a", "#f59e0b"], installe: false }],
          jeux: [
            J("rocket-league", "Rocket League", "Sport · Gratuit", "proton", "heroic", ["#0a2a6b", "#f97316"], { identifiant: "Sugar", steam: 252950, note: "Gratuit sur Epic, donc par Heroic. Easy Anti-Cheat est activé pour Linux : les parties classées marchent." }),
            J("fortnite", "Fortnite", "Battle royale · Gratuit", "nuage", "nuage", ["#1e1b4b", "#a855f7"], { adresse: "https://www.xbox.com/play" }),
            J("roblox", "Roblox", "Bac à sable · Gratuit", "natif", "flatpak", ["#111827", "#e11d48"], { identifiant: "org.vinegarhq.Sober" }),
            J("cs2", "Counter-Strike 2", "Tir tactique · Gratuit", "natif", "steam", ["#1f2937", "#f59e0b"], { steam: 730, installe: true }),
            J("elden-ring", "Elden Ring", "Action-RPG", "proton", "steam", ["#1c1917", "#d4a017"], { steam: 1245620 }),
            J("supertuxkart", "SuperTuxKart", "Course · Libre", "natif", "flatpak", ["#1e3a8a", "#22d3ee"], { identifiant: "net.supertuxkart.SuperTuxKart" }),
            J("valorant", "Valorant", "Tir tactique", "indisponible", "aucune", ["#3f0d12", "#ff4655"], { logo: "https://cmsassets.rgpub.io/sanity/images/dsfx7636/news/cbf4460132cdfeb2a97fad5f9dd25ba0bc058f76-128x128.png" })],
          bibliotheque: [
            { id: "730", nom: "Counter-Strike 2", origine: "steam", taille_go: 33.2, image: null },
            { id: "1086940", nom: "Baldur's Gate 3", origine: "steam", taille_go: 122.4, image: null },
            { id: "413150", nom: "Stardew Valley", origine: "steam", taille_go: 0.6, image: null }],
          compte: [
            { id: "1245620", nom: "ELDEN RING", origine: "steam-compte", taille_go: 0, image: null },
            { id: "292030", nom: "The Witcher 3: Wild Hunt", origine: "steam-compte", taille_go: 0, image: null },
            { id: "1145360", nom: "Hades", origine: "steam-compte", taille_go: 0, image: null }] };
      }
      case "lance_installer": {
        const e = window.APERCU_ECOUTE || {};
        (async () => { for (const p of [12, 40, 77, 100]) { await attendre(500); e["lance-progres"] && e["lance-progres"]({ identifiant: a.identifiant, part: p, texte: `Téléchargement ${p} %` }); }
          e["lance-fin"] && e["lance-fin"]({ identifiant: a.identifiant, bien: true, texte: "Installé." }); })();
        return null;
      }
      case "arret_vue": return { adresse: "grenofar@grenos", gestes: [
        { cle: "maj", titre: "Mettre à jour et redémarrer", note: "Poser 12 mises à jour avant d'ouvrir le bureau, puis redémarrer.", genre: "accent", icone: "maj" },
        { cle: "session", titre: "Fermer la session", note: "Revenir à l'écran de connexion.", genre: "", icone: "compte" },
        { cle: "veille", titre: "Mettre en veille", note: "La machine s'éteint presque entièrement, et retrouve tout au réveil.", genre: "", icone: "nuit" },
        { cle: "redemarrer", titre: "Redémarrer", note: "Éteindre puis rallumer la machine.", genre: "", icone: "redemarrer" },
        { cle: "eteindre", titre: "Éteindre", note: "Arrêter complètement la machine.", genre: "danger", icone: "eteindre" }] };
      case "arret_faire": await attendre(200); return null;
      case "bienvenue_vue": return { salut: "Bonjour Léa", adresse: "lea@grenos", astuces: ["Clic droit sur le bureau : créer un dossier, un fichier, changer le fond.", "Alt+Tab passe d'une fenêtre à l'autre, Alt+F4 ferme celle du dessus.", "Ctrl+Maj+Échap ouvre le gestionnaire de tâches, Ctrl+Alt+Échap le mode jeu."],
        gestes: [["GrenPlace", "Ajouter des applications en un clic — Steam, un traitement de texte, un éditeur de code.", "grenplace", "paquet"], ["Jeux", "Ce que cette machine sait faire tourner, et ce qu'il lui manque.", "grenos-jeux", "manette"], ["Connexions", "Le Wi-Fi et le Bluetooth, dans une fenêtre.", "grenos-connexions", "wifi"], ["Réglages", "Le thème jour et nuit, l'écran, le son, l'heure.", "grenos-parametres", "reglages"], ["Tes fichiers", "Ouvrir, renommer, copier, supprimer.", "grenos-fichiers", "dossier"], ["Terminal", "Pour qui veut taper des commandes.", "x-terminal-emulator", "terminal"]].map(([nom, note, programme, icone]) => ({ nom, note, programme, icone })) };
      case "secours_vue": return { echecs: 2, cause: "iwlwifi 0000:00:14.3: probe with driver iwlwifi failed with error -110", propositions: [
        { cle: "analyser", intitule: "Analyser la machine", possible: true, pourquoi: "" }, { cle: "journal", intitule: "Voir ce que le dernier démarrage a dit", possible: true, pourquoi: "" },
        { cle: "instantane", intitule: "Restaurer un instantané", possible: false, pourquoi: "aucun disque d'instantanés n'est encore choisi" }, { cle: "pilote", intitule: "Désactiver « iwlwifi »", possible: true, pourquoi: "" }] };
      case "connexions_etat": await attendre(400); return { lien: { connecte: true, par: "Wi-Fi", nom: "Livebox-4F2A" }, wifi_la: true, wifi_on: true, bt_la: true, bt_on: true };
      case "connexions_wifi_liste": await attendre(500); return [{ nom: "Livebox-4F2A", force: 82, protege: true, connecte: true }, { nom: "Freebox-Salon", force: 55, protege: true, connecte: false }, { nom: "Café du coin", force: 20, protege: false, connecte: false }];
      case "connexions_bt_liste": return [{ adresse: "AA:BB:CC:DD:EE:FF", nom: "Casque Sony WH-1000XM5", connecte: true }, { adresse: "11:22:33:44:55:66", nom: "Manette Xbox", connecte: false }];
      case "taches_mesure": return { total: 214, coeurs: 8, cpu: 12 + Math.random() * 10, memoire_utilisee_go: 6.2, memoire_totale_go: 15.6, lignes: [
        { pid: 1834, nom: "firefox-esr", cpu: 6.2, memoire_mo: 1240 }, { pid: 2210, nom: "grenos-ui", cpu: 1.1, memoire_mo: 96 }, { pid: 980, nom: "Xorg", cpu: 2.4, memoire_mo: 140 },
        { pid: 1203, nom: "grenos-shell-natif", cpu: 0.3, memoire_mo: 38 }, { pid: 1404, nom: "pipewire", cpu: 0.4, memoire_mo: 22 }, { pid: 3001, nom: "steam", cpu: 0.8, memoire_mo: 610 }] };
      case "hub_etat": return { sondes: 2, ventilateurs: 1, paves: [
        { titre: "La machine", lignes: [["Nom", "grenos"], ["Version", "grenOS 20261118-1129"], ["Noyau Linux", "6.12.48"], ["Allumée depuis", "2 h 14 min"], ["Genre", "machine réelle"]] },
        { titre: "Processeur et mémoire", lignes: [["Processeur", "AMD Ryzen 7 5800X"], ["Cœurs", "16"], ["Mémoire vive", "15,6 Go, dont 9,1 Go libres"]] },
        { titre: "Disques", lignes: [["nvme0n1", "SSD NVMe de 1000 Go, Samsung 980"]] },
        { titre: "Services", lignes: [["Tout va bien", "aucun service en échec"]] }] };
      case "hub_diagnostic": {
        const e = window.APERCU_ECOUTE || {};
        (async () => {
          for (const [nom, etat, detail] of [["Processeur", "bon", "16 cœurs ont calculé sans erreur"], ["Mémoire", "bon", "64 Mo écrits et relus à l'identique"], ["Stockage", "a surveiller", "écriture lente : 38 Mo/s"], ["Réseau", "bon", "deb.debian.org répond"]]) {
            await attendre(500); e["hub-ligne"] && e["hub-ligne"]({ nom, etat, detail });
          }
          e["hub-fin"] && e["hub-fin"]({ fin: true, etat: "a surveiller", phrase: "Un point à surveiller : le stockage.", points: 4 });
        })();
        return null;
      }
      case "grenplace_catalogue": await attendre(300); return { origine: "en ligne", applications: [
        { slug: "steam", nom: "Steam", rayon: "Jeux", editeur: "Valve", description: "Des milliers de jeux, dont beaucoup de jeux Windows qui tournent ici.", logo: "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/com.valvesoftware.Steam.png", source: "flatpak", identifiant: "com.valvesoftware.Steam", installe: true },
        { slug: "gimp", nom: "GIMP", rayon: "Création", editeur: "GIMP", description: "Retoucher des photos, dessiner.", logo: "", source: "flatpak", identifiant: "org.gimp.GIMP", installe: false },
        { slug: "libreoffice", nom: "LibreOffice", rayon: "Bureautique", editeur: "The Document Foundation", description: "Textes, tableurs, présentations.", logo: "", source: "apt", identifiant: "libreoffice", installe: false }] };
      case "grenplace_retards": await attendre(900); return ["steam"];
      case "installer": {
        const e = window.APERCU_ECOUTE || {};
        (async () => { for (const p of [10, 35, 70, 100]) { await attendre(400); e.installation && e.installation({ cle: a.cle, part: p, ligne: `Téléchargement ${p} %`, texte: null, fin: false, bien: false }); }
          e.installation && e.installation({ cle: a.cle, part: 100, ligne: null, texte: `${a.nom} est installé. Il est dans le menu.`, fin: true, bien: true }); })();
        return null;
      }
      case "jeux_vue": await attendre(500); return { coeurs: 16, memoire: 15.6, carte: "NVIDIA GeForce RTX 3060", pilote: "nouveau", virtuelle: false, vulkan: true, vulkan_detail: "NVIDIA GeForce RTX 3060 (NVK)",
        pilote_paquet: "nvidia-driver", pilote_conseil: "Cette carte est une NVIDIA, et le pilote libre en cours ne sait pas jouer. Le pilote de NVIDIA le sait. Il faut redémarrer après.",
        gamemode_pose: true, gamemode_arme: false, option_steam: "gamemoderun %command%", heroic: "com.heroicgameslauncher.hgl", heroic_pose: false, rocket_prete: false, roblox_pose: true, fortnite_pose: false,
        outils: [{ nom: "Steam", note: "La boutique et la couche Proton.", source: "flatpak", identifiant: "com.valvesoftware.Steam", installe: true }, { nom: "MangoHud", note: "Les images par seconde par-dessus le jeu.", source: "apt", identifiant: "mangohud", installe: false }],
        jeux: [{ nom: "Rocket League", etat: "marche", note: "Fonctionne sur Linux depuis avril 2026.", action: "steam", adresse: "steam://store/252950" }, { nom: "Fortnite", etat: "en nuage", note: "Epic refuse d'activer l'anti-triche Linux.", action: "nuage" }, { nom: "Roblox", etat: "marche", note: "Par Sober, de VinegarHQ.", action: "flatpak", identifiant: "org.vinegarhq.Sober" }] };
      case "reglages": switch (a.fonction) {
        case "reglages_ecran_appliquer": return { souci: "", precedent: { taille: "1920x1080", brut: "60.00" }, mode: a.arguments && a.arguments.taille, taux: a.arguments && a.arguments.brut };
        case "reglages_ecran_garder": case "reglages_ecran_revenir": return { souci: "" };
        case "reglages_affichage": return { mode: "firmware", rtx50: true, choix: "firmware", essai: false, peut_reessayer: true };
        case "reglages_affichage_reessayer": case "reglages_affichage_confirmer": return { souci: "" };
        case "reglages_ecran": return { ecran: { nom: "HDMI-1", actuelle: "1920x1080", taux: "60.00" }, modes: [{ taille: "2560x1440", taux: [{ arrondi: 60, brut: "60.00", annoncee: true }] }, { taille: "1920x1080", taux: [{ arrondi: 144, brut: "144.00", annoncee: true }, { arrondi: 60, brut: "60.00", annoncee: true }, { arrondi: 75, brut: "75", annoncee: false }] }] };
        case "reglages_son": return { sorties: [["a", "Stéréo analogique"], ["b", "Stéréo numérique (HDMI)"]], courante: "a", volume: 62, muet: false, diagnostic: "La sortie par défaut est branchée et ouverte.\nLe mélangeur de la carte est ouvert (Master 80 %).\nPipeWire tourne en temps réel : le son tiendra quand la machine peine.\nCarte son : HDA Intel PCH, pilotée par snd_hda_intel." };
        case "reglages_heure": return { fuseau: "Europe/Paris", automatique: true, fuseaux: ["Europe/Brussels", "Europe/Paris", "America/Montreal"] };
        case "reglages_maj": return { version: "grenOS 20261118-1129", resume: "3 paquets à installer. Vérifié le 2026-11-18 08:12 UTC." };
        case "reglages_composants": await attendre(800); return { present: true, composants: [] };
        case "reglages_securite": return { resume: "ClamAV est installé. Signatures absentes : rien n'est encore analysable.", present: true, signatures: false };
        case "reglages_materiel": return { lignes: [["Processeur", "16 cœurs"], ["Mémoire vive", "15,6 Go (9,1 Go libres)"], ["Carte graphique", "NVIDIA GeForce RTX 3060"]], explication: "grenOS utilise d'office tout ce que la machine lui donne." };
        default: return {};
      }
      case "reglages_reseau": return "connecté";
      case "reglages_compte": return { protege: false, adresse: "lea@grenos", nom: "Léa" };
      case "reglages_parcourir_fond": return "/usr/share/backgrounds/aurore.png";
      case "maj_mode": case "maj_verifier": await attendre(300); return null;
      case "reglages_dpi_lire": return 96;
      case "reglages_cle_etat": return { outil: true, cles: window.APERCU_CLES || 0 };
      case "reglages_cle_ajouter": await attendre(900); window.APERCU_CLES = (window.APERCU_CLES || 0) + 1; return "Clé enregistrée. À l'ouverture de session et pour les gestes d'administrateur, toucher la clé suffit ; le mot de passe marche toujours.";
      case "reglages_cle_retirer": window.APERCU_CLES = 0; return "Clés retirées. Le mot de passe reste le seul moyen.";
      case "grenai_claude_code": return null;
      case "grenai_mcp_etat": return { branche: !!window.APERCU_MCP, desktop: false, code_installe: false, serveur: true };
      case "grenai_mcp": window.APERCU_MCP = a && a.brancher; return a && a.brancher ? "C'est branché : Claude peut maintenant voir l'écran, piloter la souris et le clavier, et lancer des commandes sur cette machine." : "grenOS est débranché de Claude.";
      case "reglages_dispositions_lire": return true;
      case "reglages_dispositions": return "En faisant glisser une fenêtre vers le haut, la barre des dispositions apparaît.";
      case "reglages_barre_lire": return window.APERCU_BARRE || "dock";
      case "reglages_barre": window.APERCU_BARRE = a.disposition; return a.disposition === "dock" ? "Dock : la barre flotte, centrée, et ne prend que la place de ce qu'elle porte." : "Barre : pleine largeur, collée en bas de l'écran.";
      case "reglages_dpi": return `Taille ${Math.round(a.dpi * 100 / 96)} %.`;
      case "reglages_fonds": return [{ nom: "Images/montagne.jpg", chemin: "/home/lea/Images/montagne.jpg", actuel: true }, { nom: "Bureau/plage.png", chemin: "/home/lea/Bureau/plage.png", actuel: false }];
      case "fichiers_maison": return "/home/lea";
      case "fichiers_rapides": return [["Bureau", "bureau"], ["Téléchargements", "telechargement"], ["Documents", "texte"], ["Images", "image"], ["Musique", "audio"], ["Vidéos", "video"]].map(([nom, icone]) => ({ nom, icone, chemin: "/home/lea/" + nom, groupe: "" })).concat([["Téléchargements", "telechargement", "Downloads"], ["Images", "image", "Pictures"], ["Vidéos", "video", "Videos"]].map(([nom, icone, d]) => ({ nom, icone, chemin: "/media/windows/Users/lea/" + d, groupe: "windows" })));
      case "fichiers_ce_pc": return [{ nom: "Léa", chemin: "/home/lea", icone: "compte" }, { nom: "Système de fichiers", chemin: "/", icone: "disque" }, { nom: "Clé USB 32 Go", chemin: null, peripherique: "/dev/sdb1", icone: "disque", fs: "vfat", conseil: "FAT32 : un fichier de plus de 4 Go n'y entre pas, et une coupure peut l'abîmer. Pour une clé partagée avec Windows et Mac, exFAT ; pour grenOS seul, ext4 — Disques › Formater (cela efface la clé)." }];
      case "fichiers_lister": {
        const E = (nom, dossier, famille, genre, octets) => ({ nom, chemin: a.dossier.replace(/\/$/, "") + "/" + nom, dossier, famille, genre, octets, modifie: 1795000000 - octets % 100000 });
        window.APERCU_FICHIERS = window.APERCU_FICHIERS || [E("Bureau", true, "dossier", "Dossier de fichiers", 0), E("Documents", true, "dossier", "Dossier de fichiers", 0), E("Images", true, "dossier", "Dossier de fichiers", 0),
          E("rapport-final.pdf", false, "pdf", "Document PDF", 2480000), E("vacances.jpg", false, "image", "Image JPEG", 4210000), E("projet.zip", false, "archive", "Archive Zip", 15800000), E("notes.txt", false, "texte", "Document texte brut", 1200)];
        return { dossier: a.dossier, caches: 3, elements: window.APERCU_FICHIERS.map((x) => ({ ...x, chemin: a.dossier.replace(/\/$/, "") + "/" + x.nom })) };
      }
      case "fichiers_nouveau_dossier": {
        if ((window.APERCU_FICHIERS || []).some((x) => x.nom === a.nom)) throw `« ${a.nom} » existe déjà ici.`;
        window.APERCU_FICHIERS.push({ nom: a.nom, dossier: true, famille: "dossier", genre: "Dossier de fichiers", octets: 0, modifie: 1795000000 });
        return a.dossier + "/" + a.nom;
      }
      case "fichiers_renommer": { const x = window.APERCU_FICHIERS.find((f) => a.ancien.endsWith("/" + f.nom)); if (x) x.nom = a.nom; return a.ancien.replace(/[^/]+$/, a.nom); }
      case "fichiers_proprietes": return [["Nom", "rapport-final.pdf"], ["Type", "Document PDF"], ["Emplacement", "/home/lea"], ["Taille", "2480000"], ["Modifié le", "1795000000"]];
      case "fichiers_viser": return { genre: "dossier", chemin: a.texte };
      case "fichiers_coller": return { faits: a.sources.length, soucis: [] };
      case "fichiers_corbeille": return { faits: a.chemins.length, refus: [] };
      case "fichiers_apercu": return /\.(jpg|png)$/.test(a.chemin) ? { genre: "image", donnees: "data:image/svg+xml;utf8,<svg xmlns=%22http://www.w3.org/2000/svg%22 width=%22400%22 height=%22260%22><rect width=%22400%22 height=%22260%22 fill=%22%233b82f6%22/></svg>" } : /\.txt$/.test(a.chemin) ? { genre: "texte", texte: "Courses :\n- pain\n- lait", coupe: false } : { genre: "rien" };
      case "agenda_lire": return [{ id: "1", date: new Date().toISOString().slice(0, 10), heure: "14:30", titre: "Dentiste", note: "Apporter la carte" }, { id: "2", date: new Date().toISOString().slice(0, 10), heure: "", titre: "Anniversaire de Léa", note: "" }];
      case "agenda_poser": return [...(await window.APERCU("agenda_lire")), a.evenement];
      case "agenda_retirer": return (await window.APERCU("agenda_lire")).filter((e) => e.id !== a.id);
      case "grenai_reglages": return { fournisseur: "ollama", fournisseurs: [{ cle: "ollama", nom: "Ollama (sur cette machine)", modele: "llama3.2", a_une_cle: false, local: true }, { cle: "openai", nom: "OpenAI (GPT)", modele: "gpt-4o-mini", a_une_cle: true, local: false }, { cle: "anthropic", nom: "Anthropic (Claude)", modele: "claude-opus-5-5", a_une_cle: false, local: false }] };
      case "grenai_demander": await attendre(700); return "Voici une réponse d'exemple, écrite par l'aperçu : aucun modèle n'a été appelé.";
      case "recuperation_lister": await attendre(500); return [{ dev: "/dev/nvme0n1p2", fs: "ext4", taille: "476,4G", etiquette: "", grenos: true, machine: "grenos-lea", comptes: ["lea"], montee: "" }, { dev: "/dev/sda1", fs: "ext4", taille: "20G", etiquette: "DONNEES", grenos: false, machine: "", comptes: [], montee: "" }];
      case "recuperation_faire": await attendre(600); return a.geste === "ouvrir" ? "/run/grenos-recup/nvme0n1p2" : "Le système de fichiers est sain.";
      case "fichiers_dire": console.log("série :", a.texte); return null;
      default: return null;
    }
  };
})();
