// « Préparer le matériel » (23 novembre) : ce que grenOS a trouvé, ce qui
// manque, et un seul bouton qui l'installe. Avec le Secure Boot, le code de
// l'écran bleu du redémarrage s'affiche en grand, étapes comprises.
"use strict";
(function () {
  const { api, el, svg, habiller, aurore, pret, toast, installer } = window.grenos;
  const D = {};
  const CLE_CODE = "grenos-materiel-code";
  const lireCode = () => { try { return localStorage.getItem(CLE_CODE) || ""; } catch (e) { return ""; } };
  const noterCode = (c) => { try { c ? localStorage.setItem(CLE_CODE, c) : localStorage.removeItem(CLE_CODE); } catch (e) { /* rien */ } };

  function ligneMateriel(l) {
    return el("div.ligne", el("span.icone", svg(l.icone)),
      el("div", el("div.nom", l.nom), l.detail ? el("div.detail", l.detail) : ""),
      el("span.valeur", l.valeur || "inconnu"));
  }

  // L'écran bleu, expliqué AVANT qu'il n'arrive : il est en anglais et en
  // clavier QWERTY, et il ne laisse que dix secondes pour appuyer sur une touche.
  function etapesMok(code) {
    return el("div.carte.verre.pr-mok",
      el("div.pr-mok-tete", el("span.pastille", svg("cadenas")), el("b", "Au redémarrage : un écran bleu, une seule fois")),
      code ? el("div.pr-code", { "aria-label": `Code : ${code.split("").join(" ")}` }, code) : "",
      el("ol.pr-etapes",
        el("li", "Appuie sur une touche dès que « Perform MOK management » s'affiche (tu as 10 secondes)."),
        el("li", "Choisis ", el("b", "Enroll MOK"), ", puis ", el("b", "Continue"), ", puis ", el("b", "Yes"), "."),
        el("li", code ? "Tape le code ci-dessus avec les chiffres de la rangée du haut, SANS Maj (cet écran est en clavier QWERTY)." : "Tape le code qui t'a été donné à l'installation, avec les chiffres de la rangée du haut, SANS Maj."),
        el("li", "Choisis ", el("b", "Reboot"), ". grenOS démarre avec le pilote NVIDIA : tes deux écrans, à leur vraie fréquence.")),
      el("p.note", "C'est la règle du Secure Boot pour un pilote qui ne vient pas de Debian : on l'autorise une fois, il est reconnu ensuite. Si l'écran bleu passe trop vite, rien ne casse : grenOS démarre avec le pilote libre, et la page te le redira."));
  }

  function carteAction(e) {
    const code = lireCode();
    if (e.sur_cle) {
      return el("div.carte.verre.pr-etat", el("h2", "D'abord, installer grenOS"),
        el("p.sous", "Sur la clé USB, un pilote disparaîtrait au redémarrage. Installe grenOS sur ton disque : ce bouton t'attendra sur le bureau du grenOS installé."));
    }
    if (!e.nvidia) {
      return el("div.carte.verre.pr-etat.pr-bien", el("h2", "Ton matériel est prêt"),
        el("p.sous", "Ta carte graphique fonctionne avec les pilotes libres, déjà dans grenOS : rien à installer, et ils se mettent à jour avec le système."),
        el("div.pr-boutons", el("button.bouton.principal", { onclick: finir }, svg("coche"), "Terminer")));
    }
    // 26 novembre : une Kepler (GT 710/730, GTX 600/700) n'a plus aucun
    // pilote de NVIDIA pour nos noyaux. Le libre est ce qui la fait marcher.
    if (e.trop_ancienne && e.pilote !== "nvidia") {
      return el("div.carte.verre.pr-etat.pr-bien", el("h2", "Ton matériel est prêt"),
        el("p.sous", `${e.carte || "Ta carte NVIDIA"} est trop ancienne pour le pilote de NVIDIA : plus aucune de ses versions ne suit les noyaux d'aujourd'hui. C'est le pilote libre, déjà dans grenOS, qui la fait marcher : rien à installer.`),
        el("div.pr-boutons", el("button.bouton.principal", { onclick: finir }, svg("coche"), "Terminer")));
    }
    if (e.pilote === "nvidia") {
      noterCode("");
      return el("div.carte.verre.pr-etat.pr-bien", el("h2", "Le pilote NVIDIA tourne"),
        el("p.sous", "Ta carte est gérée entièrement : tous tes écrans, leur fréquence, la 3D et les jeux exigeants."),
        el("div.pr-boutons", el("button.bouton.principal", { onclick: finir }, svg("coche"), "Terminer")));
    }
    if (e.redemarrer) {
      return el("div.pr-pile",
        el("div.carte.verre.pr-etat", el("h2", "Plus qu'un redémarrage"),
          el("p.sous", "Le pilote NVIDIA est installé. Il se charge au prochain démarrage."),
          el("div.pr-boutons", el("button.bouton.principal", { onclick: redemarrer }, svg("redemarrer"), "Redémarrer maintenant"))),
        e.cle_en_attente ? etapesMok(code) : "");
    }
    return el("div.carte.verre.pr-etat",
      el("h2", "Le pilote de ta carte graphique"),
      el("p.sous", `${e.carte || "Ta carte NVIDIA"} tourne sans son pilote officiel : un seul écran, à 60 Hz, et pas de 3D. Le pilote de NVIDIA la gère entièrement — tous tes écrans, leur vraie fréquence, et les jeux.`),
      el("ol.pr-etapes",
        el("li", "grenOS télécharge le pilote NVIDIA déjà signé, avec le noyau qui va avec — quelques minutes, ne coupe pas la machine."),
        el("li", "Tu redémarres : c'est prêt. Rien à taper, même avec le Secure Boot.")),
      el("div.pr-boutons",
        D.bouton = el("button.bouton.principal.grand", { onclick: preparer }, svg("eclair"), "Préparer le matériel"),
        el("button.bouton.discret", { onclick: plusTard }, "Plus tard")),
      el("p.note", "Il faut Internet. Si quelque chose manque, grenOS remet tout comme avant : rien ne casse."));
  }

  async function preparer() {
    D.bouton.disabled = true;
    // Le paquet que la carte demande (26 novembre) : nvidia-driver-580 pour
    // une Maxwell, Pascal ou Volta (GT 1030…), nvidia-driver-580-open après.
    const paquet = (await api.appeler("preparer_etat")).paquet || "nvidia-driver-580-open";
    let fin = "";
    api.ecouter("installation", (a) => { if (a.fin && a.texte) fin = a.texte; });
    const bien = await installer({ nom: "Pilote NVIDIA", source: "apt", identifiant: paquet, depuis: "les paquets signés d'Ubuntu" });
    const code = (fin.match(/\b(\d{8})\b/) || [])[1] || "";
    if (code) noterCode(code);
    api.appeler("premier_dire", { texte: `preparer : pilote nvidia ${bien ? "installe" : "echec"}${code ? ", code MOK donne" : ""}` }).catch(() => {});
    if (!bien) toast("L'installation n'a pas abouti : le pilote libre reste en place. Le détail était dans la fenêtre d'installation.");
    await dessiner();
  }

  async function redemarrer() {
    try { await api.appeler("preparer_redemarrer"); } catch (e) { toast(String(e)); }
  }

  async function finir() {
    try { await api.appeler("preparer_fini"); } catch (e) { /* le raccourci reste, sans gravité */ }
    api.appeler("fermer");
  }

  async function plusTard() {
    await finir();
  }

  async function dessiner() {
    const e = await api.appeler("preparer_etat");
    D.action.replaceChildren(carteAction(e));
    D.materiel.replaceChildren(...e.materiel.map(ligneMateriel));
  }

  (async function () {
    habiller(await api.appeler("apparence"));
    document.body.append(aurore());
    D.action = el("section.pr-action");
    D.materiel = el("div.lignes");
    document.body.append(el("div.page-preparer",
      el("header.tete", el("div.logo", svg("gpu")),
        el("div", el("h1", "Préparer le matériel"), el("small.note", "Les pilotes de ta machine, installés et activés"))),
      D.action,
      el("section.carte.verre.pr-materiel", el("div.etiquette", "Ce que grenOS a trouvé"), D.materiel)));
    await dessiner();
    pret();
  })();

  document.addEventListener("keydown", (e) => {
    if (document.querySelector(".cx-fond")) return;
    if (e.key === "Escape") api.appeler("fermer");
  });
})();
