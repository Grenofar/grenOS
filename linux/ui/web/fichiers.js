// L'Explorateur de fichiers : « Accès rapide » puis la page « Ce PC », un fil
// d'Ariane qui devient une barre d'adresse (F4, Ctrl+L), des onglets (Ctrl+T,
// Ctrl+W, Ctrl+Tab), et les raccourcis de Windows relus sur sa page « Keyboard
// shortcuts » — Retour arrière y veut dire PRÉCÉDENT, Alt+Haut le parent.
//
// Les onglets ne sont pas des vues : un onglet est un dossier avec SON
// histoire (onglets.py avant le 18 novembre). Le dernier ne se ferme pas, et
// après une fermeture l'actif est le voisin de gauche.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, toast } = window.grenos;
  const D = {};
  const S = {
    onglets: [], actif: 0, elements: [], caches: 0, montrerCaches: false,
    choisis: new Set(), ancre: -1, tri: { cle: "nom", sens: 1 }, apercu: false,
    pressePapier: [], couper: false, maison: "/", cePc: [],
  };
  const nombre = (v, d = 1) => v.toFixed(d).replace(".", ",");
  const dire = (t) => api.appeler("fichiers_dire", { texte: t });
  const base = (c) => c.replace(/\/+$/, "").split("/").pop() || "/";
  const parent = (c) => { const p = c.replace(/\/+$/, "").split("/").slice(0, -1).join("/"); return p || "/"; };
  const joindre = (d, n) => (d === "/" ? "/" : d + "/") + n;
  // CE PC, UNE PAGE (26 novembre). Grenofar : « dans explorateur ajoute une
  // page Ce PC où on peut voir les trucs Windows (enlève donc de la gauche),
  // une page, pas juste une catégorie à gauche ». Comme sous Windows : les
  // dossiers, ceux de Windows, puis chaque disque avec sa jauge de place
  // libre. C'est un « dossier » de plus pour les onglets et l'historique.
  const CE_PC = "ce-pc:";
  const enCePc = () => onglet().dossier === CE_PC;
  const nomDe = (c) => (c === CE_PC ? "Ce PC" : base(c));
  const ICONES = { dossier: "dossier", image: "image", video: "film", audio: "son", archive: "paquet", pdf: "texte", texte: "texte", code: "code", programme: "terminal", fichier: "fichier" };

  function taille(o) {
    if (o < 1024) return `${o} o`;
    for (const [s, u] of [[1024 ** 3, "Go"], [1024 ** 2, "Mo"], [1024, "Ko"]]) if (o >= s) return `${nombre(o / s)} ${u}`;
    return `${o} o`;
  }
  function date(s) {
    if (!s) return "";
    const d = new Date(s * 1000), z = (n) => String(n).padStart(2, "0");
    return `${z(d.getDate())}/${z(d.getMonth() + 1)}/${d.getFullYear()} ${z(d.getHours())}:${z(d.getMinutes())}`;
  }
  function etat(t) { D.etat.textContent = t; }

  // ---- Onglets ---------------------------------------------------------------
  const onglet = () => S.onglets[S.actif];
  function nouvelOnglet(dossier) { return { dossier, histoire: [dossier], rang: 0 }; }

  function dessinerOnglets() {
    D.onglets.hidden = S.onglets.length <= 1;
    D.onglets.replaceChildren(...S.onglets.map((o, i) =>
      el(`button${i === S.actif ? ".actif" : ""}`, { onclick: () => { S.actif = i; changerDOnglet(); } }, nomDe(o.dossier))),
      el("button.fx-fermer-onglet", { title: "Fermer cet onglet (Ctrl+W)", onclick: fermerOnglet }, "×"));
  }

  // La recherche est effacée en changeant d'onglet : garder le filtre d'un
  // onglet sur un autre montrerait une liste amputée sans qu'on sache pourquoi.
  async function changerDOnglet() {
    if (D.cherche.value) D.cherche.value = "";
    await aller(onglet().dossier, false);
    dessinerOnglets();
    D.liste.focus();
    dire(`fichiers : onglets — ${S.onglets.length}, actif « ${nomDe(onglet().dossier)} »`);
  }
  function ouvrirOnglet() { S.onglets.splice(S.actif + 1, 0, nouvelOnglet(onglet().dossier)); S.actif++; changerDOnglet(); }
  function fermerOnglet() {
    if (S.onglets.length <= 1) {
      etat("C'est le dernier onglet : Alt+F4 ferme la fenêtre.");
      // Le refus aussi se dit : un compteur qui ne descendrait jamais
      // dirait sinon la même chose qu'un dernier onglet bien gardé.
      dire(`fichiers : onglets — 1, Ctrl+W refuse sur le dernier`);
      return;
    }
    S.onglets.splice(S.actif, 1);
    S.actif = Math.max(0, S.actif - 1);
    changerDOnglet();
  }

  // ---- Navigation --------------------------------------------------------------
  async function aller(chemin, memoriser = true) {
    const cePc = chemin === CE_PC;
    let liste = { elements: [], caches: 0 };
    try {
      if (cePc) await dessinerCePc();
      else liste = await api.appeler("fichiers_lister", { dossier: chemin, caches: S.montrerCaches });
    } catch (e) { etat(String(e)); return false; }
    D.liste.hidden = cePc;
    D.cePc.hidden = !cePc;
    D.entreeCePc?.classList.toggle("actif", cePc);
    const o = onglet();
    o.dossier = chemin;
    if (memoriser) { o.histoire.splice(o.rang + 1); o.histoire.push(chemin); o.rang = o.histoire.length - 1; }
    S.elements = liste.elements;
    S.caches = liste.caches;
    S.choisis.clear();
    S.ancre = -1;
    dessiner();
    dessinerFil();
    D.precedent.disabled = o.rang <= 0;
    D.suivant.disabled = o.rang + 1 >= o.histoire.length;
    D.parent.disabled = cePc;
    api.appeler("fichiers_titre", { titre: `${cePc ? "Ce PC" : chemin === "/" ? "Système de fichiers" : base(chemin)} — Explorateur de fichiers` });
    dessinerOnglets();
    return true;
  }
  async function rafraichir() {
    const garde = new Set(S.choisis);
    await aller(onglet().dossier, false);
    for (const c of garde) if (S.elements.some((e) => e.chemin === c)) S.choisis.add(c);
    dessiner();
  }
  function precedent() { const o = onglet(); if (o.rang > 0) { o.rang--; aller(o.histoire[o.rang], false); } }
  function suivant() { const o = onglet(); if (o.rang + 1 < o.histoire.length) { o.rang++; aller(o.histoire[o.rang], false); } }
  // Au-dessus de la racine d'un disque, ou de « / », il y a Ce PC (Windows aussi).
  function auParent() {
    const d = onglet().dossier;
    if (d === CE_PC) return;
    if (d === "/" || S.cePc.some((x) => x.chemin === d && x.icone !== "compte")) aller(CE_PC);
    else aller(parent(d));
  }

  // ---- La liste ------------------------------------------------------------------
  function visibles() {
    const f = D.cherche.value.trim().toLowerCase();
    const { cle, sens } = S.tri;
    return S.elements
      .filter((e) => !f || e.nom.toLowerCase().includes(f))
      .sort((a, b) => {
        if (cle === "nom" && a.dossier !== b.dossier) return a.dossier ? -1 : 1;
        const x = a[cle], y = b[cle];
        const r = typeof x === "string" ? x.localeCompare(y, "fr", { sensitivity: "base", numeric: true }) : x - y;
        return r * sens;
      });
  }

  function dessiner() {
    if (enCePc()) {
      const n = D.cePc.querySelectorAll(".fx-tuile.disque").length;
      etat(`${n} disque${n > 1 ? "s" : ""}`);
      apercu();
      return;
    }
    const v = visibles();
    D.corps.replaceChildren(...v.map((e, i) => {
      const tr = el(`tr${S.choisis.has(e.chemin) ? ".choisi" : ""}`, { "data-i": i },
        el("td", el("span.fx-nom", el(`span.fx-icone.${e.famille}`, svg(ICONES[e.famille] || "fichier")), el("span", e.nom))),
        el("td", date(e.modifie)), el("td", e.genre), el("td.nb", e.dossier ? "" : taille(e.octets)));
      tr.addEventListener("mousedown", (ev) => choisirLigne(ev, v, i));
      tr.addEventListener("dblclick", () => ouvrir(e));
      tr.addEventListener("contextmenu", (ev) => { ev.preventDefault(); if (!S.choisis.has(e.chemin)) { S.choisis = new Set([e.chemin]); S.ancre = i; dessiner(); } menu(ev); });
      return tr;
    }));
    if (!v.length) D.corps.append(el("tr.fx-vide", el("td", { colspan: 4 }, D.cherche.value ? "Rien ne correspond." : "Ce dossier est vide.")));
    for (const th of D.entetes) th.classList.toggle("trie", th.dataset.cle === S.tri.cle);
    compte(v.length);
    apercu();
  }

  // ---- G3 : le volet d'aperçu (Alt+P) ---------------------------------------------
  let apercuDemande = "";
  async function apercu() {
    D.apercu.hidden = !S.apercu;
    D.milieu.classList.toggle("avec-apercu", S.apercu);
    if (!S.apercu) return;
    const c = choisis();
    if (c.length !== 1) {
      apercuDemande = "";
      D.apercu.replaceChildren(el("p.note.fx-apercu-vide", c.length ? `${c.length} éléments sélectionnés` : "Choisis un fichier pour le voir ici."));
      return;
    }
    const e = c[0];
    if (apercuDemande === e.chemin) return;
    apercuDemande = e.chemin;
    D.apercu.replaceChildren(el("p.note.fx-apercu-vide", "…"));
    const r = await api.appeler("fichiers_apercu", { chemin: e.chemin });
    if (apercuDemande !== e.chemin || !r) return; // une autre sélection est passée devant
    const tete = el("div.fx-apercu-tete", el(`span.fx-icone.${e.famille}`, svg(ICONES[e.famille] || "fichier")),
      el("div", el("strong", e.nom), el("small.note", [e.genre, e.dossier ? "" : taille(e.octets), date(e.modifie)].filter(Boolean).join(" · "))));
    let corps;
    if (r.genre === "image") corps = el("img.fx-apercu-image", { src: r.donnees, alt: e.nom });
    else if (r.genre === "texte") corps = el("pre.fx-apercu-texte", r.texte + (r.coupe ? "\n…" : ""));
    else if (r.genre === "dossier") corps = el("p.note", `${r.elements} élément(s) dedans.`);
    else if (r.genre === "trop") corps = el("p.note", "Image trop lourde pour l'aperçu : ouvre-la.");
    else corps = el("p.note", "Pas d'aperçu pour ce type de fichier.");
    D.apercu.replaceChildren(tete, corps);
  }
  function basculerApercu() { S.apercu = !S.apercu; apercuDemande = ""; apercu(); }

  function compte(total) {
    let t = `${total} ${total <= 1 ? "élément" : "éléments"}`;
    const n = S.choisis.size;
    if (n > 1) t += `   ·   ${n} sélectionnés`; else if (n === 1) t += "   ·   1 sélectionné";
    if (S.caches) t += `   ·   ${S.caches} masqué${S.caches > 1 ? "s" : ""}${S.montrerCaches ? " (affichés)" : ""}`;
    etat(t);
  }

  function choisirLigne(ev, v, i) {
    if (ev.button === 2 && S.choisis.has(v[i].chemin)) return;
    if (ev.shiftKey && S.ancre >= 0) {
      const [a, b] = [Math.min(S.ancre, i), Math.max(S.ancre, i)];
      S.choisis = new Set(v.slice(a, b + 1).map((e) => e.chemin));
    } else if (ev.ctrlKey) {
      S.choisis.has(v[i].chemin) ? S.choisis.delete(v[i].chemin) : S.choisis.add(v[i].chemin);
      S.ancre = i;
    } else {
      S.choisis = new Set([v[i].chemin]);
      S.ancre = i;
    }
    dessiner();
  }

  function choisirChemin(chemin) {
    S.choisis = new Set([chemin]);
    const v = visibles();
    S.ancre = v.findIndex((e) => e.chemin === chemin);
    dessiner();
    D.corps.querySelector("tr.choisi")?.scrollIntoView({ block: "nearest" });
  }

  const choisis = () => S.elements.filter((e) => S.choisis.has(e.chemin));

  function ouvrir(e) {
    if (e.dossier) aller(e.chemin);
    else api.appeler("fichiers_ouvrir", { chemin: e.chemin }).catch((x) => etat(String(x)));
  }

  // ---- Fil d'Ariane et barre d'adresse ---------------------------------------------
  function dessinerFil() {
    const d = onglet().dossier;
    let morceaux, reste, courant;
    // Le disque qui porte ce dossier (le plus profond) : « Ce PC › Windows › Users ».
    const lecteur = S.cePc
      .filter((x) => x.chemin && x.chemin !== "/" && x.icone !== "compte" && (d === x.chemin || d.startsWith(x.chemin + "/")))
      .sort((a, b) => b.chemin.length - a.chemin.length)[0];
    if (d === CE_PC) {
      morceaux = [["Ce PC", CE_PC]]; reste = ""; courant = "";
    } else if (d === S.maison || d.startsWith(S.maison + "/")) {
      morceaux = [[S.cePc[0]?.nom || "Dossier personnel", S.maison]]; reste = d.slice(S.maison.length); courant = S.maison;
    } else if (lecteur) {
      morceaux = [["Ce PC", CE_PC], [nomDisque(lecteur), lecteur.chemin]]; reste = d.slice(lecteur.chemin.length); courant = lecteur.chemin;
    } else {
      morceaux = [["Ce PC", CE_PC], ["Système de fichiers", "/"]]; reste = d; courant = "";
    }
    for (const m of reste.split("/").filter(Boolean)) { courant = `${courant}/${m}`; morceaux.push([m, courant]); }
    // replaceChildren écrit « null » en toutes lettres : la première miette
    // n'a pas de flèche, elle n'a rien du tout (« null grnfr », 26 novembre).
    D.fil.replaceChildren(...morceaux.flatMap(([t, c], i) => [
      i ? el("span.fx-fleche", "›") : null,
      el("button.fx-miette", { onclick: (ev) => { ev.stopPropagation(); aller(c); } }, t)]).filter(Boolean));
  }
  function ouvrirAdresse() {
    D.adresse.value = enCePc() ? "" : onglet().dossier;
    D.fil.hidden = true; D.adresse.hidden = false;
    D.adresse.focus(); D.adresse.select();
  }
  function fermerAdresse() { D.adresse.hidden = true; D.fil.hidden = false; D.liste.focus(); }
  // Un chemin faux ne s'avale pas en silence : le champ reste ouvert et la
  // barre d'état nomme la raison.
  async function adresseTapee() {
    const r = await api.appeler("fichiers_viser", { texte: D.adresse.value, depuis: onglet().dossier });
    if (r.genre === "dossier") { fermerAdresse(); aller(r.chemin); }
    else if (r.genre === "fichier") { fermerAdresse(); await aller(parent(r.chemin)); api.appeler("fichiers_ouvrir", { chemin: r.chemin }); }
    else etat(`Ce dossier n'existe pas : ${r.chemin}`);
  }

  // ---- Volet ---------------------------------------------------------------------
  // L'accès rapide, puis UNE entrée « Ce PC » : les disques et les dossiers
  // de Windows sont sur sa page, plus dans le volet (26 novembre).
  async function dessinerVolet() {
    const rapides = await api.appeler("fichiers_rapides");
    S.cePc = await api.appeler("fichiers_ce_pc");
    const ligne = (nom, icone, action) => el("button.fx-entree", { onclick: action }, svg(icone), el("span", nom));
    D.entreeCePc = ligne("Ce PC", "disque", () => aller(CE_PC));
    D.entreeCePc.classList.toggle("actif", S.onglets.length > 0 && enCePc());
    D.volet.replaceChildren(
      el("div.etiquette", "Accès rapide"),
      ...rapides.filter((r) => r.groupe !== "windows").map((r) => ligne(r.nom, iconeRapide(r), () => aller(r.chemin))),
      el("hr.fx-separation"),
      D.entreeCePc);
  }

  // ---- Ce PC -----------------------------------------------------------------------
  const iconeRapide = (r) => (r.icone === "telechargement" ? "fleche" : (ICONES[r.icone] || r.icone));
  const nomDisque = (d) => (d.chemin === "/" ? "grenOS" : d.windows ? "Windows" : d.nom);
  const FORMATS = { vfat: "FAT32", ntfs: "NTFS", ntfs3: "NTFS", exfat: "exFAT", ext4: "ext4", btrfs: "Btrfs", "crypto_LUKS": "chiffré" };

  async function ouvrirLecteur(d) {
    if (d.chemin) { await aller(d.chemin); if (d.conseil) etat(d.conseil); return; }
    if (!d.peripherique) { etat("Ce disque ne peut pas être ouvert d'ici."); return; }
    etat("Ouverture du disque…");
    try {
      const ou = await api.appeler("fichiers_monter", { peripherique: d.peripherique });
      S.cePc = await api.appeler("fichiers_ce_pc");
      await aller(ou);
      if (d.conseil) etat(d.conseil);
    } catch (e) { etat(String(e)); }
  }

  // Une tuile s'ouvre d'un double-clic, ou d'Entrée quand elle a le focus,
  // comme dans le Ce PC de Windows ; un clic la choisit.
  function tuile(nom, icone, lignes, action, classes = "") {
    const b = el(`button.fx-tuile${classes}`, { title: nom, "data-nom": nom.toLowerCase() },
      el("span.fx-tuile-icone", svg(icone)), el("span.fx-tuile-texte", el("b", nom), ...lignes.filter(Boolean)));
    b.addEventListener("click", (e) => { if (e.detail === 0) action(); });
    b.addEventListener("dblclick", action);
    return b;
  }

  // La place prise et la place libre ; un disque pas encore ouvert n'a que sa taille.
  function jauge(d) {
    if (!d.taille) return [];
    if (d.libre == null) return [el("small.fx-tuile-note", `${taille(d.taille)} — pas encore ouvert`)];
    const pris = Math.max(0, Math.min(100, (100 * (d.taille - d.libre)) / d.taille));
    return [el(`span.fx-jauge${pris >= 90 ? ".pleine" : ""}`, el("i", { style: `width:${pris.toFixed(1)}%` })),
      el("small.fx-tuile-note", `${taille(d.libre)} libres sur ${taille(d.taille)}`)];
  }

  async function dessinerCePc() {
    const [rapides, lecteurs] = await Promise.all([api.appeler("fichiers_rapides"), api.appeler("fichiers_ce_pc")]);
    S.cePc = lecteurs;
    const section = (titre, note, grille) => (grille.children.length
      ? el("section.fx-section", el("h3.etiquette", titre), note ? el("p.note.fx-section-note", note) : null, grille) : null);
    const grille = (classes, tuiles) => el(`div.fx-grille${classes}`, ...tuiles);
    const disques = lecteurs.filter((d) => d.icone !== "compte").map((d) => {
      const genre = d.windows ? "Windows · lecture seule" : d.chemin === "/" ? "Système de fichiers" : FORMATS[d.fs] || d.fs || "";
      const t = tuile(nomDisque(d), "disque", [genre ? el("small.fx-tuile-note", genre) : null, ...jauge(d)],
        () => ouvrirLecteur(d), `.disque${d.windows ? ".windows" : ""}`);
      if (d.conseil) t.title = `${nomDisque(d)} — ${d.conseil}`;
      return t;
    });
    D.cePc.replaceChildren(...[
      section("Dossiers", "", grille("", rapides.filter((r) => r.groupe !== "windows").map((r) => tuile(r.nom, iconeRapide(r), [], () => aller(r.chemin))))),
      section("Windows · lecture seule", "Les fichiers de Windows : copie-les dans grenOS pour les modifier.",
        grille("", rapides.filter((r) => r.groupe === "windows").map((r) => tuile(r.nom, iconeRapide(r), [], () => aller(r.chemin), ".windows")))),
      section("Disques et lecteurs", "", grille(".disques", disques)),
    ].filter(Boolean));
    filtrerCePc();
  }

  function filtrerCePc() {
    const f = D.cherche.value.trim().toLowerCase();
    for (const t of D.cePc.querySelectorAll(".fx-tuile")) t.hidden = !!f && !t.dataset.nom.includes(f);
    for (const s of D.cePc.querySelectorAll(".fx-section")) s.hidden = ![...s.querySelectorAll(".fx-tuile")].some((t) => !t.hidden);
  }

  // ---- Boîtes ----------------------------------------------------------------------
  function demander(titre, question, defaut, cache = false) {
    return new Promise((ok) => {
      const champ = el("input.champ", { value: defaut, spellcheck: "false", type: cache ? "password" : "text", autocomplete: "off" });
      const fond = el("div.cx-fond");
      const fin = (v) => { fond.remove(); D.liste.focus(); ok(v); };
      fond.append(el("form.cx-dialogue.carte", { onsubmit: (e) => { e.preventDefault(); fin(champ.value.trim() || null); } },
        el("h2", titre), el("p.note", question), champ,
        el("div.cx-boutons", el("button.bouton.discret", { type: "button", onclick: () => fin(null) }, "Annuler"),
          el("button.bouton.principal", { type: "submit" }, "Valider"))));
      fond.addEventListener("keydown", (e) => { e.stopPropagation(); if (e.key === "Escape") fin(null); });
      document.body.append(fond);
      champ.focus();
      // Le nom sans son extension, comme Windows : on renomme « photo », pas « .jpg ».
      if (!cache) {
        const point = defaut.lastIndexOf(".");
        champ.setSelectionRange(0, point > 0 ? point : defaut.length);
      }
    });
  }

  async function proprietes() {
    const c = choisis();
    if (c.length !== 1) return;
    let lignes;
    try { lignes = await api.appeler("fichiers_proprietes", { chemin: c[0].chemin }); } catch (e) { etat(String(e)); return; }
    const fond = el("div.cx-fond");
    const fermer = el("button.bouton", { onclick: () => { fond.remove(); D.liste.focus(); } }, "Fermer");
    fond.append(el("div.cx-dialogue.carte", el("h2", "Propriétés"),
      el("dl.fx-proprietes", lignes.filter(([, v]) => v).flatMap(([n, v]) => [el("dt", n),
        el("dd", n === "Taille" ? `${taille(Number(v))} (${Number(v).toLocaleString("fr-FR")} octets)` : n === "Modifié le" ? date(Number(v)) : v)])),
      el("div.cx-boutons", fermer)));
    fond.addEventListener("keydown", (e) => { e.stopPropagation(); if (e.key === "Escape" || e.key === "Enter") fermer.click(); });
    document.body.append(fond);
    fermer.focus();
  }

  // ---- Gestes ------------------------------------------------------------------------
  async function nouveauDossier() {
    if (enCePc()) return;
    const nom = await demander("Nouveau dossier", "Nom du dossier :", "Nouveau dossier");
    if (!nom) return;
    try {
      const chemin = await api.appeler("fichiers_nouveau_dossier", { dossier: onglet().dossier, nom });
      await rafraichir();
      choisirChemin(chemin);
      dire(`fichiers : Ctrl+Maj+N -> dossier cree, ${visibles().length} element(s) affiche(s)`);
    } catch (e) { etat(String(e)); }
  }
  async function renommer() {
    const c = choisis();
    if (c.length !== 1) return;
    const nom = await demander("Renommer", "Nouveau nom :", c[0].nom);
    if (!nom) return;
    try {
      const neuf = await api.appeler("fichiers_renommer", { ancien: c[0].chemin, nom });
      if (neuf) { await rafraichir(); choisirChemin(neuf); }
    } catch (e) { etat(String(e)); }
  }
  function copier(couper) {
    S.pressePapier = choisis().map((e) => e.chemin);
    S.couper = couper;
    etat(couper ? `${S.pressePapier.length} élément(s) prêt(s) à être déplacé(s).` : `${S.pressePapier.length} élément(s) copié(s).`);
  }
  async function coller() {
    if (!S.pressePapier.length || enCePc()) return;
    const r = await api.appeler("fichiers_coller", { sources: S.pressePapier, dossier: onglet().dossier, couper: S.couper });
    if (S.couper) { S.pressePapier = []; S.couper = false; }
    await rafraichir();
    etat(r.soucis.length ? `${r.faits} fait(s), ${r.soucis.slice(0, 2).join(" ; ")}` : `${r.faits} élément(s) collé(s).`);
  }
  async function corbeille() {
    const c = choisis();
    if (!c.length) return;
    const r = await api.appeler("fichiers_corbeille", { chemins: c.map((e) => e.chemin) });
    await rafraichir();
    etat(r.refus.length ? `Corbeille impossible pour ${r.refus.length} élément(s) — ${r.refus[0]}` : `${r.faits} élément(s) mis à la corbeille.`);
  }
  async function basculerCaches() {
    S.montrerCaches = !S.montrerCaches;
    await rafraichir();
    dire(`fichiers : Ctrl+H -> ${S.montrerCaches ? "caches montres" : "caches remasques"}, ${visibles().length} element(s) affiche(s)`);
  }
  async function archive(geste) {
    etat(geste === "compresser" ? "Compression en cours…" : "Extraction en cours…");
    try {
      const [quoi, nom] = (await api.appeler("fichiers_archive", { geste, choisis: choisis().map((e) => e.chemin), dossier: onglet().dossier })).split("|");
      await rafraichir();
      etat(`« ${nom} » est prêt.`);
      dire(`fichiers : ${quoi} -> ${nom}, ${visibles().length} element(s) affiche(s)`);
    } catch (e) { etat(String(e)); }
  }
  const estArchive = (c) => /\.(zip|7z|rar|tar|gz|bz2|xz|tgz)$/i.test(c);
  const estChiffre = (c) => /\.(gpg|pgp)$/i.test(c);
  const lisibleOcr = (c) => /\.(png|jpe?g|tiff?|bmp|webp|pdf)$/i.test(c);

  // H6 : l'OCR, sur la machine. Un .txt à côté de l'image, ou un PDF cherchable.
  async function ocr() {
    const c = choisis();
    if (c.length !== 1 || !lisibleOcr(c[0].chemin)) return;
    etat("Lecture du texte… (une page scannée prend quelques secondes)");
    try {
      const nom = await api.appeler("fichiers_ocr", { chemin: c[0].chemin });
      await rafraichir();
      choisirChemin(`${onglet().dossier.replace(/\/$/, "")}/${nom}`);
      etat(`« ${nom} » est prêt. Le texte a été lu sur cette machine : rien n'est parti sur Internet.`);
    } catch (e) { etat(String(e)); }
  }

  // B6 : une copie chiffrée, l'original gardé. La phrase est demandée deux
  // fois : une faute de frappe ici, et le fichier ne se rouvrirait jamais.
  async function chiffrer() {
    const c = choisis();
    if (c.length !== 1 || c[0].dossier) { etat("Choisis un seul fichier à chiffrer (un dossier : compresse-le d'abord)."); return; }
    const p1 = await demander("Chiffrer", `Phrase de passe pour « ${c[0].nom} » (8 caractères au moins) :`, "", true);
    if (!p1) return;
    const p2 = await demander("Chiffrer", "La même, pour être sûr :", "", true);
    if (p2 !== p1) { etat("Les deux phrases ne sont pas identiques : rien n'a été chiffré."); return; }
    etat("Chiffrement…");
    try {
      const nom = await api.appeler("fichiers_chiffrer", { chemin: c[0].chemin, phrase: p1 });
      await rafraichir();
      etat(`« ${nom} » est prêt. L'original est toujours là : supprime-le quand tu as vérifié que la copie s'ouvre.`);
    } catch (e) { etat(String(e)); }
  }
  async function dechiffrer() {
    const c = choisis();
    if (c.length !== 1) return;
    const p = await demander("Déchiffrer", `Phrase de passe de « ${c[0].nom} » :`, "", true);
    if (!p) return;
    etat("Déchiffrement…");
    try {
      const nom = await api.appeler("fichiers_dechiffrer", { chemin: c[0].chemin, phrase: p });
      await rafraichir();
      etat(`« ${nom} » est déchiffré, à côté du fichier chiffré.`);
    } catch (e) { etat(String(e)); }
  }

  function menu(ev) {
    document.querySelector(".fx-menu")?.remove();
    const c = choisis();
    const entree = (texte, action, actif = true, raccourci = "") =>
      el("button", { disabled: !actif, onclick: () => { m.remove(); action(); } }, el("span", texte), raccourci ? el("small", raccourci) : null);
    const trait = () => el("hr");
    const m = el("div.fx-menu.carte", ...(c.length ? [
      entree("Ouvrir", () => ouvrir(c[0])),
      entree("Renommer", renommer, c.length === 1, "F2"), trait(),
      entree("Copier", () => copier(false), true, "Ctrl+C"),
      entree("Couper", () => copier(true), true, "Ctrl+X"),
      entree("Coller", coller, S.pressePapier.length > 0, "Ctrl+V"), trait(),
      c.every((e) => estArchive(e.chemin)) ? entree("Extraire ici", () => archive("extraire")) : entree("Compresser en .zip", () => archive("compresser")),
      c.length === 1 && estChiffre(c[0].chemin) ? entree("Déchiffrer…", dechiffrer) : entree("Chiffrer…", chiffrer, c.length === 1 && !c[0].dossier),
      c.length === 1 && lisibleOcr(c[0].chemin) ? entree("Extraire le texte (OCR)", ocr) : null, trait(),
      entree("Mettre à la corbeille", corbeille, true, "Suppr"), trait(),
      entree("Propriétés", proprietes, c.length === 1),
    ] : [
      entree("Nouveau dossier", nouveauDossier, true, "Ctrl+Maj+N"),
      entree("Nouvelle fenêtre", nouvelleFenetre, true, "Ctrl+N"),
      entree("Coller", coller, S.pressePapier.length > 0, "Ctrl+V"), trait(),
      entree(S.montrerCaches ? "Masquer les fichiers cachés" : "Afficher les fichiers cachés", basculerCaches, true, "Ctrl+H"),
      entree("Actualiser", rafraichir, true, "F5"),
    ]));
    document.body.append(m);
    const { innerWidth: l, innerHeight: h } = window;
    m.style.left = `${Math.min(ev.clientX, l - m.offsetWidth - 6)}px`;
    m.style.top = `${Math.min(ev.clientY, h - m.offsetHeight - 6)}px`;
    setTimeout(() => document.addEventListener("mousedown", function hors(e) { if (!m.contains(e.target)) { m.remove(); document.removeEventListener("mousedown", hors); } }), 0);
  }
  function nouvelleFenetre() { api.appeler("fichiers_nouvelle_fenetre", { dossier: onglet().dossier }); }

  // ---- Clavier -------------------------------------------------------------------------
  function clavier(e) {
    if (document.querySelector(".cx-fond")) return;
    if (e.key === "Escape") document.querySelector(".fx-menu")?.remove();
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const c = e.ctrlKey, a = e.altKey, m = e.shiftKey;
    const fait = () => { e.preventDefault(); e.stopPropagation(); };
    if (a && k === "ArrowLeft") return fait(), precedent();
    if (a && k === "ArrowRight") return fait(), suivant();
    if (a && k === "ArrowUp") return fait(), auParent();
    if (a && k === "p") return fait(), basculerApercu();
    if (k === "F5") return fait(), rafraichir();
    if (c && k === "f") return fait(), D.cherche.focus();
    if (k === "F4" || (c && k === "l")) return fait(), ouvrirAdresse();
    if (k === "Escape") {
      if (!D.adresse.hidden) return fait(), fermerAdresse();
      if (document.activeElement === D.cherche || D.cherche.value) { fait(); D.cherche.value = ""; filtrer(); D.liste.focus(); return; }
    }
    if (c && k === "t") return fait(), ouvrirOnglet();
    if (c && k === "w") return fait(), fermerOnglet();
    if (c && k === "Tab") { fait(); S.actif = (S.actif + (m ? -1 : 1) + S.onglets.length) % S.onglets.length; return changerDOnglet(); }
    if (c && /^[1-9]$/.test(k)) { fait(); const r = k === "9" ? S.onglets.length - 1 : Number(k) - 1; if (r < S.onglets.length) { S.actif = r; changerDOnglet(); } return; }
    if (c && m && k === "n") return fait(), nouveauDossier();
    if (c && k === "n") return fait(), nouvelleFenetre();
    if (document.activeElement === D.cherche || document.activeElement === D.adresse) return;
    if (k === "Backspace") return fait(), precedent();
    if (c && k === "h") return fait(), basculerCaches();
    if (k === "F2") return fait(), renommer();
    if (k === "Delete") return fait(), corbeille();
    if (c && k === "c") return fait(), copier(false);
    if (c && k === "x") return fait(), copier(true);
    if (c && k === "v") return fait(), coller();
    if (c && k === "a") { fait(); S.choisis = new Set(visibles().map((x) => x.chemin)); return dessiner(); }
    if (k === "Enter") { const x = choisis(); if (x.length) { fait(); ouvrir(x[0]); } return; }
    if (k === "ArrowDown" || k === "ArrowUp") {
      fait();
      const v = visibles();
      if (!v.length) return;
      const i = Math.min(v.length - 1, Math.max(0, S.ancre + (k === "ArrowDown" ? 1 : -1)));
      choisirChemin(v[i].chemin);
    }
  }

  function filtrer() {
    if (enCePc()) return filtrerCePc();
    dessiner();
    const f = D.cherche.value.trim();
    if (f) dire(`fichiers : recherche « ${f} » -> ${visibles().length} element(s)`);
  }

  // ---- Cadre ---------------------------------------------------------------------------
  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    S.maison = await api.appeler("fichiers_maison");
    const depart = window.GRENOS_ONGLET || S.maison;
    S.onglets = [nouvelOnglet(depart)];
    const outil = (icone, titre, f) => el("button.fx-outil", { title: titre, onclick: f }, svg(icone));
    D.precedent = outil("retour", "Précédent (Alt+Gauche, Retour arrière)", precedent);
    D.suivant = outil("fleche", "Suivant (Alt+Droite)", suivant);
    D.parent = el("button.fx-outil.fx-haut", { title: "Dossier parent (Alt+Haut)", onclick: auParent }, svg("fleche"));
    D.fil = el("div.fx-fil", { onclick: ouvrirAdresse });
    D.adresse = el("input.fx-adresse", { hidden: true, spellcheck: "false" });
    D.adresse.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); adresseTapee(); } });
    D.adresse.addEventListener("blur", () => setTimeout(() => { if (!D.adresse.hidden && document.activeElement !== D.adresse) fermerAdresse(); }, 150));
    D.cherche = el("input.fx-cherche", { type: "search", placeholder: "Chercher ici (Ctrl+F)", spellcheck: "false" });
    D.cherche.addEventListener("input", filtrer);
    D.onglets = el("nav.fx-onglets", { hidden: true });
    D.volet = el("aside.fx-volet");
    D.corps = el("tbody");
    D.entetes = [["nom", "Nom"], ["modifie", "Modifié le"], ["genre", "Type"], ["octets", "Taille"]].map(([cle, t]) => {
      const th = el(`th${cle === "octets" ? ".nb" : ""}`, { "data-cle": cle }, t);
      th.addEventListener("click", () => { S.tri = { cle, sens: S.tri.cle === cle ? -S.tri.sens : 1 }; dessiner(); });
      return th;
    });
    D.liste = el("div.fx-liste", { tabindex: "0" }, el("table.fx-table", el("thead", el("tr", ...D.entetes)), D.corps));
    D.liste.addEventListener("contextmenu", (ev) => { if (ev.target.closest("tr[data-i]")) return; ev.preventDefault(); S.choisis.clear(); dessiner(); menu(ev); });
    D.liste.addEventListener("mousedown", (ev) => { if (!ev.target.closest("tr[data-i]") && !ev.target.closest("th") && ev.button === 0) { S.choisis.clear(); dessiner(); } });
    D.etat = el("footer.fx-etat");
    D.apercu = el("aside.fx-apercu", { hidden: true });
    D.cePc = el("div.fx-cepc", { hidden: true, tabindex: "-1" });
    D.cePc.addEventListener("contextmenu", (ev) => ev.preventDefault());
    D.milieu = el("div.fx-milieu", D.volet, D.liste, D.cePc, D.apercu);
    const boutonApercu = el("button.fx-outil", { title: "Volet d'aperçu (Alt+P)", onclick: basculerApercu }, svg("oeil"));
    document.body.append(el("div.page-fichiers",
      el("header.fx-barre", D.precedent, D.suivant, D.parent, el("div.fx-adresse-cadre", D.fil, D.adresse), D.cherche, boutonApercu),
      D.onglets,
      D.milieu,
      D.etat));
    document.addEventListener("keydown", clavier, true);
    await dessinerVolet();
    if (!(await aller(depart))) await aller(S.maison);
    D.liste.focus();
    pret();
    dire(`fichiers ouvert, ${S.elements.length} element(s) dans ${nomDe(onglet().dossier)}, ${S.cePc.length} entree(s) dans Ce PC`);
  })();
})();
