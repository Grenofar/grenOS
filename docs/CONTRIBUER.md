# Contribuer à grenOS

## Nom des branches
Travaillez toujours sur une branche nommée `feature/<slug>` où `<slug>` est une courte description en minuscules avec des tirets, par exemple `feature/ajout-ahci`. Ne commitez jamais directement sur `main`.

## Ce que le dépôt refuse
Le dépôt refuse deux catégories de changements :
1. Tous les commits poussés directement sur `main` (seulement les merges via pull request sont acceptés).
2. Toute modification des fichiers sous `.github/workflows/` (les workflows de CI sont protégés).

## Exercice des gardes
Chaque garde (script de vérification) doit être exercé dans les deux sens :
- Dans le sens « refus » : introduire une violation volontaire et confirmer que le garde échoue.
- Dans le sens « acceptation » : corriger la violation et confirmer que le garde passe.
Un garde qui n'a pas été vu refuser n'est pas considéré comme fiable.

## Preuve et blocage
Une preuve (un test réussi, une modification vérifiée) est rapportée jusqu'à ce qu'elle ait obtenu deux passages verts consécutifs dans la CI. Après ces deux succès, elle devient bloquante : tout changement qui la casserait sera rejeté par les gardes automatisés.
