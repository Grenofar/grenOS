# grenos-outil-de-base

Outil de base du système grenOS Linux.

## Description

Ce script est le point d'entrée principal du bureau grenOS. Il initialise l'environnement graphique, lance les services de fond, et fournit l'interface utilisateur comprenant le panneau, le menu des applications, et la gestion des fenêtres. Il est écrit en Python 3 et utilise GTK 3 pour l'affichage.

## Utilisation

L'outil est lancé automatiquement au démarrage de la session utilisateur. Il ne prend pas d'arguments en ligne de commande et s'exécute en arrière-plan jusqu'à la fermeture de la session.

## Fichier source

Le code source de cet outil se trouve à :
`linux/config/includes.chroot/usr/bin/grenos-shell`

## Comportement

Au lancement, l'outil :
- Charge la palette de couleurs définie dans `usr/lib/grenos/grenosui.py`
- Initialise le gestionnaire de fenêtres Openbox
- Affiche le panneau inférieur avec le menu des applications, l'horloge et la zone de notification
- Lance le bureau permettant d'afficher des icônes et d'interagir avec le système de fichiers
- Gère les événements clavier et souris via les liaisons GTK
- Fournit un accès aux paramètres système via l'icône de la roue dentée
- Permet de verrouiller l'écran ou de fermer la session depuis le menu

L'outil ne modifie pas le système en dehors de la session utilisateur et ne nécessite pas de privilèges administratifs pour son fonctionnement normal.

