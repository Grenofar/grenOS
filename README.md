# grenOS

Un système d'exploitation x86_64 en Rust, écrit par une équipe d'agents IA
supervisée depuis un site web.

Deux objectifs de valeur égale : **l'OS**, et **la fabrique qui le produit**.

## Comment ça marche

```
Humain ──► Vercel ──► Supabase ──► Cerveau (agents) ──► git push
                          ▲                                 │
                          └──── verdict ◄── GitHub Actions ◄─┘
                                            build + QEMU
```

Trois plans séparés, pour une raison précise à chaque fois :

| Plan | Rôle | Pourquoi là |
|---|---|---|
| **Vercel** | UI, missions, dashboard live | affiche et commande, **n'exécute aucun agent** — les fonctions serverless sont coupées après quelques minutes |
| **Supabase** | file de tâches, état, Realtime, RLS | bus et mémoire partagée ; le worker est sans état et peut être tué à tout moment |
| **Cerveau** | la boucle d'agents, 24/7 | tient dans 256 Mo : appels LLM et planification, aucun calcul lourd |
| **GitHub Actions** | `cargo build`, QEMU, tests | la seule chose autorisée à dire « ça marche » |

## L'équipe

```
              MASTER          ← seul à router le travail
    ┌───────────┼───────────┐
ARCHITECT     CODER      TESTER
              └─ sub: kernel, filesystem, drivers, security, review
```

Les prompts vivent dans [`agents/`](agents/) et **sont la configuration**, pas
de la documentation : le worker les charge et en fait les system prompts.
Changer le comportement d'un agent, c'est un diff relisible dans git.

Commence par [`agents/README.md`](agents/README.md) : c'est le protocole de
coordination, et il explique les quatre garde-fous qui empêchent neuf IA de se
marcher dessus.

## Les règles qui tiennent le système

1. **Étoile, pas maillage.** Aucun agent ne parle à un autre. Neuf agents en
   maillage, c'est 36 canaux et deux IA qui peuvent boucler toute la nuit.
2. **Personne ne valide son propre travail.** Le Testeur ne peut pas écrire
   dans `kernel/src/`. Seule vérité : une CI verte.
3. **Un seul écrivain par fichier.** Verrous en base, pas en bonne volonté.
4. **Aucun agent ne peut modifier `agents/`.** Un système qui réécrit ses
   propres prompts dérive sans que personne ne le voie.

Ces règles sont appliquées **en code** — sandbox de chemins, contraintes SQL,
RLS. Un prompt est une demande ; le runtime est la loi.

## Démarrer

```bash
cp .env.example .env.local     # puis remplir — voir les liens dans le fichier
npm install
```

Dans Supabase → SQL Editor, exécuter dans l'ordre :

```
packages/db/migrations/0001_schema.sql
packages/db/migrations/0002_rls.sql
packages/db/migrations/0003_seed_agents.sql
```

Puis :

```bash
npm run dev       # le site sur http://localhost:3000
npm run worker    # la boucle d'agents
```

## Structure

```
agents/          prompts = configuration des agents
docs/            architecture, journal des décisions, état vivant
packages/db/     migrations SQL (schéma, RLS, seed)
packages/router/ routeur multi-modèles, quotas, cascade de repli
apps/web/        le site Next.js (Vercel)
worker/          le daemon cerveau
kernel/          l'OS lui-même
```

## Documentation

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — le détail technique
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — pourquoi chaque choix, et ce qui a
  été écarté
- [`docs/STATE.md`](docs/STATE.md) — où en est le projet maintenant
