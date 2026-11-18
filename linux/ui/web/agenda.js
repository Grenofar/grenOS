// L'Agenda : le mois, les jours fériés français, et les événements de la
// personne (agenda.rs les garde dans un fichier à elle).
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, toast } = window.grenos;
  const MOIS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
  const JOURS = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
  const z = (n) => String(n).padStart(2, "0");
  const cle = (d) => `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}`;
  const S = { vu: new Date(), choisi: cle(new Date()), evenements: [] };
  S.vu.setDate(1);
  const D = {};

  // Pâques (algorithme de Meeus) : Pâques, l'Ascension et la Pentecôte en
  // dépendent. Les fériés fixes de France à côté.
  function paques(a) {
    const b = a % 19, c = Math.floor(a / 100), d = a % 100, e = Math.floor(c / 4), f = c % 4,
      g = Math.floor((c + 8) / 25), h = Math.floor((c - g + 1) / 3), i = (19 * b + c - e - h + 15) % 30,
      k = Math.floor(d / 4), l = d % 4, m = (32 + 2 * f + 2 * k - i - l) % 7, n = Math.floor((b + 11 * i + 22 * m) / 451),
      mois = Math.floor((i + m - 7 * n + 114) / 31), jour = ((i + m - 7 * n + 114) % 31) + 1;
    return new Date(a, mois - 1, jour);
  }
  function feries(a) {
    const p = paques(a);
    const plus = (j) => { const d = new Date(p); d.setDate(d.getDate() + j); return cle(d); };
    return {
      [`${a}-01-01`]: "Jour de l'an", [plus(1)]: "Lundi de Pâques", [`${a}-05-01`]: "Fête du Travail", [`${a}-05-08`]: "Victoire 1945",
      [plus(39)]: "Ascension", [plus(50)]: "Lundi de Pentecôte", [`${a}-07-14`]: "Fête nationale", [`${a}-08-15`]: "Assomption",
      [`${a}-11-01`]: "Toussaint", [`${a}-11-11`]: "Armistice", [`${a}-12-25`]: "Noël",
    };
  }

  function dessinerMois() {
    const a = S.vu.getFullYear(), m = S.vu.getMonth();
    const F = feries(a);
    D.titre.textContent = `${MOIS[m]} ${a}`;
    const premier = new Date(a, m, 1);
    const decalage = (premier.getDay() + 6) % 7; // lundi d'abord
    const debut = new Date(a, m, 1 - decalage);
    const aujourdhui = cle(new Date());
    const cases = [];
    for (let i = 0; i < 42; i++) {
      const d = new Date(debut); d.setDate(debut.getDate() + i);
      const k = cle(d);
      const nb = S.evenements.filter((e) => e.date === k).length;
      const classes = [d.getMonth() !== m ? "autre" : "", k === aujourdhui ? "aujourdhui" : "", k === S.choisi ? "choisi" : "", F[k] ? "ferie" : "", (i % 7) > 4 ? "weekend" : ""].filter(Boolean).join(".");
      const c = el(`button.ag-jour${classes ? "." + classes : ""}`, { title: F[k] || "" },
        el("span.ag-num", String(d.getDate())),
        F[k] ? el("small.ag-ferie", F[k]) : null,
        nb ? el("span.ag-points", "•".repeat(Math.min(nb, 3))) : null);
      c.addEventListener("click", () => { S.choisi = k; if (d.getMonth() !== m) { S.vu = new Date(d.getFullYear(), d.getMonth(), 1); } dessiner(); });
      cases.push(c);
    }
    D.grille.replaceChildren(...JOURS.map((j) => el("div.ag-entete", j)), ...cases);
  }

  function dessinerJour() {
    const [a, m, j] = S.choisi.split("-").map(Number);
    const d = new Date(a, m - 1, j);
    const F = feries(a);
    D.jour.textContent = d.toLocaleDateString("fr-FR", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
    const du = S.evenements.filter((e) => e.date === S.choisi);
    // filter(Boolean) : replaceChildren écrit « null » en toutes lettres
    // pour un enfant absent (vu dans l'aperçu le 18 novembre).
    D.liste.replaceChildren(...[
      F[S.choisi] ? el("div.ag-evt.ferie", el("strong", F[S.choisi]), el("small", "jour férié")) : null,
      ...du.map((e) => el("div.ag-evt", el("span.ag-heure", e.heure || "journée"),
        el("div", el("strong", e.titre), e.note ? el("small", e.note) : null),
        el("button.bouton.discret.petit", { title: "Retirer", onclick: () => retirer(e.id) }, "✕"))),
      !du.length && !F[S.choisi] ? el("p.note", "Rien de prévu.") : null].filter(Boolean));
  }

  function dessiner() { dessinerMois(); dessinerJour(); }

  async function ajouter(ev) {
    ev.preventDefault();
    const titre = D.champTitre.value.trim();
    if (!titre) { D.champTitre.focus(); return; }
    const e = { id: `${Date.now()}-${Math.floor(Math.random() * 1e6)}`, date: S.choisi, heure: D.champHeure.value, titre, note: D.champNote.value.trim() };
    try {
      S.evenements = await api.appeler("agenda_poser", { evenement: e });
      D.champTitre.value = D.champNote.value = ""; D.champHeure.value = "";
      dessiner();
    } catch (x) { toast(String(x)); }
  }
  async function retirer(id) {
    try { S.evenements = await api.appeler("agenda_retirer", { id }); dessiner(); } catch (x) { toast(String(x)); }
  }
  function decaler(n) { S.vu = new Date(S.vu.getFullYear(), S.vu.getMonth() + n, 1); dessinerMois(); }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.titre = el("h1.ag-titre");
    D.grille = el("div.ag-grille");
    D.jour = el("h2.ag-jour-titre");
    D.liste = el("div.ag-liste");
    D.champTitre = el("input.champ", { placeholder: "Un rendez-vous, un anniversaire…", maxlength: 200 });
    D.champHeure = el("input.champ.ag-heure-champ", { type: "time" });
    D.champNote = el("input.champ", { placeholder: "Une note (facultatif)", maxlength: 400 });
    document.body.append(el("div.page-agenda",
      el("section.ag-mois",
        el("header.ag-barre",
          el("button.fx-outil", { title: "Mois précédent", onclick: () => decaler(-1) }, svg("retour")),
          D.titre,
          el("button.fx-outil", { title: "Mois suivant", onclick: () => decaler(1) }, svg("fleche")),
          el("button.bouton.petit", { onclick: () => { const t = new Date(); S.vu = new Date(t.getFullYear(), t.getMonth(), 1); S.choisi = cle(t); dessiner(); } }, "Aujourd'hui")),
        D.grille),
      el("aside.ag-cote.carte.verre", D.jour, D.liste,
        el("form.ag-form", { onsubmit: ajouter }, D.champTitre, el("div.ag-ligne", D.champHeure, el("button.bouton.principal.petit", { type: "submit" }, "Ajouter")), D.champNote),
        el("p.note.ag-note", "Gardé sur cette machine seulement. Pas encore de rappel : l'agenda ne prévient pas à l'heure."))));
    S.evenements = (await api.appeler("agenda_lire")) || [];
    dessiner();
    pret();
  })();

  document.addEventListener("keydown", (e) => {
    if (e.target.tagName === "INPUT") return;
    if (e.key === "PageUp") decaler(-1);
    if (e.key === "PageDown") decaler(1);
  });
})();
