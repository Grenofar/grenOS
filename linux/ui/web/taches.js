// Gestionnaire de tâches (Ctrl+Maj+Échap), en quatre onglets (23 novembre) :
//   Processus    tout ce qui tourne, regroupé par application — sauf grenOS
//                lui-même, qu'on ne peut ni arrêter ni retirer ;
//   Performance  le processeur et la mémoire, en direct, sur deux minutes ;
//   Démarrage    ce qui s'ouvre avec la session : couper, retirer, AJOUTER ;
//   Jeu          libérer la RAM, et le mode jeu de grenPerf.
// Les mesures sont celles de grenPerf (perf.rs, faites à la manière de Perf
// Saver) : une seule lecture de /proc toutes les 1,5 s nourrit les onglets.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, toast } = window.grenos;
  const D = {};
  const ONGLETS = [["processus", "Processus", "fenetres"], ["performance", "Performance", "cpu"],
    ["demarrage", "Démarrage", "fusee"], ["jeu", "Jeu", "manette"]];
  let onglet = window.GRENOS_ONGLET || "processus";
  if (!ONGLETS.some(([c]) => c === onglet)) onglet = "processus";
  let dernier = null;
  const POINTS = 80;                                   // 80 × 1,5 s = 2 minutes
  const histo = { cpu: [], ram: [] };
  let filtre = "";
  let tri = { cle: "cpu", sens: -1 };
  let choisi = null;
  const aFermer = new Set();
  let jeuVu = false;
  const POUR_JOUER = /steam|proton|wine|heroic|lutris|gamemode|sober|bottles/i;
  const nombre = (v, d = 1) => v.toFixed(d).replace(".", ",");
  const taille = (mo) => (mo >= 1024 ? `${nombre(mo / 1024)} Go` : `${Math.round(mo)} Mo`);
  const visibles = (s) => s.groupes.filter((g) => !g.protege);

  // ---- La tête : deux jauges toujours visibles --------------------------
  function jauge(nom) {
    const j = { valeur: el("strong", "—"), barre: el("i"), detail: el("small") };
    j.bloc = el("div.tc-jauge", el("div.etiquette", nom), j.valeur, el("div.progression", j.barre), j.detail);
    return j;
  }

  function tete(s) {
    D.cpu.valeur.textContent = `${Math.round(s.cpu)} %`;
    D.cpu.barre.style.width = `${Math.min(100, s.cpu)}%`;
    D.cpu.detail.textContent = `${s.coeurs} cœur(s) · charge ${s.charge}`;
    const utilisee = s.memoire_totale_go - s.memoire_dispo_go;
    const part = utilisee / Math.max(0.01, s.memoire_totale_go) * 100;
    D.mem.valeur.textContent = `${Math.round(part)} %`;
    D.mem.barre.style.width = `${Math.min(100, part)}%`;
    D.mem.detail.textContent = `${nombre(utilisee)} Go sur ${nombre(s.memoire_totale_go)} Go`;
    D.jeu.classList.toggle("actif", s.mode_jeu);
    D.jeu.replaceChildren(svg("manette"), s.mode_jeu ? "Mode jeu actif" : "Mode jeu");
  }

  // ---- Processus ----------------------------------------------------------
  function processus(s) {
    const recherche = el("input.champ.tc-recherche", { type: "search", placeholder: "Chercher une application ou un processus", value: filtre });
    recherche.addEventListener("input", () => { filtre = recherche.value.trim().toLowerCase(); D.corpsTable && remplirTable(); });
    D.corpsTable = el("tbody");
    D.fin = el("button.bouton.danger", { disabled: true, onclick: arreter }, "Fin de tâche");
    D.caches = el("span.note");
    const cpu = entete("Processeur", "cpu", true);
    if (tri.cle === "cpu") cpu.classList.add("trie");
    remplirTable();
    return [recherche,
      el("div.tc-defile.carte.verre", el("table.tc-table",
        el("thead", el("tr", entete("Nom", "nom"), cpu, entete("Mémoire", "memoire_mo", true), entete("Processus", "nombre", true), entete("Type", "a_moi"))),
        D.corpsTable)),
      el("footer.tc-pied", D.caches, D.fin)];
  }

  function remplirTable() {
    const s = dernier;
    if (!s || !D.corpsTable) return;
    const tous = visibles(s);
    const caches = s.groupes.length - tous.length;
    const groupes = tous
      .filter((g) => !filtre || g.nom.toLowerCase().includes(filtre) || g.description.toLowerCase().includes(filtre))
      .sort((a, b) => {
        const x = a[tri.cle], y = b[tri.cle];
        if (typeof x === "string") return x.localeCompare(y, "fr") * tri.sens;
        return ((x === y ? 0 : x > y ? 1 : -1)) * tri.sens;
      });
    D.corpsTable.replaceChildren(...groupes.map((g) => {
      const tr = el(`tr${g.nom === choisi ? ".choisi" : ""}`,
        el("td", el("div.tc-qui", el("b", g.description || g.nom), el("small", g.description ? g.nom : ""))),
        el("td.nb", `${nombre(g.cpu)} %`),
        el("td.nb", taille(g.memoire_mo)),
        el("td.nb", String(g.nombre)),
        el("td", el(`span.tc-type${g.a_moi ? "" : ".systeme"}`, g.a_moi ? "Application" : "Système")));
      tr.addEventListener("click", () => { choisi = g.nom; remplirTable(); });
      tr.addEventListener("dblclick", () => { choisi = g.nom; arreter(); });
      return tr;
    }));
    D.caches.textContent = `${tous.reduce((n, g) => n + g.nombre, 0)} processus · ${caches} élément(s) de grenOS masqués : ils font tourner le bureau.`;
    D.fin.disabled = !tous.some((g) => g.nom === choisi);
  }

  function entete(texte, cle, nb) {
    const th = el(`th${nb ? ".nb" : ""}`, texte);
    th.addEventListener("click", () => {
      tri = { cle, sens: tri.cle === cle ? -tri.sens : (cle === "nom" ? 1 : -1) };
      for (const t of document.querySelectorAll(".tc-table th")) t.classList.remove("trie");
      th.classList.add("trie");
      remplirTable();
    });
    return th;
  }

  function confirmer(titre, texte, action) {
    return new Promise((ok) => {
      const fond = el("div.cx-fond");
      const fin = (v) => { fond.remove(); ok(v); };
      const non = el("button.bouton.discret", { type: "button", onclick: () => fin(false) }, "Annuler");
      fond.append(el("div.cx-dialogue.carte", el("h2", titre), el("p.note", texte),
        el("div.cx-boutons", non, el("button.bouton.danger", { onclick: () => fin(true) }, action))));
      fond.addEventListener("keydown", (e) => { if (e.key === "Escape") { e.stopPropagation(); fin(false); } });
      document.body.append(fond);
      non.focus();
    });
  }

  async function arreter() {
    const g = dernier && visibles(dernier).find((x) => x.nom === choisi);
    if (!g) return;
    const nom = g.description || g.nom;
    const texte = g.a_moi
      ? "Ce qui n'est pas enregistré dans cette application sera perdu."
      : "C'est un programme du système : grenOS demandera ton mot de passe avant de l'arrêter.";
    if (!(await confirmer(`Arrêter « ${nom} » ?`, texte, "Arrêter"))) return;
    try {
      const n = g.a_moi ? await api.appeler("perf_fermer", { nom: g.nom }) : await api.appeler("taches_arreter_groupe", { pids: g.pids });
      toast(`${nom} arrêté (${n} processus).`);
    } catch (e) { toast(String(e)); }
    choisi = null;
    setTimeout(rafraichir, 800);
  }

  // ---- Performance --------------------------------------------------------
  function tuile(titre, valeur, detail, part, chaud) {
    return el(`div.carte.verre.tuile-chiffre${chaud ? ".chaud" : ""}`, el("div.etiquette", titre), el("div.chiffre", valeur),
      el("div.jauge", el("i", { style: `width:${Math.max(1, Math.min(100, part * 100))}%` })), el("div.note", detail));
  }

  function performance(s) {
    D.courbeCpu = el("canvas.tc-courbe", { width: 900, height: 180, "aria-label": "Processeur sur deux minutes" });
    D.courbeRam = el("canvas.tc-courbe", { width: 900, height: 180, "aria-label": "Mémoire sur deux minutes" });
    D.tuilesPerf = el("section.tc-tuiles");
    const corps = [D.tuilesPerf,
      el("div.carte.verre.tc-graphe", el("div.tc-graphe-tete", el("b", "Processeur"), el("span.note", "deux dernières minutes")), D.courbeCpu),
      el("div.carte.verre.tc-graphe", el("div.tc-graphe-tete", el("b", "Mémoire"), el("span.note", "deux dernières minutes")), D.courbeRam)];
    majPerformance(s);
    return corps;
  }

  function majPerformance(s) {
    if (!D.tuilesPerf || !D.tuilesPerf.isConnected) return;
    const utilisee = s.memoire_totale_go - s.memoire_dispo_go;
    const partRam = utilisee / Math.max(0.01, s.memoire_totale_go);
    const echange = s.echange_total_go > 0 ? s.echange_utilise_go / s.echange_total_go : 0;
    const applis = s.groupes.filter((g) => g.a_moi && !g.protege).length;
    D.tuilesPerf.replaceChildren(
      tuile("Processeur", `${Math.round(s.cpu)} %`, `${s.coeurs} cœur(s) · charge ${s.charge}`, s.cpu / 100, s.cpu > 85),
      tuile("Mémoire", `${nombre(utilisee)} Go`, `sur ${nombre(s.memoire_totale_go)} Go · ${nombre(s.memoire_cache_go)} Go de cache`, partRam, partRam > 0.88),
      tuile("Échange", s.echange_total_go > 0 ? `${nombre(s.echange_utilise_go)} Go` : "aucun", s.echange_total_go > 0 ? `sur ${nombre(s.echange_total_go)} Go` : "pas de mémoire d'échange", echange, echange > 0.5),
      tuile("Applications", String(applis), `${s.groupes.reduce((n, g) => n + g.nombre, 0)} processus en tout`, 0, false));
    courbe(D.courbeCpu, histo.cpu, "--accent");
    courbe(D.courbeRam, histo.ram, "--reussi");
  }

  function courbe(toile, valeurs, couleur) {
    if (!toile) return;
    const c = toile.getContext("2d");
    const l = toile.width, h = toile.height;
    const style = getComputedStyle(document.body);
    const trait = style.getPropertyValue(couleur).trim() || "#5b8cff";
    const grille = style.getPropertyValue("--bord").trim() || "rgba(255,255,255,.1)";
    c.clearRect(0, 0, l, h);
    c.strokeStyle = grille;
    c.lineWidth = 1;
    for (const p of [0.25, 0.5, 0.75]) { c.beginPath(); c.moveTo(0, h * p); c.lineTo(l, h * p); c.stroke(); }
    if (valeurs.length < 2) return;
    const pas = l / (POINTS - 1);
    const x0 = l - (valeurs.length - 1) * pas;
    const y = (v) => h - 4 - (Math.min(100, Math.max(0, v)) / 100) * (h - 8);
    c.beginPath();
    valeurs.forEach((v, i) => (i ? c.lineTo(x0 + i * pas, y(v)) : c.moveTo(x0, y(v))));
    c.strokeStyle = trait;
    c.lineWidth = 2.5;
    c.lineJoin = "round";
    c.stroke();
    c.lineTo(l, h);
    c.lineTo(x0, h);
    c.closePath();
    c.globalAlpha = 0.18;
    c.fillStyle = trait;
    c.fill();
    c.globalAlpha = 1;
  }

  // ---- Démarrage ----------------------------------------------------------
  async function demarrage() {
    let items = [];
    try { items = await api.appeler("taches_demarrage"); } catch (e) { toast(String(e)); }
    const zone = el("div.lignes");
    for (const d of items) {
      const b = el("button.interrupteur", { role: "switch", type: "button", "aria-checked": String(d.actif), "aria-label": `${d.nom} au démarrage`,
        onclick: async () => {
          const voulu = b.getAttribute("aria-checked") !== "true";
          try { await api.appeler("perf_regler_demarrage", { id: d.id, actif: voulu }); b.setAttribute("aria-checked", String(voulu)); }
          catch (e) { toast(String(e)); }
        } });
      const retirer = d.retirable ? el("button.bouton.petit.discret", { onclick: async () => {
        try { await api.appeler("taches_demarrage_retirer", { id: d.id }); toast(`${d.nom} ne démarre plus avec la session.`); }
        catch (e) { toast(String(e)); }
        montrer();
      } }, "Retirer") : "";
      zone.append(el("div.ligne", el("span.icone", svg("fusee")),
        el("div.tc-qui", el("b.nom", d.nom), el("small.detail", d.commentaire || d.id)),
        el("span.valeur.tc-actions", retirer, b)));
    }
    if (!items.length) zone.append(el("p.note", "Aucune application ne démarre avec ta session. grenOS lance déjà tout ce qu'il lui faut."));
    return [el("div.barre-outils",
      el("p.sous", "Ce qui s'ouvre tout seul à chaque session. Couper ne désinstalle rien. Les éléments de grenOS n'apparaissent pas : il les lance lui-même."),
      el("button.bouton.principal", { onclick: ajouter }, svg("fusee"), "Ajouter une application")), zone];
  }

  async function ajouter() {
    let apps = [];
    try { apps = await api.appeler("taches_applications"); } catch (e) { toast(String(e)); return; }
    const fond = el("div.cx-fond");
    const fermer = () => fond.remove();
    const recherche = el("input.champ", { type: "search", placeholder: "Chercher une application" });
    const liste = el("div.tc-choix-apps");
    const remplir = () => {
      const q = recherche.value.trim().toLowerCase();
      liste.replaceChildren(...apps.filter((a) => !q || a.nom.toLowerCase().includes(q) || a.commentaire.toLowerCase().includes(q)).slice(0, 200).map((a) =>
        el("button.tc-app", { type: "button", onclick: async () => {
          try { await api.appeler("taches_demarrage_ajouter", { id: a.id }); toast(`${a.nom} démarrera avec ta session.`); }
          catch (e) { toast(String(e)); }
          fermer();
          montrer();
        } }, el("b", a.nom), el("small.note", a.commentaire))));
    };
    recherche.addEventListener("input", remplir);
    fond.append(el("div.cx-dialogue.carte.tc-dialogue", el("h2", "Démarrer avec la session"),
      el("p.note", "Choisis l'application qui s'ouvrira toute seule à chaque fois que tu arrives sur ton bureau."),
      recherche, liste, el("div.cx-boutons", el("button.bouton.discret", { onclick: fermer }, "Fermer"))));
    fond.addEventListener("keydown", (e) => { if (e.key === "Escape") { e.stopPropagation(); fermer(); } });
    document.body.append(fond);
    remplir();
    recherche.focus();
  }

  // ---- Jeu ----------------------------------------------------------------
  function jeu(s) {
    const candidats = s.groupes.filter((g) => g.a_moi && !g.protege && !POUR_JOUER.test(g.nom) && (g.en_trop || g.memoire_mo > 150));
    if (!jeuVu) { candidats.filter((g) => g.en_trop).forEach((g) => aFermer.add(g.nom)); jeuVu = true; }
    const choix = el("div.choix.liste");
    for (const g of candidats) {
      const t = el("button.tuile.compacte", { role: "checkbox", type: "button", "aria-checked": String(aFermer.has(g.nom)),
        onclick: () => { aFermer.has(g.nom) ? aFermer.delete(g.nom) : aFermer.add(g.nom); t.setAttribute("aria-checked", String(aFermer.has(g.nom))); } },
        el("span.pastille", svg(g.en_trop ? "sablier" : "paquet")),
        el("span", el("b", g.description || g.nom), el("small", `${taille(g.memoire_mo)}${g.en_trop ? " · inutile pendant une partie" : ""}`)),
        el("span.coche", svg("coche")));
      choix.append(t);
    }
    const dispo = s.memoire_dispo_go;
    D.resultatRam = el("p.note", `${nombre(dispo)} Go disponibles sur ${nombre(s.memoire_totale_go)} Go.`);
    return [
      el("div.tc-jeu",
        el("div.carte.verre.tc-carte-jeu",
          el("div.tc-carte-tete", el("span.pastille", svg("ram")), el("b", "Libérer la RAM")),
          el("p.sous", "Compacte la mémoire et rapatrie ce qui dort dans l'échange. Le chiffre affiché est mesuré avant et après, pas promis."),
          D.resultatRam,
          el("button.bouton.principal", { onclick: liberer }, svg("eclair"), "Libérer la RAM")),
        el("div.carte.verre.tc-carte-jeu",
          el("div.tc-carte-tete", el("span.pastille", svg("manette")), el("b", s.mode_jeu ? "Mode jeu actif" : "Mode jeu")),
          el("p.sous", "Ferme ce qui est coché ci-dessous, passe tes autres applications en arrière-plan, et garde la priorité pour Steam, Proton et le jeu. Le couper rend tout comme avant. Raccourci : Ctrl+Alt+Échap."),
          el("button.bouton.principal", { onclick: basculerJeu }, svg("manette"), s.mode_jeu ? "Couper le mode jeu" : "Activer le mode jeu"))),
      candidats.length ? el("div.etiquette", "À fermer pendant la partie") : el("p.note", "Rien de lourd ni d'inutile ne tourne en ce moment."),
      choix];
  }

  async function liberer() {
    D.resultatRam.textContent = "Libération de la mémoire…";
    try {
      const r = await api.appeler("perf_liberer");
      D.resultatRam.textContent = `${nombre(r.avant_go)} Go → ${nombre(r.apres_go)} Go disponibles. ${r.message}`;
    } catch (e) { D.resultatRam.textContent = String(e); }
  }

  async function basculerJeu() {
    const actif = !(dernier && dernier.mode_jeu);
    try {
      const b = await api.appeler("perf_mode_jeu", { actif, fermer: actif ? [...aFermer] : [], garder: [] });
      toast(b.message);
    } catch (e) { toast(String(e)); }
    await rafraichir();
    montrer();
  }

  // ---- La boucle ----------------------------------------------------------
  async function montrer() {
    D.onglets.querySelectorAll(".onglet").forEach((o) => o.setAttribute("aria-selected", String(o.dataset.cle === onglet)));
    if (!dernier) return;
    const contenu = onglet === "processus" ? processus(dernier)
      : onglet === "performance" ? performance(dernier)
      : onglet === "demarrage" ? await demarrage() : jeu(dernier);
    D.corps.replaceChildren(...contenu);
    D.corps.dataset.onglet = onglet;
  }

  async function rafraichir() {
    try { dernier = await api.appeler("perf_instantane"); } catch (e) { return; }
    histo.cpu.push(dernier.cpu);
    histo.ram.push((1 - dernier.memoire_dispo_go / Math.max(0.01, dernier.memoire_totale_go)) * 100);
    for (const h of Object.values(histo)) while (h.length > POINTS) h.shift();
    tete(dernier);
    if (onglet === "processus") remplirTable();
    else if (onglet === "performance") majPerformance(dernier);
  }

  function boucle() {
    if (!document.hidden) rafraichir();
    setTimeout(boucle, 1500);
  }

  window.grenosOnglet = (cle) => { if (ONGLETS.some(([c]) => c === cle)) { onglet = cle; montrer(); } };

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.cpu = jauge("Processeur");
    D.mem = jauge("Mémoire");
    D.jeu = el("button.bouton.mode-jeu", { onclick: () => { onglet = "jeu"; montrer(); } });
    D.onglets = el("nav.onglets", { role: "tablist" });
    for (const [cle, titre, icone] of ONGLETS) {
      D.onglets.append(el("button.onglet", { role: "tab", "data-cle": cle, onclick: () => { onglet = cle; montrer(); } }, svg(icone), titre));
    }
    D.corps = el("section.tc-corps");
    document.body.append(el("div.page-taches",
      el("header.tc-tete.carte.verre", D.cpu.bloc, D.mem.bloc, D.jeu),
      D.onglets, D.corps));
    await api.appeler("perf_instantane");          // la base des pourcentages
    // LA PREUVE DE LA CI (23 novembre) : ci-demarrage.sh ouvre cette fenêtre
    // par Ctrl+Maj+Échap et attend « taches ouvert » au journal, que dit
    // taches_mesure à son premier appel. Les onglets lisent grenPerf ; sans
    // cet appel, la fenêtre marchait et la CI ne le savait pas (run 45129195984).
    api.appeler("taches_mesure").catch(() => {});
    await new Promise((r) => setTimeout(r, 700));
    await rafraichir();
    await montrer();
    pret();
    setTimeout(boucle, 1500);
  })();

  document.addEventListener("keydown", (e) => {
    if (document.querySelector(".cx-fond")) return;
    if (e.key === "Escape") api.appeler("fermer");
    else if (e.key === "Delete" && choisi != null && onglet === "processus") arreter();
  });
})();
