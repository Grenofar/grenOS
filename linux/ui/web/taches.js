// Gestionnaire de tâches : deux jauges, la liste triée par appétit, et un
// bouton pour arrêter ce qui ne répond plus (ou la touche Suppr).
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, toast } = window.grenos;
  const D = {};
  let filtre = "";
  let tri = { cle: "cpu", sens: -1 };
  let choisi = null;
  let derniere = null;
  const nombre = (v, d = 1) => v.toFixed(d).replace(".", ",");

  function jauge(nom) {
    const j = { valeur: el("strong", "—"), barre: el("i"), detail: el("small") };
    j.bloc = el("div.tc-jauge", el("div.etiquette", nom), j.valeur, el("div.progression", j.barre), j.detail);
    return j;
  }

  function dessiner() {
    const m = derniere;
    if (!m) return;
    D.cpu.valeur.textContent = `${m.cpu.toFixed(0)} %`;
    D.cpu.barre.style.width = `${m.cpu}%`;
    D.cpu.detail.textContent = `${m.coeurs} cœurs`;
    const part = m.memoire_totale_go ? m.memoire_utilisee_go / m.memoire_totale_go * 100 : 0;
    D.mem.valeur.textContent = `${part.toFixed(0)} %`;
    D.mem.barre.style.width = `${Math.min(part, 100)}%`;
    D.mem.detail.textContent = `${nombre(m.memoire_utilisee_go)} Go sur ${nombre(m.memoire_totale_go)} Go`;
    D.nombre.textContent = `${m.total} processus`;
    const lignes = m.lignes
      .filter((l) => !filtre || l.nom.toLowerCase().includes(filtre))
      .sort((a, b) => {
        const x = a[tri.cle], y = b[tri.cle];
        return (typeof x === "string" ? x.localeCompare(y, "fr") : x - y) * tri.sens;
      });
    D.corps.replaceChildren(...lignes.map((l) => {
      const tr = el(`tr${l.pid === choisi ? ".choisi" : ""}`,
        el("td", l.nom), el("td.nb", `${nombre(l.cpu)} %`),
        el("td.nb", l.memoire_mo ? `${l.memoire_mo.toFixed(0)} Mo` : "—"), el("td.nb", String(l.pid)));
      tr.addEventListener("click", () => { choisi = l.pid; dessiner(); });
      tr.addEventListener("dblclick", () => { choisi = l.pid; arreter(); });
      return tr;
    }));
    D.fin.disabled = choisi == null || !m.lignes.some((l) => l.pid === choisi);
  }

  async function mesurer() {
    try { derniere = await api.appeler("taches_mesure"); } catch (e) { return; }
    dessiner();
  }

  function confirmer(nom) {
    return new Promise((ok) => {
      const fond = el("div.cx-fond");
      const fin = (v) => { fond.remove(); ok(v); };
      const non = el("button.bouton.discret", { type: "button", onclick: () => fin(false) }, "Annuler");
      fond.append(el("div.cx-dialogue.carte",
        el("h2", `Arrêter « ${nom} » ?`),
        el("p.note", "Ce qui n'est pas enregistré dans cette application sera perdu."),
        el("div.cx-boutons", non, el("button.bouton.danger", { onclick: () => fin(true) }, "Arrêter"))));
      fond.addEventListener("keydown", (e) => { if (e.key === "Escape") { e.stopPropagation(); fin(false); } });
      document.body.append(fond);
      non.focus();
    });
  }

  async function arreter() {
    const l = derniere && derniere.lignes.find((x) => x.pid === choisi);
    if (!l) return;
    if (!(await confirmer(l.nom))) return;
    try { await api.appeler("taches_arreter", { pid: l.pid }); } catch (e) { toast(String(e)); }
    setTimeout(mesurer, 1000);
  }

  function entete(texte, cle, nb) {
    const th = el(`th${nb ? ".nb" : ""}`, texte);
    th.addEventListener("click", () => {
      tri = { cle, sens: tri.cle === cle ? -tri.sens : (cle === "nom" ? 1 : -1) };
      for (const t of document.querySelectorAll(".tc-table th")) t.classList.remove("trie");
      th.classList.add("trie");
      dessiner();
    });
    return th;
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.cpu = jauge("Processeur");
    D.mem = jauge("Mémoire");
    D.corps = el("tbody");
    D.nombre = el("span.note");
    D.fin = el("button.bouton.danger", { disabled: true, onclick: arreter }, "Fin de tâche");
    const recherche = el("input.champ.tc-recherche", { type: "search", placeholder: "Chercher une tâche" });
    recherche.addEventListener("input", () => { filtre = recherche.value.trim().toLowerCase(); dessiner(); });
    const cpu = entete("Processeur", "cpu", true);
    cpu.classList.add("trie");
    document.body.append(el("div.page-taches",
      el("header.tc-tete.carte.verre", D.cpu.bloc, D.mem.bloc),
      recherche,
      el("div.tc-defile.carte.verre", el("table.tc-table",
        el("thead", el("tr", entete("Nom", "nom"), cpu, entete("Mémoire", "memoire_mo", true), entete("PID", "pid", true))),
        D.corps)),
      el("footer.tc-pied", D.nombre, D.fin)));
    await mesurer();
    pret();
    setInterval(mesurer, 2000);
    recherche.focus();
  })();

  document.addEventListener("keydown", (e) => {
    if (document.querySelector(".cx-fond")) return;
    if (e.key === "Escape") api.appeler("fermer");
    else if (e.key === "Delete" && choisi != null) arreter();
  });
})();
