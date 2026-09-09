-- =============================================================================
-- grenOS 0003 — enregistrement des agents
--
-- La source de vérité reste agents/**/*.md (D-007). Cette table en est le
-- miroir, resynchronisé par le worker au démarrage. Ce seed sert à avoir un
-- dashboard lisible avant même le premier lancement du worker.
-- =============================================================================

insert into agents (
  id, name, status, role_class, reports_to, model_role,
  max_tokens_per_task, max_attempts, can_write, allowed_paths, forbidden_paths
) values
  -- Le Maître d'abord : les autres le référencent.
  ('master', 'Master', 'active', 'orchestrator', null, 'master',
   40000, 2, true,
   array['docs/STATE.md', 'docs/DECISIONS.md'],
   array['agents/**', '.github/workflows/**', 'kernel/**', 'apps/**', 'packages/**', '**/.env*']),

  ('architect', 'Architect', 'active', 'planner', 'master', 'architect',
   80000, 2, true,
   array['docs/**'],
   array['agents/**', '.github/workflows/**', 'kernel/**', 'apps/**', 'packages/**', '**/.env*']),

  ('coder', 'Coder', 'active', 'worker', 'master', 'coder',
   60000, 3, true,
   array['kernel/**', 'packages/**', 'apps/**'],
   array['agents/**', '.github/workflows/**', 'docs/DECISIONS.md', '**/.env*']),

  -- Interdit d'écrire dans kernel/src : un vérificateur qui peut modifier
  -- l'implémentation finit par faire passer le test en changeant le code (D-009).
  ('tester', 'Tester', 'active', 'verifier', 'master', 'tester',
   40000, 2, true,
   array['kernel/tests/**', 'tests/**'],
   array['agents/**', '.github/workflows/**', 'kernel/src/**', '**/.env*']),

  -- Sous-agents : spécifiés, pas encore routés. Restent dormants tant que le
  -- Maître ne sait pas quand les appeler (agents/README.md §10).
  ('kernel', 'Kernel Specialist', 'dormant', 'worker', 'master', 'coder',
   80000, 3, true,
   array['kernel/src/arch/**', 'kernel/src/mm/**', 'kernel/src/interrupts/**', 'kernel/src/task/**'],
   array['agents/**', '.github/workflows/**', '**/.env*']),

  ('filesystem', 'Filesystem Agent', 'dormant', 'worker', 'master', 'coder',
   70000, 3, true,
   array['kernel/src/fs/**', 'kernel/src/block/**'],
   array['agents/**', '.github/workflows/**', '**/.env*']),

  ('drivers', 'Drivers Agent', 'dormant', 'worker', 'master', 'coder',
   70000, 3, true,
   array['kernel/src/drivers/**', 'kernel/src/pci/**'],
   array['agents/**', '.github/workflows/**', '**/.env*']),

  ('security', 'Security Agent', 'dormant', 'verifier', 'master', 'architect',
   60000, 2, true,
   array['docs/security/**'],
   array['agents/**', '.github/workflows/**', 'kernel/**', 'packages/**', 'apps/**', '**/.env*']),

  -- Aucun chemin autorisé : le relecteur ne peut rien écrire, par construction.
  ('review', 'Review Agent', 'dormant', 'verifier', 'master', 'architect',
   50000, 2, false,
   array[]::text[],
   array['**'])

on conflict (id) do update set
  name                = excluded.name,
  status              = excluded.status,
  role_class          = excluded.role_class,
  reports_to          = excluded.reports_to,
  model_role          = excluded.model_role,
  max_tokens_per_task = excluded.max_tokens_per_task,
  max_attempts        = excluded.max_attempts,
  can_write           = excluded.can_write,
  allowed_paths       = excluded.allowed_paths,
  forbidden_paths     = excluded.forbidden_paths;
