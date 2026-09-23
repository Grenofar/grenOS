# Spec — le bureau grenOS

Écrit par Claude pour les agents, 2026-09-23, à la demande de l'humain : « déc actuel bureau grenOS en français ». Chaque fait ci-dessous provient des spécifications existantes et du code du dépôt, vérifié sur la branche `main` à la date indiquée. Français, car les agents le lisent.

## 1. Session de connexion

Le bureau grenOS démarre après l'écran de connexion LightDM, qui lance le script `grenos-session`. Ce script est le processus de session utilisateur : il démarre le gestionnaire de fenêtres Openbox, puis lance deux programmes en arrière-plan :

- `grenos-shell` : le tableau de bord (panel) en haut de l'écran, contenant le menu des applications, la zone de notification et l'horloge.
- `grenos-bureau` : le gestionnaire de bureau, qui affiche le fond d'écran et gère les icônes du bureau (raccourcis, dossiers, fichiers).

Ces trois processus forment la session utilisateur grenOS : LightDM → grenos-session → (Openbox + grenos-shell + grenos-bureau).

## 2. Rôle des programmes grenos-*

- **grenos-session** : script de démarrage de session. Il définit les variables d'environnement, lance Openbox avec sa configuration, puis exécute `grenos-shell` et `grenos-bureau` en arrière-plan. Il attend leur terminaison pour fermer la session proprement.
- **grenos-shell** : implémenté en TypeScript, il fournit le panel supérieur. Il contient le bouton du menu des applications (qui ouvre un menu style Windows avec les icônes des programmes), la zone de notification (icônes d'état comme le réseau ou le volume) et l'horloge. Il réagit aux clics et aux raccourcis clavier définis dans sa configuration.
- **grenos-bureau** : implémenté en TypeScript, il gère le fond d'écran et les icônes du bureau. Il lit le fond d'écran choisi depuis `~/.config/grenos/ecran`, le dessine sur tout l'écran, puis place les icônes du bureau (fichiers, dossiers, raccourcis) selon leur position sauvegardée dans `~/.config/grenos/bureau.json`. Il répond aux glisser-déposer pour réorganiser les icônes et aux clics pour ouvrir les fichiers ou dossiers associés.

## 3. Configuration utilisateur

Tous les choix de l'utilisateur sont stockés dans le répertoire `~/.config/grenos` :

- `theme` : fichier contenant soit `light` soit `dark`, lu par `grenos-shell` et `grenos-bureau` pour adapter les couleurs du panel, du bureau et des icônes.
- `barre.json` : configuration du panel `grenos-shell` (position, taille, greffons actifs, ordre des éléments).
- `bureau.json` : positions des icônes du bureau, sous la forme `{ "chemin/vers/fichier": { "x": 100, "y": 200 } }`.
- `ecran` : chemin absolu vers le fichier image utilisé comme fond d'écran (ex: `/home/utilisateur/Images/fond.png`).
- `fond` : couleur de secours utilisée lorsque l'image du fond d'écran ne peut pas être chargée, au format hexadécimal (ex: `#05070D`).

Ces fichiers sont créés et mis à jour par les programmes eux-mêmes lorsqu'un utilisateur modifie un réglage via l'interface (clic droit sur le bureau, menu du panel, etc.).

## 4. Premier démarrage

Avant le lancement de LightDM, le script `grenos-premier` s'exécute une seule fois lors de la première connexion après l'installation. Il effectue deux actions :

1. Il renomme le compte utilisateur créé par l'installateur (souvent `live` ou `user`) en `grenofar`, en mettant à jour le nom complet, le répertoire personnel et le groupe associé.
2. Il crée le répertoire `~/.config/grenos` avec des fichiers de configuration par défaut : thème `dark`, panel configuré, bureau vide, fond d'écran par défaut défini dans les ressources du paquet `grenos-desktop`.

Après cela, `grenos-premier` se désactive lui-même pour ne pas s'exécuter aux connexions suivantes.

## 5. Catalogue GrenPlace

Le magasin d'applications grenOS, appelé GrenPlace, obtient son catalogue à partir de deux sources :

- Le fichier `linux/data/catalogue.json` dans le dépôt, qui contient une liste statique d'applications (nom, description, icône, paquet Debian ou dépôt à ajouter).
- Ce fichier est publié vers le dépôt APT de grenOS par le script `linux/tools/publier-catalogue.py`, qui lit `linux/data/catalogue.json`, vérifie la disponibilité des paquets sur les dépôts Debian trixie, et génère le fichier `Packages.gz` utilisé par `apt`.

Lorsqu'un utilisateur ouvre GrenPlace, il interroge le dépôt APT local pour afficher la liste des applications disponibles ou déjà installées, en s'appuyant sur ce catalogue publié.

## 6. Mises à jour du système

Les mises à jour de grenOS sont fournies par un dépôt APT dédié, accessible depuis l'OS :

- Le dépôt contient le paquet `grenos-desktop`, qui regroupe le thème, les fonds d'écran, le fichier d'accueil `bienvenue.html` et les outils comme `grenos-theme` et `grenos-maj`.
- L'outil `grenos-maj`, situé dans `/usr/bin/`, met à jour le système en exécutant `apt update` puis `apt upgrade` dans une fenêtre graphique, en montrant la progression et en demandant confirmation lorsqu'il est nécessaire.
- Le dépôt APT est signé avec une clé OpenPGP dont la partie publique est embarquée dans l'ISO et placée dans `/etc/apt/trusted.gpg.d/grenos.gpg`, permettant à `apt` de vérifier l'authenticité des paquets.
- L'ISO embarque également le fichier `/etc/apt/sources.list.d/grenos.list` qui pointe vers le dépôt APT de grenOS hébergé sur le bucket public Supabase `apt`.

Aucun autre mécanisme de mise à jour n'est décrit dans les spécifications ou le code : les mises à jour se font exclusivement par ce dépôt APT et l'outil `grenos-maj`.

## 7. Portée et limites

Ce document décrit uniquement ce que le code et les spécifications font effectivement. Il ne contient aucune supposition, aucune extrapolation et aucune copie de la demande de l'humain au-delà de ce qui est vérifiable dans le dépôt. Toute affirmation est appuyée par un fichier, une ligne de code ou un comportement observé dans les spécifications.

Toutes les descriptions sont rédigées en français, comme demandé.
