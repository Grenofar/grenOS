# État vivant du projet

> Maintenu par l'Agent Maître une fois le système en ligne.
> Écrit pour quelqu'un qui revient après un jour d'absence et ne se souvient de
> rien. Actuellement tenu à la main.
>
> Dernière mise à jour : 2026-09-10

## Situation

**La fabrique est complète et branchée.** Les cinq clés ont été vérifiées
contre les vrais services, les cinq migrations sont passées, le site compile.
Rien n'a encore tourné en conditions réelles : le worker n'a jamais fait un
tour de boucle complet.

`kernel/` est vide, volontairement. L'écrire est la mission n°1 — si la
fabrique ne sait pas produire un hello-world, mieux vaut le découvrir tout de
suite.

## Vérifié contre les vrais services

```
npm run doctor
  supabase  ✓ 11 tables · fonctions SQL (0004) · 9 agents (4 actifs)
            ✓ opérateurs : belgacemmaroua@gmail.com, rayanbelgacem747@gmail.com
  gemini    ✓ clé valide, les 3 modèles du catalogue existent
  github    ✓ dépôt accessible en écriture · lecture des runs Actions
```

## La contrainte qui gouverne tout : le quota

**~20 requêtes/jour et par modèle**, mesuré, pas lu (D-016). Trois modèles
dans la cascade, donc **~60 requêtes/jour au total**. Une tâche de codeur en
coûte 1 à 3.

Concrètement : une vingtaine de tâches par jour, tous agents confondus. Assez
pour prouver la mission n°1, pas pour construire un OS. Les options pour
élargir sont dans D-016, aucune n'est urgente.

## Mission n°1

**Faire booter un kernel hello-world dans QEMU**, par la chaîne Maître →
Architecte → Codeur → Testeur, sans intervention humaine sur le code.

Contrat que la CI applique déjà :
- `kernel/Cargo.toml` existe
- `cargo build --release` passe
- `cargo clippy -- -D warnings` passe
- une image `.iso` ou `.img` est produite
- au boot QEMU, `grenOS` apparaît sur le port série
- aucun `panic`, `double fault` ni `triple fault`
- le tout en moins de 90 secondes

## Fait

- Architecture et décisions D-001 à D-016
- Protocole de coordination, 4 agents actifs + 5 dormants
- **Base** — 5 migrations : schéma, RLS, seed, fonctions atomiques, raccord CI
- **Routeur** — cascade Gemini, quotas appris du serveur, modèles à
  raisonnement, 403 non répété
- **Worker** — chargeur de prompts, sandbox de chemins, enveloppe JSON, client
  GitHub sans clone, exécuteur, cycle du Maître conditionné au changement d'état
- **CI** — build + clippy + boot QEMU, verdict écrit dans Supabase, triggers SQL
  qui font avancer la tâche
- **Site** — connexion Google (repli par lien email), dashboard live, missions,
  détail de mission, runs, agents, coupe-circuit, FR/EN
- **Outils** — `npm run doctor`, `npm run env`, `npm run vercel`
- **34 tests** verts

## En ligne

**https://grenos-dev.vercel.app** — HTTP 200, en-têtes de sécurité appliqués.

Il aura fallu quatre causes distinctes pour y arriver, et aucune n'était
visible depuis le code :
1. `main` ne contenait que `.gitattributes` — Vercel déployait une branche vide
2. Vercel ne détectait pas Next.js : `next` doit figurer dans le `package.json`
   de la racine du déploiement, pas seulement dans `apps/web`
3. `package-lock.json` n'était pas versionné
4. Les clés `"//"` que j'utilisais comme commentaires dans `vercel.json` sont
   refusées par son schéma — c'est ce qui bloquait à la fin

## Reste à faire

1. **Premier lancement réel** : `npm run worker` avec une mission en base.
   Le worker démarre et boucle proprement, mais aucune mission n'a jamais été
   traitée de bout en bout.
2. **NVIDIA NIM** — clé demandée. 40 requêtes/minute contre 20/jour chez
   Gemini : c'est ce qui débloque le volume (D-016).
3. Mission n°1.

## Bloqué

Rien.

## Budget consommé

Une poignée d'appels Gemini pour les tests de bout en bout. Aucune mission
lancée.
