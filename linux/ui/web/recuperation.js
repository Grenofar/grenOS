// grenRecovery : depuis la clé grenOS, réparer une installation qui ne
// démarre plus. Chaque geste qui écrit demande confirmation et dit ce qu'il
// va faire ; « Récupérer des fichiers » ouvre le disque en lecture seule.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, toast } = window.grenos;
  const D = {};

  function confirmer(titre, texte, bouton) {
    return new Promise((ok) => {
      const fond = el("div.cx-fond");
      const fin = (v) => { fond.remove(); ok(v); };
      const non = el("button.bouton.discret", { type: "button", onclick: () => fin(false) }, "Annuler");
      fond.append(el("div.cx-dialogue.carte", el("h2", titre), el("p.note", texte),
        el("div.cx-boutons", non, el("button.bouton.principal", { onclick: () => fin(true) }, bouton))));
      fond.addEventListener("keydown", (e) => { if (e.key === "Escape") fin(false); });
      document.body.append(fond);
      non.focus();
    });
  }

  function choisirCompte(comptes) {
    return new Promise((ok) => {
      const fond = el("div.cx-fond");
      const fin = (v) => { fond.remove(); ok(v); };
      const choix = el("select.champ", comptes.map((c) => el("option", { value: c }, c)));
      fond.append(el("form.cx-dialogue.carte", { onsubmit: (e) => { e.preventDefault(); fin(choix.value); } },
        el("h2", "Retirer un mot de passe"),
        el("p.note", "Le compte s'ouvrira sans mot de passe au prochain démarrage. Tu en remettras un dans Réglages › Compte."),
        choix,
        el("div.cx-boutons", el("button.bouton.discret", { type: "button", onclick: () => fin(null) }, "Annuler"), el("button.bouton.principal", { type: "submit" }, "Retirer"))));
      document.body.append(fond);
      choix.focus();
    });
  }

  async function geste(p, quoi, bouton) {
    const nom = p.machine || p.etiquette || p.dev;
    let compte = null;
    if (quoi === "mdp") {
      if (!p.comptes.length) { toast("Aucun compte trouvé dans /home de cette installation."); return; }
      compte = await choisirCompte(p.comptes);
      if (!compte) return;
    } else if (quoi !== "ouvrir") {
      const textes = {
        verifier: ["Vérifier le disque", `On vérifie et on répare le système de fichiers de ${p.dev} (${nom}). La partition doit être fermée. Cela peut prendre quelques minutes.`, "Vérifier"],
        noyau: ["Reconstruire le démarrage du noyau", `On refait l'initrd de tous les noyaux de ${nom}, puis le menu de démarrage. C'est ce qui répare un démarrage cassé par une mise à jour interrompue.`, "Reconstruire"],
        demarrage: ["Réparer le démarrage", `On réinstalle GRUB sur le disque de ${nom}, puis son menu. À faire quand la machine ne trouve plus grenOS du tout.`, "Réparer"],
      }[quoi];
      if (!(await confirmer(...textes))) return;
    }
    bouton.disabled = true;
    D.etat.textContent = "En cours…";
    try {
      const r = await api.appeler("recuperation_faire", { geste: quoi, partition: p.dev, compte });
      if (quoi === "ouvrir") {
        await api.appeler("fichiers_nouvelle_fenetre", { dossier: r });
        D.etat.textContent = `${p.dev} est ouverte en lecture seule dans l'explorateur : copie tes fichiers vers une clé USB. Rien ne peut y être abîmé.`;
      } else {
        D.etat.textContent = r || "Fait.";
      }
    } catch (e) { D.etat.textContent = String(e); }
    bouton.disabled = false;
  }

  function carte(p) {
    const b = (texte, quoi, genre = "") => {
      const x = el(`button.bouton.petit${genre ? "." + genre : ""}`, texte);
      x.addEventListener("click", () => geste(p, quoi, x));
      return x;
    };
    return el("section.carte.verre.rc-carte",
      el("div.rc-tete", el("strong", p.grenos ? `grenOS — ${p.machine || "sans nom"}` : (p.etiquette || "Partition")),
        el("span.note", `${p.dev} · ${p.fs} · ${p.taille}${p.montee ? " · ouverte" : ""}`)),
      p.grenos && p.comptes.length ? el("p.note", `Comptes : ${p.comptes.join(", ")}`) : null,
      el("div.rc-gestes",
        b("Récupérer des fichiers", "ouvrir", "principal"),
        b("Vérifier le disque", "verifier"),
        p.grenos ? b("Retirer un mot de passe", "mdp") : null,
        p.grenos ? b("Reconstruire le noyau", "noyau") : null,
        p.grenos ? b("Réparer le démarrage", "demarrage") : null));
  }

  async function lister() {
    D.liste.replaceChildren(el("p.note", "On cherche les installations sur les disques…"));
    try {
      const l = await api.appeler("recuperation_lister");
      const grenos = l.filter((p) => p.grenos), autres = l.filter((p) => !p.grenos);
      D.liste.replaceChildren(...[
        grenos.length ? null : el("p.note", "Aucune installation grenOS trouvée sur les disques de cette machine."),
        ...grenos.map(carte),
        autres.length ? el("div.etiquette", "Autres partitions") : null,
        ...autres.map(carte)].filter(Boolean));
    } catch (e) { D.liste.replaceChildren(el("p.note", String(e))); }
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.liste = el("div.rc-liste");
    D.etat = el("p.rc-etat");
    document.body.append(el("div.page-recuperation",
      el("header.tete", el("div.logo", svg("bouclier")),
        el("div", el("h1", "grenRecovery"), el("small.note", "Réparer une installation qui ne démarre plus — à lancer depuis la clé grenOS"))),
      el("section.carte.verre.rc-outils",
        el("button.bouton.petit", { onclick: () => api.appeler("recuperation_instantanes").catch((e) => toast(String(e))) }, "Restaurer un instantané (Timeshift)"),
        el("button.bouton.petit", { onclick: () => api.appeler("fichiers_nouvelle_fenetre", { dossier: "/media" }) }, "Ouvrir les sauvegardes (disques externes)"),
        el("button.bouton.petit", { onclick: lister }, svg("maj"), "Relire les disques")),
      D.etat, D.liste));
    pret();
    lister();
  })();
})();
