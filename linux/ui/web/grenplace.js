// GrenPlace : un volet de rayons à gauche (comme le magasin de Windows), la
// recherche, et une fiche par application avec son bouton. Le catalogue vient
// de notre backend ; les applications et leurs logos viennent de Flathub ou
// de Debian, jamais de chez nous.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, installer } = window.grenos;
  const COULEURS = { "Jeux": "#7c4df0", "Bureautique": "#2563eb", "Création": "#d9488a", "Internet": "#0ea5a3", "Développement": "#f2820c", "Système": "#5a6880", "Intelligence artificielle": "#16a34a" };
  const D = {};
  let applications = [];
  let origine = "";
  let rayon = "Tout";
  let filtre = "";
  // null : on ne sait pas encore. [] : tout est à jour. Les confondre ferait
  // écrire « tout est à jour » avant d'avoir regardé.
  let retards = null;

  function logo(a) {
    const lettre = (a.nom || "?").trim().slice(0, 1).toUpperCase();
    const tuile = el("span.gp-tuile", { style: `background:${COULEURS[a.rayon] || "var(--accent)"}` }, lettre);
    if (!a.logo) return tuile;
    const img = el("img.gp-logo", { src: a.logo, alt: "", loading: "lazy" });
    img.addEventListener("error", () => img.replaceWith(tuile));
    return img;
  }

  async function poser(a, miseAJour) {
    const fait = await installer({ nom: a.nom, source: a.source, identifiant: a.identifiant, miseAJour });
    if (fait) {
      a.installe = true;
      if (miseAJour && retards) retards = retards.filter((s) => s !== a.slug);
    } else {
      a.installe = await api.appeler("grenplace_installe", { source: a.source, identifiant: a.identifiant });
    }
    remplir();
  }

  function fiche(a, miseAJour) {
    const depuis = a.source === "apt" ? "paquet Debian" : "Flatpak, depuis Flathub";
    let action;
    if (miseAJour) action = el("button.bouton.principal.petit", { onclick: () => poser(a, true) }, "Mettre à jour");
    else if (a.installe) action = el("span.gp-installe", svg("coche"), "Installé");
    else action = el("button.bouton.principal.petit", { onclick: () => poser(a, false) }, "Installer");
    return el("article.gp-fiche.carte.verre", logo(a),
      el("div.gp-texte", el("strong", a.nom), el("p", a.description || ""),
        el("small.etiquette", [a.rayon, depuis, a.editeur].filter(Boolean).join(" · "))),
      action);
  }

  function correspond(a) {
    return !filtre || `${a.nom} ${a.description || ""}`.toLowerCase().includes(filtre);
  }

  function remplir() {
    D.sous.textContent = `${applications.length} applications · catalogue ${origine} · rien n'est stocké chez nous, tout vient de Flathub ou de Debian`;
    for (const b of D.volet.querySelectorAll("button")) b.classList.toggle("actif", b.dataset.rayon === rayon);
    D.compteur.textContent = retards && retards.length ? String(retards.length) : "";
    if (rayon === "Mises à jour") {
      if (retards == null) {
        D.liste.replaceChildren(el("p.carte.verre.note", "Lecture… On demande à Flathub et à Debian ce qui a changé depuis la dernière fois."));
      } else if (!retards.length) {
        D.liste.replaceChildren(el("p.carte.verre.note", "Tout est à jour. Les applications installées sont à leur dernière version."));
      } else {
        const liste = applications.filter((a) => retards.includes(a.slug));
        D.liste.replaceChildren(
          el("div.gp-tete-maj.carte.verre", el("strong", `${liste.length} application${liste.length > 1 ? "s" : ""} à mettre à jour`),
            el("button.bouton.principal.petit", { onclick: toutMettreAJour }, "Tout mettre à jour")),
          ...liste.filter(correspond).map((a) => fiche(a, true)));
      }
      return;
    }
    const montrees = applications.filter((a) => (rayon === "Tout" || a.rayon === rayon) && correspond(a));
    D.liste.replaceChildren(...(montrees.length
      ? montrees.map((a) => fiche(a, false))
      : [el("p.carte.verre.note", applications.length ? "Rien ne correspond." : "Le catalogue n'a pas pu être lu.")]));
  }

  // Une par une, et SEULEMENT celles-là : `flatpak update -y` toucherait aussi
  // ce qui ne vient pas de notre catalogue, sans le dire.
  async function toutMettreAJour() {
    for (const slug of [...(retards || [])]) {
      const a = applications.find((x) => x.slug === slug);
      if (a) await poser(a, true);
    }
    chercherRetards();
  }

  async function chercherRetards() {
    retards = null;
    remplir();
    const installees = applications.filter((a) => a.installe);
    try { retards = await api.appeler("grenplace_retards", { applications: installees }); }
    catch (e) { retards = []; }
    remplir();
  }

  function volet() {
    const rayons = ["Tout", ...new Set(applications.map((a) => a.rayon).filter(Boolean))];
    const bouton = (nom, texte) => el("button", { "data-rayon": nom, onclick: () => { rayon = nom; remplir(); } }, texte);
    D.compteur = el("span.gp-pastille");
    D.volet.replaceChildren(...rayons.map((r) => bouton(r, r === "Tout" ? "Accueil" : r)), el("hr"),
      el("button", { "data-rayon": "Mises à jour", onclick: () => { rayon = "Mises à jour"; remplir(); } }, "Mises à jour", D.compteur));
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.sous = el("small.note", "Chargement du catalogue…");
    D.volet = el("nav.gp-volet.carte.verre");
    D.compteur = el("span");
    D.liste = el("div.gp-liste");
    const recherche = el("input.champ.gp-recherche", { type: "search", placeholder: "Chercher une application" });
    recherche.addEventListener("input", () => { filtre = recherche.value.trim().toLowerCase(); remplir(); });
    document.body.append(el("div.page-grenplace", D.volet,
      el("main.gp-colonne",
        el("header.tete", el("div.logo", svg("paquet")), el("div", el("h1", "GrenPlace"), D.sous)),
        recherche, D.liste)));
    pret();
    try {
      const v = await api.appeler("grenplace_catalogue");
      applications = (v && v.applications) || [];
      origine = (v && v.origine) || "introuvable";
    } catch (e) {
      origine = "introuvable";
    }
    volet();
    remplir();
    recherche.focus();
    chercherRetards();
  })();
})();
