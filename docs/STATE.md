# État du projet grenOS

Date : 2026-09-25 00:57 UTC

## Mission en cours
- **Titre** : grenOS Linux: the look, the store and the updates
- **Statut** : En cours (running)
- **Budget tokens utilisé** : 1 430 877 / 20 000 000

## Ce qui est fait
- Distribution Linux basée sur Debian (Debian live image sous `linux/`).
- Schéma de couleurs grenOS et configuration Calamares intégrés.
- Environnement de bureau grenOS autonome (sans KDE/Plasma).
- Catalogue GrenPlace (`linux/data/catalogue.json`) consolidé à 34 applications vérifiées.
- Garde-fous CI (`publier-catalogue.py`) actifs pour vérifier l'intégrité des slugs, l'existence des paquets Debian trixie et la disponibilité des logos.

## En cours / Prochaine étape
- Tâche 72169646 a échoué en CI lors de la validation du catalogue.
- Nouvelle tâche Coder démarrant de `main` pour ajouter au moins 10 applications graphiques au catalogue, portant le total à au moins 44 entrées tout en préservant les 34 slugs existants.

## Blocages / Risques
- Veiller à ce que chaque application ajoutée soit une application à fenêtre (pas d'outil terminal), avec un paquet existant dans Debian trixie et une URL de logo accessible (HTTP 200).
