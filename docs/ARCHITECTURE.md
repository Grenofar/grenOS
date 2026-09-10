# Architecture technique

## 1. Le cycle de vie d'une mission

C'est le cœur du système. Tout le reste n'est que plomberie autour de ce cycle.

```
 Humain
   │  crée une mission depuis l'UI Vercel
   ▼
 Supabase.missions ──── Realtime ────► UI (affichage live)
   │
   │ le cerveau écoute la file
   ▼
 MAÎTRE  ─── décompose ──►  ARCHITECTE ─── plan + critères ──►  MAÎTRE
   │
   │ dispatch (max 3 en vol, sans conflit de chemins)
   ▼
 CODEUR ─── écrit les fichiers ──► commit sur agent/<task-id>
   │
   │ git push
   ▼
 GitHub Actions : cargo build → clippy → QEMU boot → assertions
   │
   │ écrit le verdict DIRECTEMENT dans Supabase (service_role en secret CI)
   ▼
 Supabase.runs ──── Realtime ────► UI + réveille le MAÎTRE
   │
   ▼
 MAÎTRE : vert → accepte et débloque la suite
          rouge → classe l'échec et route (Codeur ou Architecte)
          épuisé → escalade vers l'humain
```

Point clé : **la CI écrit dans Supabase, elle ne renvoie pas au worker.** Le
cerveau n'a pas à sonder GitHub en boucle, ce qui économise de la RAM et des
appels API. Le verdict arrive par Realtime.

## 2. Schéma Supabase (esquisse)

| Table | Rôle |
|---|---|
| `agents` | registre : id, nom, statut, modèle, budget. Miroir des `.md`. |
| `missions` | demande humaine, budget global, statut |
| `tasks` | l'enveloppe de tâche (§3 du protocole agents) |
| `messages` | tout le trafic Maître ↔ agents, append-only |
| `artifacts` | designs, logs, diffs — gros contenus dans Storage |
| `runs` | un run CI : commit, statut, logs, verdict par critère |
| `leases` | verrous de fichiers (un seul écrivain par chemin) |
| `model_usage` | tokens par provider/modèle/jour, pour le routeur |
| `events` | journal append-only, alimente la timeline de l'UI |

**RLS** : allowlist de 2 emails. Le front n'utilise que la clé `anon` ; toute
écriture sensible passe par le worker ou la CI avec `service_role`.

**Realtime** activé sur `tasks`, `runs`, `events` — c'est ce qui rend le
dashboard vivant sans polling.

## 3. Le cerveau (worker)

Contrainte dure : **256 Mo RAM / 512 Mo disque** (bot-hosting.net free).

Conséquences de conception, non négociables :
- Dépendances : `@supabase/supabase-js` + `fetch` natif. Rien d'autre de lourd.
- Pas de framework d'agents. La boucle fait ~200 lignes et se lit en entier.
- Les gros contenus (logs, diffs) vont dans Storage, jamais gardés en mémoire.
- Un seul process, boucle événementielle sur Realtime + timer de secours.
- Redémarrage sans perte : tout l'état est en base, le worker est sans mémoire
  propre. On peut le tuer à tout moment.

## 4. Le routeur de modèles

```
role → [primaire, fallback1, fallback2]
```

À chaque appel :
1. Vérifier le quota du jour en base (`model_usage`).
2. Appeler le primaire. Sur 429 / 5xx / modèle disparu → fallback suivant.
3. Enregistrer les tokens consommés.
4. Si toute la cascade échoue → `provider_error`, le Maître met la tâche en
   attente au lieu de la faire échouer.

Le routeur **doit survivre à la disparition d'un modèle** sans intervention :
les listes gratuites changent chaque semaine. Un modèle inconnu retourné par
l'API n'est pas une erreur fatale, c'est un fallback.

## 5. Les mains (GitHub Actions)

Déclenchement : push sur `agent/**`.

```yaml
build → clippy → qemu-boot (timeout strict) → POST verdict vers Supabase
```

Le repo est public → minutes illimitées. C'est ce qui rend la boucle de
correction automatique économiquement viable : le Testeur peut relancer autant
de fois que nécessaire.

Secrets CI : `SUPABASE_URL`, `SUPABASE_SERVICE_ROLE_KEY`. Rien d'autre.

## 6. Le control plane (Vercel)

Pages :
- **Dashboard** — carte par agent, état live, tâche en cours, tokens du jour
- **Mission** — création, timeline des échanges, arbre des tâches
- **Runs** — historique CI, verdict par critère, logs
- **Diffs** — ce que les agents ont écrit, avant/après
- **Quotas** — consommation par provider, alerte avant épuisement
- **Kill switch** — arrêt immédiat de tous les agents

Le site **n'exécute jamais un agent**. Il lit Supabase et écrit des missions.
Aucune clé de modèle ne l'atteint.

## 6 bis. Déploiement Vercel — pourquoi `vercel.json` est à la racine

Le site vit dans `apps/web`, mais `vercel.json` est à la **racine du dépôt**.

La raison : le réglage *Root Directory* n'existe que dans l'interface Vercel et
ne peut pas être exprimé en JSON. Tant qu'on en dépendait, l'installation
reposait sur un clic introuvable. On construit donc depuis la racine :

```json
"buildCommand":    "npm run build --workspace=@grenos/web"
"outputDirectory": "apps/web/.next"
```

Trois pièges, tous rencontrés pour de vrai :

1. **`next` doit figurer dans le `package.json` de la racine.** Vercel détecte
   le framework à la racine du déploiement ; sans lui il refuse avec
   « No Next.js version detected » avant même de lancer quoi que ce soit.
   `npm workspaces` mutualise la dépendance, elle n'est pas installée deux fois.

2. **`vercel.json` n'accepte aucune propriété hors schéma.** Les clés `"//"`
   utilisées comme commentaires font échouer la validation — d'où cette section
   plutôt que des commentaires dans le fichier.

3. **Un seul `vercel.json` est lu** : celui qui se trouve à la racine du
   déploiement. Si *Root Directory* est un jour réglé sur `apps/web`, c'est
   celui de `apps/web` qui compte et celui de la racine devient inerte. En
   garder deux, c'est se condamner à modifier celui qui est ignoré.

## 7. Sécurité structurelle

Les garde-fous sont dans le code, pas dans les prompts :

| Garde-fou | Où il vit |
|---|---|
| `allowed_paths` / `forbidden_paths` | runtime, avant écriture disque |
| Verrous de fichiers | table `leases`, contrainte SQL |
| Budget tokens | routeur + Maître |
| `agents/**` interdit à tous | runtime |
| Pas de merge sans CI verte | branch protection GitHub |
| Isolation des clés | `service_role` jamais côté navigateur |

Principe : **un prompt est une demande, le sandbox est la loi.** Un modèle peut
toujours désobéir ; le runtime, non.
