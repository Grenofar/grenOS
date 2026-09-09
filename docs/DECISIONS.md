# Journal des décisions

Append-only. On ajoute, on ne réécrit pas. Si une décision est annulée, on
ajoute une nouvelle entrée qui la remplace et on marque l'ancienne.

---

### D-001 — Vercel n'exécute aucun agent
**2026-09-09 · actif**

Les fonctions serverless Vercel sont coupées après quelques minutes, ce qui est
incompatible avec du travail de fond de plusieurs heures. Vercel affiche et
commande ; l'exécution vit ailleurs.

---

### D-002 — Séparation cerveau / mains
**2026-09-09 · actif**

Le cerveau (appels LLM, planification) tient dans 150 Mo et ne consomme presque
pas de CPU. Les mains (`cargo build`, QEMU) demandent plusieurs Go. Les héberger
ensemble force à payer pour le pire des deux.

Cerveau → bot-hosting.net gratuit, 24/7. Mains → GitHub Actions.

**Conséquence contraignante** : le worker doit tenir dans 256 Mo / 512 Mo, donc
aucune dépendance lourde. Jamais de framework d'agents.

---

### D-003 — bot-hosting.net gratuit ne peut pas compiler
**2026-09-09 · actif**

Vérifié : 256 Mo RAM, 25 % d'un cœur, 512 Mo disque. La toolchain Rust seule
pèse ~2,5 Go. De plus ces conteneurs Pterodactyl n'ont ni root ni `/dev/kvm`,
donc QEMU y est impossible même en payant.

→ Le free tier héberge le cerveau uniquement. Si 256 Mo craquent, Starter+ à
2,99 €/mois (2 Go) suffit ; jamais plus.

---

### D-004 — Kernel Rust sur Limine
**2026-09-09 · actif**

Retenu contre : fork xv6/rv6 (plus rapide à démarrer mais on hérite d'un design
pédagogique C/RISC-V), et distro Linux (pas de l'OS-dev).

Limine 11.x est stable et gère l'UEFI et le mode long, ce qui évite d'écrire un
bootloader avant d'écrire un kernel.

---

### D-005 — GitHub Actions seul pour build et tests
**2026-09-09 · actif**

Retenu contre l'exécution sur le PC de l'utilisateur. Coût : ~1-2 min de latence
par cycle. Gain : la boucle tourne vraiment 24/7 et force les agents à commiter
proprement, donc tout est traçable.

---

### D-006 — Repo public
**2026-09-09 · actif**

Minutes GitHub Actions illimitées, ce qui rend viable la boucle de correction
automatique (le Testeur relance sans compter).

Contrepartie assumée : discipline absolue sur les secrets. Une clé poussée est
compromise en minutes. Réponse en cas de fuite : **révoquer**, pas nettoyer
l'historique.

---

### D-007 — Les `.md` d'agents sont la configuration
**2026-09-09 · actif**

`agents/**/*.md` n'est pas de la documentation : le worker les charge et en fait
les system prompts. Modifier un `.md` change le comportement, et chaque
changement est un diff relisible dans git.

Corollaire : **aucun agent ne peut écrire dans `agents/`**. Un système qui
réécrit ses propres prompts dérive sans que personne ne s'en aperçoive.

---

### D-008 — Topologie en étoile, pas en maillage
**2026-09-09 · actif**

Seul le Maître route. Aucun agent ne parle à un autre directement.

9 agents en maillage = 36 canaux. Deux agents en désaccord peuvent boucler toute
la nuit et vider le quota gratuit sans surveillance. En étoile, chaque message
est loggé, budgété et interruptible, et il n'y a qu'un endroit à regarder quand
ça déraille.

---

### D-009 — Aucun agent ne valide son propre travail
**2026-09-09 · actif**

Le mode d'échec principal des équipes d'IA codeuses : le modèle est confiant, le
code ne compile pas, et personne ne s'en aperçoit pendant dix commits.

Seule source de vérité : **une CI verte**. Le Testeur n'a pas le droit d'écrire
dans `kernel/src/**`, sinon il finirait par faire passer le test en changeant le
code.

---

### D-010 — Langues
**2026-09-09 · actif**

`agents/**/*.md` en anglais (ce sont des prompts LLM, l'anglais améliore
nettement la qualité des sorties). `CLAUDE.md` et `docs/**` en français. UI
bilingue FR/EN avec un dictionnaire simple, sans lib i18n lourde.

---

### D-011 — MiniMax écarté
**2026-09-09 · ~~remplacé par D-014~~ (la partie Cerebras reste valable)**

Vérifié en septembre 2026 : facturation dès le premier token, pas de vrai tier
gratuit. Cerebras est retenu mais **jamais pour le Codeur** : contexte plafonné
à 8 192 tokens sur le tier gratuit.

---

### D-012 — OpenRouter et Groq écartés, Mistral retenu
**2026-09-09 · actif · redéfinit la cascade de modèles**

OpenRouter free est plafonné à 50 requêtes/jour (1 000 seulement après un achat
de 10 $), Groq à 1 000 req/jour et 200K tokens/jour. Insuffisant pour un agent
codeur qui envoie 40-60K tokens de contexte par tâche.

Mistral « Experiment » donne accès à **tous** les modèles, Codestral et Mistral
Large compris, à 500K TPM et 1 req/s. C'est de très loin le plus gros volume
gratuit disponible. La lenteur (1 req/s) est sans conséquence : les agents
travaillent en arrière-plan, personne n'attend devant un écran.

**Contrepartie acceptée** : le tier gratuit exige la vérification par téléphone
et l'**opt-in à l'entraînement sur les données envoyées**. Le repo étant public,
le code part de toute façon en clair ; aucun secret ne transite par les prompts.
Si ce compromis devient inacceptable, il faut passer au tier payant Mistral.

---

### D-013 — CLAUDE.md reste local
**2026-09-09 · actif**

Ajouté au `.gitignore` sur demande. Il n'est pas publié avec le reste du repo.

Conséquence : `docs/ARCHITECTURE.md`, `docs/DECISIONS.md` et `docs/STATE.md`
doivent rester autosuffisants, car ce sont eux que verra quelqu'un qui clone le
repo. Ne jamais déplacer dans `CLAUDE.md` une information dont un agent ou un
contributeur a besoin.

---

### D-014 — MiniMax gardé comme option payante, désactivée par défaut
**2026-09-09 · actif · annule le rejet de MiniMax dans D-011**

Vérification faite : MiniMax n'a toujours pas de tier gratuit (crédits d'essai à
l'inscription, puis facturation dès le premier token). Mais sur les deux critères
qui comptent ici, il domine la liste :

- **Qualité agentique** : M2.7 annonce 78 % sur SWE-Bench Verified, avec un
  entraînement explicite sur les boucles compile-run-fix et l'édition
  multi-fichiers — exactement le travail du Codeur et du Testeur.
- **Prix** : ~0,21 $/M en entrée et 0,84 $/M en sortie, contexte 204K.
  À ~100 tâches de codeur par jour, l'ordre de grandeur est de 2 $/jour.

Décision : intégré au routeur en **dernier recours uniquement**, activé par la
seule présence de `MINIMAX_API_KEY`, sous un plafond de dépense dur
(`MINIMAX_BUDGET_USD`) appliqué dans le routeur, pas dans un prompt. Sans clé,
la cascade reste intégralement gratuite et le système fonctionne.

API compatible OpenAI (`platform.minimax.io`) : un seul adaptateur suffit, donc
l'ajouter maintenant ne coûte quasiment rien même s'il reste inactif.

---

### D-015 — Un seul provider : Gemini
**2026-09-09 · actif · met D-012 et D-014 en sommeil**

Mistral écarté pour l'instant (l'opt-in à l'entraînement sur les données n'a
pas été retenu), MiniMax pas activé. On démarre sur Gemini seul :
`gemini-3.8-flash`, `gemini-3.7-flash`, `gemini-3.6-flash`, tous les trois
gratuits au niveau API.

**Ce qui rend ça viable** : les quotas Google sont comptés **par modèle**, pas
par compte. Trois modèles = ~4500 requêtes/jour au lieu de 1500. La cascade du
routeur sert donc à deux choses à la fois : basculer en cas de panne, et
répartir la charge pour multiplier le budget.

Le Testeur démarre sur 3.6 : il appelle souvent et raisonne peu, autant lui
faire consommer le quota dont les autres rôles n'ont pas besoin.

Réversible sans douleur : `packages/router/src/models.ts` contient tout le
catalogue et toutes les cascades. Aucun prompt d'agent ne nomme un modèle — ils
déclarent un `model_role`, qui est une clé dans `CASCADES`. Ajouter Mistral ou
MiniMax plus tard = un adaptateur + des lignes dans ce fichier.
