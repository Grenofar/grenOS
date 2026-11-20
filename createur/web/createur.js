// Le créateur de clé grenOS : choisir la clé, vérifier la version, créer.
"use strict";
(function () {
  // L'APERÇU, dans un simple navigateur (sans Tauri) : des clés et une
  // version factices, et une création simulée — pour juger l'écran. Le vrai
  // créateur a toujours window.__TAURI__ et n'entre jamais ici.
  function apercu() {
    let ecouteur = () => {};
    const attendre = (ms) => new Promise((r) => setTimeout(r, ms));
    return {
      core: {
        invoke: async (c) => {
          await attendre(300);
          if (c === "cles") return [
            { numero: 3, nom: "SanDisk Ultra USB 3.0", taille: 30752636928, lettres: ["G"] },
            { numero: 4, nom: "Kingston DataTraveler", taille: 2013265920, lettres: ["H"] }];
          if (c === "derniere_version") return { nom: "20261121-0110", fichier: "grenos.iso", adresse: "", taille: 1868000000, empreinte: "" };
          if (c === "creer") {
            (async () => {
              for (const [etape, n] of [["telecharger", 8], ["verifier-iso", 3], ["preparer", 1], ["copier", 6], ["verifier", 4]]) {
                for (let i = 0; i <= n; i++) { ecouteur({ payload: { etape, fait: i, total: n, texte: etape === "copier" ? "live/filesystem.squashfs" : "" } }); await attendre(220); }
              }
              ecouteur({ payload: { etape: "fini", fait: 1, total: 1, texte: "G:" } });
            })();
          }
          return null;
        },
      },
      event: { listen: async (_n, f) => { ecouteur = f; } },
    };
  }
  const T = window.__TAURI__ || apercu();
  const invoke = (c, a) => T.core.invoke(c, a);
  const $ = (id) => document.getElementById(id);
  const ETAPES = ["telecharger", "verifier-iso", "preparer", "copier", "verifier"];
  const MIN = 4 * 1e9; // l'ISO pèse ~1,8 Go : 4 Go au moins, avec de la marge

  const etat = { cles: [], choisie: null, version: null };

  function taille(o) {
    if (!o) return "?";
    const go = o / 1e9;
    return go >= 1 ? `${go.toLocaleString("fr-FR", { maximumFractionDigits: 1 })} Go`
      : `${Math.round(o / 1e6).toLocaleString("fr-FR")} Mo`;
  }
  function el(balise, attributs, ...enfants) {
    const n = document.createElement(balise);
    for (const [k, v] of Object.entries(attributs || {})) {
      if (k.startsWith("on")) n.addEventListener(k.slice(2), v);
      else if (v !== false && v != null) n.setAttribute(k, v === true ? "" : v);
    }
    for (const e of enfants.flat()) if (e != null) n.append(e);
    return n;
  }
  const ICONE_USB = '<svg viewBox="0 0 24 24" fill="none" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="7" y="2" width="10" height="9" rx="1.5"/><path d="M5 11h14v7a4 4 0 0 1-4 4H9a4 4 0 0 1-4-4z"/><path d="M10 5.5h1M13 5.5h1"/></svg>';

  function peutCreer() {
    $("creer").disabled = !(etat.choisie && etat.version);
  }

  async function chargerCles() {
    const zone = $("cles");
    zone.replaceChildren(el("p", { class: "note" }, "Recherche des clés USB…"));
    try {
      etat.cles = await invoke("cles");
    } catch (e) {
      zone.replaceChildren(el("p", { class: "note" }, `Impossible de lister les clés : ${e}`));
      return;
    }
    if (etat.choisie && !etat.cles.some((c) => c.numero === etat.choisie.numero)) etat.choisie = null;
    if (!etat.cles.length) {
      zone.replaceChildren(el("p", { class: "note" }, "Aucune clé USB branchée. Branche une clé de 8 Go ou plus, puis « Actualiser »."));
      peutCreer();
      return;
    }
    zone.replaceChildren(...etat.cles.map((c) => {
      const petite = c.taille < MIN;
      const icone = el("span", { class: "icone" });
      icone.innerHTML = ICONE_USB;
      return el("button", {
        class: "cle", type: "button", role: "radio",
        "aria-checked": String(etat.choisie && etat.choisie.numero === c.numero), disabled: petite,
        onclick: () => { etat.choisie = c; chargerCles.dessiner(); peutCreer(); },
      }, icone,
      el("span", {}, el("b", {}, c.nom || "Clé USB"),
        el("small", {}, `${taille(c.taille)}${c.lettres.length ? " · " + c.lettres.map((l) => l + ":").join(" ") : ""}${petite ? " · trop petite (4 Go au moins)" : ""}`)),
      el("span", { class: "coche" }));
    }));
    peutCreer();
  }
  chargerCles.dessiner = () => {
    for (const b of document.querySelectorAll(".cle")) b.setAttribute("aria-checked", "false");
    const i = etat.cles.findIndex((c) => etat.choisie && c.numero === etat.choisie.numero);
    const b = document.querySelectorAll(".cle")[i];
    if (b) b.setAttribute("aria-checked", "true");
  };

  async function chargerVersion() {
    const zone = $("version");
    try {
      etat.version = await invoke("derniere_version");
      zone.replaceChildren(el("b", {}, `grenOS ${etat.version.nom}`),
        el("small", {}, `${taille(etat.version.taille)} · téléchargée depuis GitHub, puis vérifiée par son empreinte`));
    } catch (e) {
      zone.replaceChildren(el("p", { class: "note" }, `La dernière version n'a pas pu être trouvée (${e}). Vérifie la connexion, puis relance le créateur.`));
    }
    peutCreer();
  }

  function etapeVers(nom, classe) {
    const i = ETAPES.indexOf(nom);
    document.querySelectorAll("#etapes li").forEach((li, j) => {
      li.classList.remove("encours", "fait", "erreur");
      if (j < i) li.classList.add("fait");
      else if (j === i) li.classList.add(classe);
    });
  }

  function finir(reussi, texte) {
    const fin = $("fin");
    fin.hidden = false;
    fin.className = reussi ? "reussi" : "rate";
    if (reussi) {
      document.querySelectorAll("#etapes li").forEach((li) => { li.classList.remove("encours"); li.classList.add("fait"); });
      fin.replaceChildren(el("h2", {}, "La clé est prête"),
        el("p", { class: "note" }, "Redémarre ton PC et choisis la clé dans le menu de démarrage (F12, F11, F8 ou Échap selon la marque de la carte mère). Le Secure Boot peut rester activé."),
        el("div", { class: "actions" }, el("button", { class: "bouton", type: "button", onclick: () => location.reload() }, "Créer une autre clé")));
    } else {
      const li = document.querySelector("#etapes li.encours");
      if (li) { li.classList.remove("encours"); li.classList.add("erreur"); }
      fin.replaceChildren(el("h2", {}, "La clé n'a pas pu être créée"), el("p", { class: "note" }, texte),
        el("div", { class: "actions" }, el("button", { class: "bouton principal", type: "button", onclick: () => location.reload() }, "Recommencer")));
    }
  }

  function demarrer() {
    $("confirmer").hidden = true;
    $("choix").hidden = true;
    $("travail").hidden = false;
    $("travail-titre").textContent = `Création de la clé · ${etat.choisie.nom} (${taille(etat.choisie.taille)})`;
    etapeVers("telecharger", "encours");
    invoke("creer", { numero: etat.choisie.numero, version: etat.version }).catch((e) => finir(false, String(e)));
  }

  T.event.listen("avancee", ({ payload: a }) => {
    if (a.etape === "fini") return finir(true, a.texte);
    if (a.etape === "erreur") return finir(false, a.texte);
    if (!ETAPES.includes(a.etape)) return;
    etapeVers(a.etape, "encours");
    const li = document.querySelector(`#etapes li[data-etape="${a.etape}"]`);
    if (!li) return;
    const pourcent = a.total ? Math.min(100, (a.fait / a.total) * 100) : 0;
    const barre = li.querySelector(".barre i");
    if (barre) barre.style.width = `${pourcent.toFixed(1)}%`;
    const petit = li.querySelector("small");
    if (a.etape === "telecharger" && a.total) petit.textContent = `${taille(a.fait)} sur ${taille(a.total)}${a.texte ? " · " + a.texte : ""}`;
    else if (a.etape === "copier" && a.total) petit.textContent = `${Math.round(pourcent)} % · ${a.texte}`;
    else if (a.etape === "verifier" && a.total) petit.textContent = `${a.fait} fichiers sur ${a.total}`;
    else if (a.texte) petit.textContent = a.texte;
  });

  $("actualiser").addEventListener("click", chargerCles);
  $("creer").addEventListener("click", () => {
    $("confirmer-texte").textContent = `Tout ce qui est sur « ${etat.choisie.nom} » (${taille(etat.choisie.taille)}) sera effacé, puis grenOS ${etat.version.nom} y sera mis.`;
    $("confirmer").hidden = false;
  });
  $("annuler").addEventListener("click", () => { $("confirmer").hidden = true; });
  $("valider").addEventListener("click", demarrer);

  chargerCles();
  chargerVersion();
})();
