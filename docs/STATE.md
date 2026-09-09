# État vivant du projet

> Maintenu par l'Agent Maître une fois le système en ligne.
> Écrit pour quelqu'un qui revient après un jour d'absence et ne se souvient de
> rien. Actuellement tenu à la main.
>
> Dernière mise à jour : 2026-09-09

## Situation

**La fabrique est complète.** Site, base, routeur, worker et CI existent et se
raccordent. Aucun agent n'a encore tourné : il manque les clés et le passage
des migrations.

Le `kernel/` est volontairement vide — c'est le travail des agents, pas le
mien. Si la fabrique ne sait pas produire un hello-world, mieux vaut le
découvrir tout de suite.

## Mission en cours

Aucune. La première sera : **faire booter un kernel hello-world dans QEMU**,
par la chaîne Maître → Architecte → Codeur → Testeur, sans intervention humaine
sur le code.

La CI attend déjà ce contrat précis :
- `kernel/Cargo.toml` existe
- `cargo build --release` passe
- `cargo clippy -- -D warnings` passe
- une image `.iso` ou `.img` est produite (via `kernel/scripts/make-iso.sh` si présent)
- au boot QEMU, la chaîne `grenOS` apparaît sur le port série
- aucun `panic`, `double fault` ni `triple fault` dans le log
- le tout en moins de 90 secondes

## Fait

- Architecture arrêtée, décisions D-001 à D-015
- Protocole de coordination et 9 agents spécifiés
- **Base** — 5 migrations : schéma, RLS, seed, fonctions atomiques, raccord CI
- **`@grenos/router`** — cascade Gemini 3.8 → 3.7 → 3.6, quotas, cooldowns
- **Worker** — chargeur de prompts, sandbox de chemins, enveloppe JSON, client
  GitHub sans clone, exécuteur, cycle du Maître, boucle principale
- **CI** — `.github/workflows/verify.yml`, build + clippy + boot QEMU, verdict
  écrit directement dans Supabase
- **Site** — auth, dashboard live, missions, runs, agents, coupe-circuit, FR/EN
- **24 tests** verts (`npm test`) : sandbox, chargeur de prompts, enveloppe,
  cascade du routeur

## Non vérifié

- Rien n'a été exécuté contre un vrai Supabase ni un vrai Gemini.
- Le site n'a jamais été compilé (`npm install` non lancé).
- Les 5 migrations n'ont jamais été passées.

Ce sont les trois premières choses qui casseront. C'est normal à ce stade.

## En attente de l'humain

| Clé | Où | Statut |
|---|---|---|
| `SUPABASE_*` (3) | supabase.com → Settings → API | à créer |
| `GEMINI_API_KEY` | aistudio.google.com/apikey | à créer |
| `GITHUB_TOKEN` | PAT fine-grained sur `Grenofar/grenOS` | en cours |

Puis :
1. Passer les 5 migrations dans le SQL Editor, dans l'ordre.
2. Ajouter `SUPABASE_URL` et `SUPABASE_SERVICE_ROLE_KEY` dans
   **Settings → Secrets → Actions** du dépôt, sinon la CI ne pourra pas
   rapporter ses verdicts.
3. `npm install`, puis `npm run dev` et `npm run worker`.

## À construire ensuite

1. Première exécution réelle, correction de ce qui casse
2. Mission n°1
3. Activation des sous-agents une fois que le Maître sait les router

## Bloqué

Rien.

## Budget consommé

Aucun appel LLM effectué à ce jour.
