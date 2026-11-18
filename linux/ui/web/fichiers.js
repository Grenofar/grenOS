// L'Explorateur de fichiers : « Accès rapide » puis « Ce PC », un fil
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
    choisis: new Set(), ancre: -1, tri: { cle: "nom", sens: 1 },
    pressePapier: [], couper: false, maison: "/", cePc: [],
  };
  const nombre = (v, d = 1) => v.toFixed(d).replace(".", ",");
  const dire = (t) => api.appeler("fichiers_dire", { texte: t });
  const base = (c) => c.replace(/\/+$/, "").split("/").pop() || "/";
  const parent = (c) => { const p = c.replace(/\/+$/, "").split("/").slice(0, -1).join("/"); return p || "/"; };
  const joindre = (d, n) => (d === "/" ? "/" : d + "/") + n;
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
      el(`button${i === S.actif ? ".actif" : ""}`, { onclick: () => { S.actif = i; changerDOnglet(); } }, base(o.dossier))),
      el("button.fx-fermer-onglet", { title: "Fermer cet onglet (Ctrl+W)", onclick: fermerOnglet }, "×"));
  }

  // La recherche est effacée en changeant d'onglet : garder le filtre d'un
  // onglet sur un autre montrerait une liste amputée sans qu'on sache pourquoi.
  async function changerDOnglet() {
    if (D.cherche.value) D.cherche.value = "";
    await aller(onglet().dossier, false);
    dessinerOnglets();
    D.liste.focus();
    dire(`fichiers : onglets — ${S.onglets.length}, actif « ${base(onglet().dossier)} »`);
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
    let liste;
    try { liste = await api.appeler("fichiers_lister", { dossier: chemin, caches: S.montrerCaches }); }
    catch (e) { etat(String(e)); return false; }
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
    D.parent.disabled = chemin === "/";
    api.appeler("fichiers_titre", { titre: `${chemin === "/" ? "Système de fichiers" : base(chemin)} — Explorateur de fichiers` });
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
  function auParent() { const d = onglet().dossier; if (d !== "/") aller(parent(d)); }

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
  }

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
    if (d === S.maison || d.startsWith(S.maison + "/")) {
      morceaux = [[S.cePc[0]?.nom || "Dossier personnel", S.maison]]; reste = d.slice(S.maison.length); courant = S.maison;
    } else {
      morceaux = [["Système de fichiers", "/"]]; reste = d; courant = "";
    }
    for (const m of reste.split("/").filter(Boolean)) { courant = `${courant}/${m}`; morceaux.push([m, courant]); }
    D.fil.replaceChildren(...morceaux.flatMap(([t, c], i) => [
      i ? el("span.fx-fleche", "›") : null,
      el("button.fx-miette", { onclick: (ev) => { ev.stopPropagation(); aller(c); } }, t)]));
  }
  function ouvrirAdresse() {
    D.adresse.value = onglet().dossier;
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
  async function dessinerVolet() {
    const rapides = await api.appeler("fichiers_rapides");
    S.cePc = await api.appeler("fichiers_ce_pc");
    const ligne = (nom, icone, action) => el("button.fx-entree", { onclick: action }, svg(icone), el("span", nom));
    D.volet.replaceChildren(
      el("div.etiquette", "Accès rapide"),
      ...rapides.map((r) => ligne(r.nom, r.icone === "telechargement" ? "fleche" : (ICONES[r.icone] || r.icone), () => aller(r.chemin))),
      el("div.etiquette", "Ce PC"),
      ...S.cePc.map((d) => ligne(d.nom, d.icone, async () => {
        if (d.chemin) { aller(d.chemin); return; }
        if (!d.peripherique) { etat("Ce disque ne peut pas être ouvert d'ici."); return; }
        etat("Ouverture du disque…");
        try { const ou = await api.appeler("fichiers_monter", { peripherique: d.peripherique }); await aller(ou); dessinerVolet(); }
        catch (e) { etat(String(e)); }
      })));
  }

  // ---- Boîtes ----------------------------------------------------------------------
  function demander(titre, question, defaut) {
    return new Promise((ok) => {
      const champ = el("input.champ", { value: defaut, spellcheck: "false" });
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
      const point = defaut.lastIndexOf(".");
      champ.setSelectionRange(0, point > 0 ? point : defaut.length);
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
    if (!S.pressePapier.length) return;
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
      c.every((e) => estArchive(e.chemin)) ? entree("Extraire ici", () => archive("extraire")) : entree("Compresser en .zip", () => archive("compresser")), trait(),
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
    document.body.append(el("div.page-fichiers",
      el("header.fx-barre", D.precedent, D.suivant, D.parent, el("div.fx-adresse-cadre", D.fil, D.adresse), D.cherche),
      D.onglets,
      el("div.fx-milieu", D.volet, D.liste),
      D.etat));
    document.addEventListener("keydown", clavier, true);
    await dessinerVolet();
    if (!(await aller(depart))) await aller(S.maison);
    D.liste.focus();
    pret();
    dire(`fichiers ouvert, ${S.elements.length} element(s) dans ${base(onglet().dossier)}, ${S.cePc.length} entree(s) dans Ce PC`);
  })();
})();
