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

---

### D-016 — Le quota gratuit réel est ~20 requêtes/jour, pas 1500
**2026-09-09 · actif · corrige un chiffre faux de D-015**

Mesuré contre l'API, pas lu sur un blog. Message de Google :

```
Quota exceeded for metric: generate_content_free_tier_requests, limit: 20
```

Les chiffres publiés (1500 req/jour) sont faux d'un facteur 75 pour les modèles
3.x. Le vrai budget est **~20 requêtes/jour et par modèle**, soit **~60/jour**
sur les trois modèles de la cascade.

**Ce que ça change concrètement** : une tâche de codeur coûte 1 à 3 requêtes.
On est donc à une vingtaine de tâches par jour, tous agents confondus. C'est
assez pour faire tourner la fabrique et prouver la mission n°1. Ce n'est pas
assez pour construire un OS.

**Trois conséquences appliquées immédiatement :**

1. **Le routeur ne devine plus aucun quota.** `dailyRequests` est devenu un
   simple repère d'affichage ; la seule autorité est le 429 de Google. Un
   429 « journalier » met le modèle au repos jusqu'au lendemain, un 429
   « par minute » ne fait qu'un cooldown. Un appel gaspillé par modèle et par
   jour, contre le risque d'avoir tort avec assurance.

2. **Le Maître ne réfléchit plus que si l'état a changé.** Il était appelé à
   chaque réveil de la boucle, soit plusieurs fois par minute : à 20 requêtes
   par jour, une mission au repos aurait vidé le quota en cinq minutes. Une
   signature de l'état (tâches, statuts, tentatives, messages non lus, verdicts)
   est comparée avant chaque cycle, et une situation inchangée ne coûte rien.

3. **Les tokens de raisonnement sont comptés.** Les modèles 3.x réfléchissent
   avant de répondre et ces tokens sortent du même budget. Les ignorer faisait
   paraître chaque mission bien moins chère qu'elle ne l'est, et le Maître
   aurait avorté beaucoup trop tard.

**À décider ensuite** (aucune option n'est urgente, la mission n°1 passe
sans) :
- élargir la cascade aux autres modèles gratuits (`gemini-3.5-flash`,
  `gemini-3.1-flash-lite`, `gemini-3-flash-preview`…), chacun ayant son propre
  quota de 20 — la cascade est déjà conçue pour ça, c'est quelques lignes dans
  `models.ts` ;
- activer la facturation Gemini : à ce volume le coût réel est de quelques
  euros par mois ;
- revenir sur MiniMax (D-014), toujours le meilleur rapport qualité/prix en
  agentique.

---

### D-017 — NVIDIA NIM en tête de cascade, Gemini en plancher
**2026-09-10 · actif · lève la contrainte de D-016**

Une seule clé de compte (`nvapi-…`, build.nvidia.com) ouvre 80 modèles, tous
exposés au format OpenAI. Vérifié : la clé est bien globale, pas par modèle.

| | Gemini | NVIDIA NIM |
|---|---|---|
| Débit | ~20 requêtes / **jour** / modèle | 40 requêtes / **minute** |
| Épuisement | quotidien, se recharge | crédits finis |
| Modèles | 3 Flash | DeepSeek V4 Pro, Kimi K3, Nemotron 3 Super… |

Les deux sont complémentaires, pas concurrents : **NVIDIA a le volume, Gemini a
la permanence.** NVIDIA mène chaque cascade, Gemini est le plancher sur lequel
le système retombe quand les crédits s'épuisent — et il ne s'épuise jamais
définitivement.

**Modèles retenus, tous testés contre l'API :**

| Modèle | Latence mesurée | Rôle |
|---|---|---|
| DeepSeek V4 Pro | 4,9 s | Codeur, Architecte — le plus fort |
| Nemotron 3 Super | 2,5 s | Maître, Testeur — tournent souvent |
| Kimi K3 | 14,4 s | Architecte en repli — raisonne longtemps, tourne rarement |

`deepseek-v4-flash-0731` est **écarté** : HTTP 504. Une cascade bâtie sur un
modèle qui ne répond pas est pire qu'une cascade plus courte.

**Ce que l'intégration a appris :**

- **Pas de `response_format: json_object`.** Le support varie sur 80 modèles et
  un modèle qui le refuse fait échouer tout l'appel. Le parseur d'enveloppe
  récupère déjà le JSON d'un bloc de code ou d'une phrase d'introduction : la
  robustesse tient à un seul endroit plutôt qu'à un tableau de compatibilité
  par modèle, qui pourrirait.
- **Un modèle qui raisonne sans répondre est une troncature déguisée.** Certains
  renvoient un `content` vide avec la réflexion dans `reasoning_content`. Traité
  comme un manque de place, donc relancé avec un budget triplé sur le *même*
  modèle — changer de modèle heurterait le même mur.
- **L'ordre des tests de 429 compte.** « Quota exceeded … requests_per_minute »
  contient le mot *quota* : tester les crédits en premier classait chaque limite
  transitoire comme définitive et mettait au repos jusqu'au lendemain un modèle
  parfaitement fonctionnel.

**Validation de bout en bout** : prompt réel du Codeur (14 016 caractères chargés
depuis `agents/02-coder.md`) → DeepSeek V4 Pro → enveloppe JSON valide → sandbox
de chemins → code Rust `no_std` portant un commentaire `// SAFETY:` sur son bloc
`unsafe`, exactement comme le protocole l'exige. Les prompts pilotent
réellement le comportement.

---

### D-018 — Les 9 agents sont actifs, et la priorité des spécialistes est appliquée par le sandbox
**2026-09-10 · actif · lève la mise en sommeil décidée en D-007**

Les cinq sous-agents passent de `dormant` à `active` : kernel, filesystem,
drivers, security, review.

**Le problème que ça posait** : le Codeur détient `kernel/**`, et le spécialiste
kernel détient `kernel/src/arch/**`. Les périmètres se chevauchent. Une règle
écrite seulement dans un prompt aurait laissé le choix à l'appréciation du
Maître — c'est-à-dire au hasard.

**Ce qui a été fait** :

1. Le Maître a une table de propriété explicite (`agents/00-master.md`), et la
   règle est nette : **un spécialiste gagne toujours à l'intérieur de ses
   chemins**. Handover de boot, pagination, tables d'interruption, formats
   sur disque et MMIO sont des domaines où une supposition plausible produit
   un reset silencieux plutôt qu'une erreur.
2. Les chemins des spécialistes sont ajoutés aux `forbidden_paths` du Codeur,
   dans le `.md` **et** dans le seed SQL. La priorité est donc appliquée par le
   runtime avant toute écriture, pas demandée poliment dans un prompt. C'est la
   même logique que partout ailleurs : un prompt est une demande, le sandbox
   est la loi.
3. Review et Security sont des vérificateurs, jamais des rédacteurs. Review
   passe **avant** le Testeur sur les gros diffs ; Security tourne **en
   parallèle** et uniquement sur ce qui touche `unsafe`, les transitions de
   privilège, l'entrée syscall, le parsing d'entrées non fiables ou la RLS.

**Le risque assumé** : neuf agents coûtent plus de requêtes que quatre, et les
crédits NVIDIA sont finis. Le prompt du Maître le dit explicitement — ne pas
tout faire passer par Review et Security. Neuf agents qui commentent chaque
diff, c'est une équipe qui n'avance plus, et chaque avis se paie.
