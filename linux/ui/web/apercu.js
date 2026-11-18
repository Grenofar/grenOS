// Données factices pour regarder le dessin dans un navigateur, hors de Tauri.
// Ne sert JAMAIS dans grenos-ui : `window.__TAURI__` y existe, et ce fichier
// ne fait alors rien.
"use strict";
(function () {
  if (window.__TAURI__) return;
  const attendre = (ms) => new Promise((r) => setTimeout(r, ms));
  const DONNEES = {
    ecrans: ["bienvenue", "langue", "clavier", "compte", "parcours", "reseau", "apparence", "usage", "confidentialite", "securite", "majs", "applications", "sauvegardes", "optimisation", "systeme", "pare-feu", "journaux", "final"]
      .map((cle) => ({ cle, titre: cle, parcours: ["bienvenue", "langue", "clavier", "compte", "parcours", "final"].includes(cle) ? ["rapide", "personnalise", "dev"] : ["systeme", "pare-feu", "journaux"].includes(cle) ? ["dev"] : ["personnalise", "dev"] })),
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
      case "premier_infos": return { donnees: DONNEES, machine: "grenos", virtuelle: "kvm", secure_boot: "inactif",
        secure_boot_phrase: "Secure Boot est désactivé, ce qu'il faut aujourd'hui pour démarrer grenOS.",
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
      case "apparence": return { theme: "nuit", accent: "bleu", effets: "equilibres" };
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
      default: return null;
    }
  };
})();
