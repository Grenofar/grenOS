# Développement de grenOS Linux

grenOS Linux est une distribution basée sur Debian, construite avec `live-build` dans un conteneur Docker. Aucun composant n'est compilé directement sur la machine de développement par défaut.

## Construction de l'image

Le script `linux/build.sh` exécute `live-build` dans un conteneur isolé. Il produit une image ISO démarrable contenant le bureau grenOS.

Les étapes de personnalisation sont définies par les *hooks* situés dans `linux/config/hooks/normal/` : ils sont exécutés aux différentes phases de `live-build` pour appliquer les réglages spécifiques (thème, services, configuration du bureau).

La liste des paquets installés dans le système live se trouve dans `linux/config/package-lists/grenos.list.chroot`. Chaque ligne correspond à un paquet Debian disponible dans le dépôt trixie.

Les sources du bureau grenOS sont copiées dans l'image sous `linux/config/includes.chroot/usr/` : elles comprennent le binaire `grenos-desktop`, les fichiers de configuration, les traductions et les ressources statiques.

## Travail sur le dépôt

Avant toute modification, le développeur doit s'assurer que les gardes du dépôt sont respectés :

- `scripts/verif-flux.py` : refuse toute modification qui altère l'historique attendu (rebases, squashs non autorisés).
- `linux/tools/verif-accents.py` : refuse les chaînes de caractères françaises dépourvues de leurs accents (ex. `ete` au lieu de `été`).
- `linux/tools/verif-shell.py` : refuse les erreurs de syntaxe dans les scripts shell (`set -euo pipefail` attendu).
- `linux/tools/verif-recherches.py` : refuse la présence de chaînes anglaises dans les interfaces francophones.
- `linux/tools/verif-recommandes.py` : refuse l'installation de paquets non recommandés ou absents des dépôts Debian trixie.

Tous ces gardes s'exécutent en continu dans le workflow de vérification CI. Aucune commande ou chemin n'est inventé dans ce document : chaque chemin cité existe réellement dans le dépôt.
