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

---

### D-019 — Le système ne ment plus à ses agents
**2026-09-10 · actif**

La mission 1 s'est bloquée sans qu'aucun agent ne se soit trompé. Reconstitution
d'après les événements et les messages en base :

1. Le vrai verdict CI (erreur `Cargo.toml`) renvoie le Codeur en tentative 2.
2. Les trois fournisseurs tombent en même temps (503, timeout). `failTask`
   écrit « No model available for role coder » dans `failure_detail` —
   **par-dessus l'erreur Cargo**.
3. Nemotron répond enfin. Son prompt contient « *Previous attempt failed: No
   model available* ». Il conclut logiquement qu'il ne peut pas travailler et
   rend `failed`, ce qui tuait la tâche sur-le-champ.
4. Le Maître lit « modèle indisponible » et escalade trois fois : mission
   `blocked`.
5. La nouvelle branche est créée depuis `main` avant tout commit : la CI juge
   une branche sans kernel, rend `spec_gap`, et le trigger l'impute à la
   tâche — pendant qu'elle s'exécute. Elle repart en `ready`, un second worker
   la prend : deux exécutions concurrentes.

**Ce qui change, chaque point appliqué en code :**

| Règle | Où |
|---|---|
| Une panne d'infrastructure ne touche jamais `failure_detail` — elle remet la tâche en file, rien d'autre | `failurePatch`, testé |
| Un agent qui rend `failed` consomme une tentative (classe `spec_gap`) au lieu de tuer la tâche | `executor.ts` |
| Une branche naît avec le premier commit de l'agent | `github.ts`, testé |
| La CI ne rend aucun verdict sur une branche identique à `main` | `verify.yml` |
| Un verdict ne s'applique qu'à une tâche en `awaiting_verification` | migration 0009 |
| Une erreur hors modèle (GitHub, base) bloque la tâche et l'annonce, sans boucler | `index.ts` |
| 5xx et timeouts mettent le modèle en pause 90 s ; timeout porté à 240 s | routeur, testé |
| Le dispatcher ne prend pas de tâche si aucun modèle du rôle ne peut répondre | `router.available` |
| Une seule escalade par décision du Maître | `master.ts` |

Le principe : **un agent ne doit lire que ce qu'il a lui-même produit ou ce que
la CI a constaté.** Tout le reste — pannes, quotas, erreurs réseau — relève de
l'infrastructure et reste dans les événements, où seuls les humains le lisent.

---

### D-020 — Un refus de clé écarte le fournisseur, pas la cascade
**2026-09-10 · actif · complète D-017**

La règle « un 401/403 arrête la cascade » date de l'époque où Gemini était le
seul fournisseur : tous les modèles partageaient la même clé, donc le même
refus. Avec deux fournisseurs, elle faisait qu'une clé Gemini refusée
empêchait d'interroger NVIDIA, et l'erreur accusait Gemini même quand c'était
la clé NVIDIA qui était rejetée.

Constaté quand un onglet `.env.local` périmé a été enregistré par-dessus le
fichier à jour : clé Gemini refusée remise en place, clé NVIDIA supprimée. Le
worker retapait le même 403 à chaque tick de 15 secondes.

Désormais un refus met **tout le fournisseur** de côté pendant 10 minutes — plus
un seul appel dans l'intervalle — et la cascade continue sur l'autre. Si rien
ne répond, l'erreur nomme le ou les fournisseurs refusés et le lien pour
régénérer la clé. `router.available()` en tient compte, donc le dispatcher ne
prend aucune tâche qu'aucun fournisseur ne peut traiter.

---

### D-021 — Un seul cerveau, et des agents qui voient ce qu'ils modifient
**2026-09-10 · actif**

Trois défauts découverts en reconstituant l'échec de la mission 1, aucun
visible dans le code relu à froid.

**Six workers tournaient en même temps**, le plus ancien depuis le matin. Sous
Windows, arrêter `npm run worker` tue npm et laisse vivre le `node` enfant :
chaque redémarrage *ajoutait* un worker. Chacun faisait son propre cycle de
Maître, avec sa propre version du code et ses propres clés — tâches en
double, exécutions concurrentes, échecs signés par un code qui n'existait plus.
→ **Verrou en base** (`settings.worker_lock`) : un second worker refuse de
démarrer tant que le premier bat (toutes les 20 s) ; après 90 s de silence il
est présumé mort. En base plutôt qu'en fichier PID, parce que le cerveau doit
pouvoir changer de machine sans que deux machines se croient aux commandes.

**Les agents travaillaient à l'aveugle.** Rien ne remplissait `context_refs` :
le Codeur réécrivait `Cargo.toml` de zéro à chaque tentative sans voir celui
de la tentative précédente, et l'Architecte a répondu à une relecture « je n'ai
aucun moyen de lire le fichier ».
→ Chaque prompt contient désormais **le contenu du dépôt** : fichiers de la
branche de l'agent (ceux qu'il peut modifier d'abord, marqués *writable*), puis
le reste de `kernel/` en lecture seule, puis les documents de conception. Liste
complète, contenus bornés à 60 000 caractères par ordre de priorité — manifeste,
toolchain et point d'entrée en premier.

**Le plan de l'Architecte n'a jamais été vu.** Il est resté sur sa branche de
tâche, jamais fusionnée, alors que la branche du Codeur part de `main`.
→ Un travail **uniquement documentaire** est commité directement sur `main`
(le sandbox l'a déjà confiné à `docs/`). Tout le reste reste sur la branche de
tâche, et rien de ce qui touche au code ne contourne la CI.

**En prime** : la dernière erreur réelle venait de la crate `x86_64 0.14`,
incompatible avec la nightly du jour. La CI respecte désormais
`kernel/rust-toolchain.toml` quand il existe, et le prompt du Codeur demande
d'épingler la toolchain et d'éviter les crates pour de simples entrées/sorties
de port.

### D-022 — Parler au Maître pendant une mission, et son carnet
**2026-09-10 · actif · demandé par l'humain**

Jusqu'ici l'humain ne parlait au Maître qu'au cadrage : une fois la mission
lancée, le chat se verrouillait, et la seule voie pour infléchir le travail
était d'attendre une escalade.
→ La page d'une mission porte un **chat avec le Maître**, sur la même table
que le cadrage (`draft_messages`, l'humain n'insère que `role = 'user'`). Le
cycle du Maître lit ces messages **avant tout le reste**, répond dans le chat,
et reprend une mission qu'il avait escaladée si la réponse de l'humain la
débloque.
→ Ce que l'humain demande est tenu dans **`docs/MASTER.md`**, le carnet du
Maître : consignes en vigueur réécrites par le Maître, puis un journal de
chaque échange **ajouté par le code**, pour qu'aucun échange ne dépende de la
bonne volonté du modèle. Le carnet est relu à chaque décision et montré à tous
les agents. Toute chaîne ayant la forme d'une clé est masquée avant le commit :
le dépôt est public.

**Pourquoi un carnet plutôt que `agents/00-master.md`** : D-007 tient toujours.
Le prompt fixe le format de sortie et la procédure de décision ; une seule
réécriture ratée par le modèle casse toute l'équipe, en silence. Le carnet
porte ce que l'humain dit, pas les règles du jeu.

### D-023 — Le Maître sans quota : l'équipe continue et lui écrit
**2026-09-10 · actif · demandé par l'humain**

Quand la cascade du Maître est épuisée, le travail ne s'arrêtait pas (les
tâches prêtes étaient prises, une CI rouge renvoyait la tâche à son auteur)
mais plus rien ne l'analysait. Pire : le Maître enregistrait sa signature
avant l'appel, et un appel raté laissait la mission attendre un changement
sans rapport pour être jugée à nouveau.
→ Sans modèle disponible, le cycle du Maître est **sauté sans rien
enregistrer**. Le premier tick où le quota revient lit tout ce qui s'est
accumulé : résultats, verdicts, messages de l'humain (30 messages, contenus de
fichiers résumés).
→ Pendant ce temps, pour chaque verdict de CI qu'il n'a pas vu sur du code,
le **Testeur** rend un verdict critère par critère et la **Review** analyse le
code (`worker/src/autopilot.ts`). Tous deux sont en lecture seule et écrivent
au Maître. Déterministe : aucun modèle ne choisit ce qui est relu, puisque
celui qui choisirait est précisément celui qui n'a plus de quota.
→ L'humain qui écrit pendant ce temps reçoit une fois « plus de quota, ton
message est gardé » ; la vraie réponse vient au retour du Maître.

### D-024 — Un seul contrat CI, et un spec_gap qui remonte au Maître
**2026-09-10 · actif · complète D-009**

L'exemple de critère du protocole disait `cargo build --target
x86_64-grenos.json`, la CI lance `cargo build --release` sans cible.
L'Architecte a recopié l'exemple, le Codeur s'est retrouvé coincé entre le plan
et la CI. En parallèle, un agent qui répondait « impossible tel que spécifié »
recevait la même tâche, et la tâche corrigée du Maître était refusée comme
doublon.
→ Ce que la CI exécute est écrit **une seule fois**, `agents/README.md` §6,
injecté dans tous les prompts, et il l'emporte sur tout document de conception.
→ Un `failed` d'agent **gare** la tâche (`blocked`, `spec_gap`, sans consommer
de tentative) ; la décision suivante du Maître la remplace et l'annule.
→ Les tâches annulées ne comptent plus pour clore une mission, et le Maître lit
des tentatives **consommées** : il avait escaladé une tâche « 3/3 » dont la
troisième tentative n'avait pas encore tourné.

### D-025 — Des agents qui vérifient avant d'écrire
**2026-09-10 · actif · demandé par l'humain (« rends-les plus intelligents »)**

Deux causes d'échec dominaient la mission 1, et aucune n'était un manque de
talent du modèle.
1. **Ils inventaient** faute de pouvoir vérifier : une cible JSON sur mesure,
   une crate qui ne compilait plus sur la nightly du jour. La règle « n'invente
   jamais une API » n'avait aucun moyen derrière elle.
2. **Ils découvraient leurs fautes dix minutes trop tard** : `Cargo.tompl`, un
   `loop {}` vide refusé par clippy, un `patch_file` qui ne s'appliquait pas.
   Chacune coûtait un run de CI et une tentative.

→ **`consult`** : un agent peut rendre une enveloppe qui ne contient que des
demandes de lecture. Le worker va chercher les documents (https seulement, six
hôtes de documentation vérifiés depuis la machine, 600 Ko et 14 000 caractères
au plus par document), les lui rend, et l'agent répond après les avoir lus.
Deux tours par tentative. Le texte rapporté est cité comme non fiable : il ne
peut changer ni la tâche ni les chemins, et le sandbox ne lit pas les prompts.
→ **Pré-vol** : avant tout commit, le worker vérifie les fichiers pour les
erreurs **certaines** (noms mal orthographiés, `.cargo/config`, `limine.cfg`,
JSON invalide, cible sur mesure, toolchain sans `x86_64-unknown-none`,
manifeste sans `[package]`, `loop {}` vide, patch inapplicable). S'il en
trouve, rien n'est commité et l'agent reçoit la liste dans la même tentative.
Deux corrections ; ensuite la tentative est perdue, mais pas un run de CI.
Seulement des certitudes : une fausse alerte apprendrait aux agents à ignorer
la liste.

### D-026 — Le travail continue d'une tâche à l'autre, et le vert rejoint main
**2026-09-10 · actif · complète D-009**

Trois constats sur la même tentative ratée de la mission 1.
1. **Rien n'était jamais fusionné.** `main` ne contenait que des documents :
   « jamais fusionné sans CI verte » avait été appliqué, « fusionné quand c'est
   vert » n'existait pas.
2. **Chaque tâche partait de `main`.** La deuxième tâche du Codeur est donc
   partie d'un dépôt sans kernel : elle a perdu la toolchain épinglée et le
   manifeste que la précédente avait enfin fait compiler.
3. **Les versions inventées** : `limine = "0.11"` alors que la dernière est
   0.6.5. Cargo abandonne avant la première ligne de code.

→ Une tâche d'écriture (Codeur, Kernel, Drivers, Filesystem) **continue la
dernière branche d'écriture de la mission**, ou celle que le Maître désigne par
`continue_from` (`"main"` pour repartir de zéro). Le prompt de l'agent le lui
dit explicitement, pour qu'il ne prenne pas ce travail pour le sien et ne
recommence pas tout.
→ Une tâche dont un run de CI est **vert** est fusionnée dans `main` par le
worker, une seule fois ; un conflit est signalé, jamais forcé.
→ Le pré-vol vérifie chaque dépendance de `Cargo.toml` sur crates.io (règles
caret, tilde et exacte de cargo) : une version qu'aucune publication ne
satisfait revient à l'agent avec la plus récente. Ce que le contrôle ne sait
pas juger, ou un crates.io muet, est laissé à la CI.

### D-027 — La suite part de la branche la plus avancée, et un plan n'est pas une source
**2026-09-11 · actif · demandé par l'humain (audit de la mission 1)**

L'audit de la mission 1 (15 runs de CI sur du code kernel, aucun vert) a
trouvé trois défauts que rien ne signalait.
1. **Le plan de l'Architecte inventait, et le Codeur l'a recopié à la
   lettre** : identifiants de requêtes Limine jamais vérifiés,
   `core::arch::x86_64::hlt()`, `outb` et `inb` (qui n'existent pas), un
   `limine.cfg`, une cible JSON. `docs/PLAN.md` est sur `main` et figure dans
   chaque prompt du Codeur ; le contrat CI corrigeait les commandes, rien ne
   corrigeait les API.
2. **« La dernière branche » n'est pas la meilleure.** La tâche suivante
   serait repartie de `agent/4ed99f62`, qui ne compile pas, plutôt que de
   `agent/4b7d4930`, qui compile et ne bute que sur clippy.
3. **Un run annulé rendait quand même un verdict.** L'étape du verdict tourne
   toujours (`always()`), y compris quand `cancel-in-progress` annule le run
   d'un commit remplacé ; le trigger comptait ce faux `failed` comme une
   tentative.

→ Une tâche d'écriture part de la branche dont le **dernier run est allé le
plus loin** (vert, puis clippy ou boot passés, puis compilé), la plus récente
à égalité ; le `continue_from` du Maître passe toujours avant. La CI liste ses
étapes dans `runs.verdicts` (`build`, `clippy`, `boot` : PASS, FAIL ou
UNVERIFIABLE) ; pour les runs plus anciens, les sections du log disent
lesquelles ont tourné.
→ Pré-vol, trois certitudes de plus : les fonctions que `core::arch::x86_64`
n'a pas ; un binaire `#![no_std]` sans `#[panic_handler]` dans toute la crate,
fichiers de la branche compris ; l'édition 2024, du manifeste ou d'une
dépendance (sur la version que cargo choisirait), face à une toolchain
épinglée avant Rust 1.85 — `nightly-2024-11-22` est la dernière 1.84. Une
branche illisible ou un `Cargo.lock` laissent ces cas à la CI.
→ CI : un commit remplacé ne rend aucun verdict ; un run interrompu sans avoir
été remplacé rend `timeout`, pas `compile_error`.
→ Prompts : un document de conception est une affirmation, pas une source ;
les Interfaces de l'Architecte citent leurs sources ; le Codeur a le squelette
du limine-rust-template, relu le 2026-09-11 ; le Maître connaît l'heure et ne
lance pas de code sur un plan en cours de réécriture.

### D-028 — Le Maître parle anglais à l'humain
**2026-09-11 · actif · demandé par l'humain · remplace « tout ce que l'humain
lit est en français » (2026-09-10), complète D-010**

→ Tout ce que l'humain lit du Maître est en **anglais**, quelle que soit la
langue dans laquelle il écrit : réponses du chat de mission et du chat de
cadrage, questions, escalades et leurs options cliquables, messages
automatiques (plus de quota, réponse impossible), carnet `docs/MASTER.md`. La
langue est nommée au modèle à chaque appel (`HUMAN_LANGUAGE`, `master.ts`).
→ Le journal du carnet s'écrit sous `## Exchange log` ; l'ancien titre
`## Journal des échanges` est encore lu, pour ne rien perdre de l'historique.
→ Inchangé : les événements du tableau de bord, écrits par le code, restent en
français, comme `CLAUDE.md` et `docs/**`.

### D-029 — Chaque tâche finit sur un kernel qui boote, et la CI dit tout
**2026-09-11 · actif · demandé par l'humain (« vérifie tout ce qui est généré »)**

La journée du 2026-09-11 a montré quatre défauts, tous invisibles en relisant
le code.
1. **Une tâche qui s'arrête avant le boot ne peut pas être verte** : la CI juge
   build, clippy et boot ensemble. `f58a45fa` (« mise en place du projet ») a
   brûlé ses trois tentatives sans pouvoir réussir.
2. **La sortie de `make-iso.sh` n'arrivait pas aux agents** : `a5780922` avait
   build et clippy verts, son script échouait (« could not find Cargo.toml »,
   lancé depuis la racine du dépôt), et seule la page GitHub le montrait.
3. **Les branches d'agents gardaient la CI de leur ancêtre** : GitHub exécute
   le workflow du commit poussé, et toutes descendaient de `48584b5`
   (2026-09-10).
4. **Un Maître illisible figeait la mission** : deux réponses refusées
   (`status: "escalate"`, puis de la prose), le plateau marqué comme jugé, et
   huit heures sans rien, sans que l'humain le sache.

→ Contrat CI, Maître, Architecte, Codeur : chaque tâche d'écriture se termine
sur un kernel qui boote ; tant qu'il ne boote pas, le boot minimal est une
seule tâche ; un Codeur qui reçoit une tâche impossible à rendre verte répond
`spec_gap`.
→ CI : la sortie de `make-iso.sh` entre dans le log du verdict sous
`--- iso ---`, et l'image d'un boot réussi est gardée 30 jours comme artefact
`grenos-<commit>`.
→ Une branche d'agent naît toujours sur `main` ; quand elle continue une autre
branche, les changements de celle-ci y sont rejoués (API compare), jamais un
fichier de workflow, jamais la suppression d'un chemin que `main` n'a pas.
→ Pré-vol : un `asm!` hors d'un bloc `unsafe`, un `limine.conf` sans entrée
de menu.
→ Choix de branche : un run `test_failure` compte comme « build et clippy
passés ».
→ Maître : une réponse illisible est rejugée au tick suivant ; la troisième
d'affilée bloque la mission et prévient l'humain, en anglais. Son prompt dit
que `status` n'a que quatre valeurs et qu'une escalade est une action.

### D-030 — L'OS se télécharge sans compte
**2026-09-11 · actif · demandé par l'humain**

L'humain veut mettre grenOS sur un PC ou dans VirtualBox sans se connecter.
Les artefacts de la CI ne s'y prêtent pas : GitHub exige un compte pour les
télécharger, et ils expirent au bout de trente jours.
→ Chaque kernel qui arrive sur `main` est reconstruit, démarré dans QEMU avec
le critère de `verify.yml`, puis publié en **Release GitHub**
(`.github/workflows/release.yml`) : public, versionné, sans expiration. Rien
ne se publie s'il ne boote pas.
→ La page **`/download`** du site est publique : ni session ni barre latérale
(`apps/web/components/Frame.tsx`). Elle ne lit que la dernière Release (API
publique de GitHub, côté serveur, revalidée toutes les cinq minutes), jamais
Supabase : aucune donnée du projet n'y passe.
→ Elle dit ce que l'OS fait vraiment. À l'étape 1, il écrit sur le port série
et rien à l'écran ; VirtualBox et les vrais PC ne sont pas vérifiés par la
CI, QEMU l'est.

### D-031 — Un second plancher
**2026-09-11 · actif · complète D-017**

Le 2026-09-11, de 17:12 à 18:20 environ, toutes les cascades sont tombées
ensemble : DeepSeek et Kimi dépassaient leurs 240 s, Nemotron répondait 503,
et gemini-3.8-flash refusait pour « forte demande ». La mission 2 a attendu une
heure sans rien consommer.
→ `gemini-3.7-flash` devient un second plancher pour le Maître, l'Architecte et
le Codeur : chaque modèle Gemini a sa propre capacité et son propre quota
journalier. Nemotron devient le dernier recours de l'Architecte. Aucun des deux
n'est appelé tant que les modèles au-dessus répondent.

### D-032 — Les images sont servies par Supabase, pas par GitHub
**2026-09-11 · actif · demandé par l'humain · remplace la Release GitHub de D-030**

L'humain veut que grenOS s'installe depuis le site, sans passer par GitHub.
→ `release.yml` dépose chaque image qui a booté dans **Supabase Storage**,
bucket public `releases` : `builds/grenos-<AAAAMMJJ-HHMM>-<sha7>.iso`, qui ne
change plus, et `index.json`, les dix dernières images, la plus récente en
tête, en cache une minute. Ce qui sort des dix est effacé : le stockage
gratuit fait 1 Go, l'image 3,6 Mo. Le dépôt utilise la clé service de la CI,
déjà son seul secret.
→ `/download` lit `index.json` côté serveur (revalidé chaque minute) et
télécharge avec `?download=`, qui envoie le fichier en pièce jointe sous son
nom. Le site ne détient toujours aucune clé : le bucket est public en lecture.
→ Plus de Release GitHub. Le code source reste lié, sur GitHub.

### D-033 — Un bureau à l'écran, jugé par la CI, et une machine VirtualBox
**2026-09-11 · actif · demandé par l'humain**

L'humain veut ouvrir grenOS dans VirtualBox avec « un fichier iso et un fichier
vbox », et y voir « un vrai UI type Windows (pas aussi bien pour l'instant mais
ressemblant) », pas une console.
→ Étape 3 de la feuille de route, « Bureau graphique », avant la mémoire
physique : elle ne dépend que du boot. Les étapes suivantes descendent d'un
rang, en base comme dans `docs/ROADMAP.md`.
→ La CI juge l'écran. Le moniteur de QEMU fait une capture à 25 s
(`scripts/ci-screen.py`) ; elle passe avec au moins trois couleurs, aucune sur
plus de 90 % de l'écran. Exigé désormais pour tout run d'agent : étape
`screen` du verdict. Les agents ne voient pas d'image ; le log du verdict la
décrit, carte en lettres comprise.
→ `release.yml` publie avec chaque image une machine VirtualBox,
`builds/grenos-<build>.vbox` (`scripts/make-vbox.py`) : 64 bits, 256 Mo,
l'ISO désignée par son nom (à garder dans le même dossier), COM1 écrit dans
`C:\Users\Public\Documents\grenos-serie.txt`. Et l'aperçu de l'écran, quand le
kernel dessine. `/download` montre les deux boutons et l'aperçu.
→ Pas de VirtualBox sur l'hôte : l'ouverture du `.vbox` est testée par
l'humain, sur son PC principal.
