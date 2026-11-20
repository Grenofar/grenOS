// GrenAI : une conversation avec le modèle choisi — sur la machine (Ollama)
// ou chez un fournisseur avec SA clé. La page dit toujours où partent les mots.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, toast } = window.grenos;
  const D = {};
  let R = null;
  let messages = [];
  let occupe = false;

  const choisi = () => R.fournisseurs.find((f) => f.cle === R.fournisseur) || R.fournisseurs[0];

  function bandeau() {
    const f = choisi();
    D.bandeau.className = `gi-bandeau ${f.local ? "local" : "nuage"}`;
    D.bandeau.textContent = f.local
      ? `Sur cette machine (${f.modele}) : rien de ce que tu écris ne quitte l'ordinateur.`
      : `${f.nom}, modèle ${f.modele} : ce que tu écris part chez ce fournisseur, avec ta clé.${f.a_une_cle ? "" : " Aucune clé enregistrée — ouvre les réglages."}`;
  }

  function bulle(m) {
    return el(`div.gi-bulle.${m.role === "user" ? "moi" : m.role === "erreur" ? "erreur" : "ia"}`, el("div.gi-texte", m.contenu));
  }

  function dessiner() {
    D.fil.replaceChildren(...(messages.length ? messages.map(bulle) : [el("div.gi-vide",
      el("strong", "Que veux-tu faire ?"),
      el("p.note", "Pose une question, colle un texte à corriger, ou résume un document. GrenAI ne tourne que pendant que cette fenêtre est ouverte."))]));
    if (occupe) D.fil.append(el("div.gi-bulle.ia.attente", el("div.gi-texte", "…")));
    D.fil.scrollTop = D.fil.scrollHeight;
    D.envoyer.disabled = occupe;
  }

  async function envoyer(texte) {
    texte = (texte ?? D.champ.value).trim();
    if (!texte || occupe) return;
    messages.push({ role: "user", contenu: texte });
    D.champ.value = "";
    occupe = true;
    dessiner();
    try {
      const reponse = await api.appeler("grenai_demander", { messages: messages.filter((m) => m.role !== "erreur") });
      messages.push({ role: "assistant", contenu: reponse });
    } catch (e) {
      messages.push({ role: "erreur", contenu: String(e) });
    }
    occupe = false;
    dessiner();
    D.champ.focus();
  }

  async function resumer() {
    try {
      const d = await api.appeler("grenai_lire_document");
      if (!d) return;
      if (!d.texte.trim()) { toast("Ce document ne contient pas de texte lisible (un scan ? passe-le d'abord par l'OCR de l'explorateur)."); return; }
      await envoyer(`Résume ce document en français, en quelques points clairs.${d.coupe ? " (Il a été coupé à 12 000 caractères.)" : ""}\n\n« ${d.nom} » :\n\n${d.texte}`);
    } catch (e) { toast(String(e)); }
  }

  function reglages() {
    const f = choisi();
    const fond = el("div.cx-fond");
    const choix = el("select.champ", R.fournisseurs.map((x) => el("option", { value: x.cle, selected: x.cle === f.cle }, x.nom)));
    const modele = el("input.champ", { value: f.modele, spellcheck: "false" });
    const cle = el("input.champ", { type: "password", autocomplete: "off", placeholder: "Clé d'API (laisser vide pour garder celle enregistrée)" });
    const note = el("p.note");
    const majNote = () => {
      const x = R.fournisseurs.find((y) => y.cle === choix.value);
      modele.value = x.modele;
      cle.hidden = x.local;
      note.textContent = x.local
        ? "Ollama tourne sur ta machine : aucun compte, aucune clé, rien ne sort. GrenPlace propose Alpaca, qui l'installe avec ses modèles."
        : `${x.cle === "anthropic" ? "Ta clé API se crée sur console.anthropic.com. " : ""}${x.a_une_cle ? "Une clé est enregistrée (elle ne s'affiche jamais). " : ""}La clé reste dans un fichier que toi seul peux lire.`;
    };
    choix.addEventListener("change", majNote);
    majNote();
    const fermer = () => { fond.remove(); D.champ.focus(); };
    fond.append(el("form.cx-dialogue.carte", {
      onsubmit: async (e) => {
        e.preventDefault();
        try {
          R = await api.appeler("grenai_regler", { fournisseur: choix.value, modele: modele.value, cle: cle.value ? cle.value : null });
          bandeau(); fermer();
        } catch (x) { note.textContent = String(x); }
      },
    }, el("h2", "Réglages de GrenAI"), choix, modele, cle, note,
      el("div.cx-boutons", el("button.bouton.discret", { type: "button", onclick: fermer }, "Annuler"), el("button.bouton.principal", { type: "submit" }, "Enregistrer"))));
    fond.addEventListener("keydown", (e) => { if (e.key === "Escape") fermer(); });
    document.body.append(fond);
    choix.focus();
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    R = await api.appeler("grenai_reglages");
    // grenOS branché à Claude (MCP, 20 novembre) : un bouton, rien d'autre à
    // faire. Il dit ce qu'il fera, et ce qu'il a fait.
    async function etatMcp() {
      const e = (await api.appeler("grenai_mcp_etat")) || {};
      D.mcpBranche = !!e.branche;
      D.mcp.replaceChildren(svg(e.branche ? "coche" : "eclair"), e.branche ? "grenOS branché à Claude" : "Brancher grenOS à Claude");
      D.mcp.title = e.branche
        ? "Claude (Desktop et Code) peut voir l'écran, utiliser la souris, le clavier et le terminal de cette machine. Clique pour débrancher."
        : "Un clic : Claude Desktop et Claude Code pourront voir l'écran, utiliser la souris, le clavier et le terminal de cette machine (serveur MCP local, sans réseau). Arrêt d'urgence : le pointeur dans le coin en haut à gauche.";
    }
    async function basculerMcp() {
      try { toast(await api.appeler("grenai_mcp", { brancher: !D.mcpBranche })); }
      catch (e) { toast(String(e)); }
      etatMcp();
    }
    D.bandeau = el("div.gi-bandeau");
    D.fil = el("div.gi-fil");
    D.champ = el("textarea.champ.gi-champ", { rows: 3, placeholder: "Écris ta question… (Entrée pour envoyer, Maj+Entrée pour aller à la ligne)" });
    D.champ.addEventListener("keydown", (e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); envoyer(); } });
    D.envoyer = el("button.bouton.principal", { onclick: () => envoyer() }, svg("fleche"), "Envoyer");
    document.body.append(el("div.page-grenai",
      el("header.tete", el("div.logo", svg("etoile")), el("div", el("h1", "GrenAI"), el("small.note", "Ton assistant, avec le modèle que tu choisis")),
        el("div.gi-outils",
          el("button.bouton.petit", { onclick: () => api.appeler("grenai_claude_code").catch((e) => toast(String(e))), title: "Ouvre Claude Code dans une console (l'installe la première fois, par l'installateur officiel d'Anthropic)" }, svg("terminal"), "Claude Code"),
          D.mcp = el("button.bouton.petit", { onclick: basculerMcp }, "…"),
          el("button.bouton.petit", { onclick: resumer }, "Résumer un document"),
          el("button.bouton.petit", { onclick: () => { messages = []; dessiner(); } }, "Nouvelle conversation"),
          el("button.bouton.petit", { onclick: reglages }, svg("reglages"), "Réglages"))),
      D.bandeau, D.fil,
      el("div.gi-saisie", D.champ, D.envoyer)));
    bandeau();
    dessiner();
    etatMcp();
    pret();
    D.champ.focus();
  })();
})();
