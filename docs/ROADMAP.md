# Feuille de route — vers un kernel complet avec drivers

Objectif : **un kernel x86_64 fonctionnel, avec de vrais drivers.**

Ce document existe parce que « fais un kernel complet » n'est pas une mission
exécutable. Une mission doit avoir une fin que la CI peut constater seule,
sinon l'équipe d'agents tourne sans jamais savoir si elle a terminé. Chaque
étape ci-dessous est une mission séparée, avec un critère vérifiable par
machine.

## Le principe : chaque étape est l'instrument de mesure de la suivante

L'ordre n'est pas administratif, il est imposé par la matière.

| Sans | On ne peut pas |
|---|---|
| port série | savoir *pourquoi* le kernel a redémarré |
| gestionnaire d'exceptions | distinguer une page fault d'un triple fault |
| pagination | mapper les registres MMIO d'un périphérique |
| allocateur | construire une liste de périphériques PCI |
| interruptions | attendre qu'un disque réponde sans bloquer le CPU |

Écrire un driver VirtIO avant d'avoir les interruptions, c'est écrire du code
qu'on ne peut ni observer ni déboguer. Sur x86_64, une erreur ne produit pas
un message : elle produit un reset silencieux. Le port série n'est pas une
étape « hello world », c'est le seul instrument de mesure du projet.

## Les missions

| # | Mission | Terminée quand |
|---|---|---|
| **1** | Boot + série | `grenOS` apparaît sur COM1 dans QEMU, sans panic, en moins de 90 s |
| **2** | GDT, IDT, exceptions | une page fault volontaire imprime son adresse et son code d'erreur au lieu de rebooter |
| **3** | Mémoire physique | la carte mémoire Limine est parsée ; l'allocateur de frames alloue et libère, prouvé par des compteurs imprimés |
| **4** | Pagination | une page fraîchement mappée est lisible et inscriptible ; une page démappée provoque une faute *capturée* |
| **5** | Tas | `alloc`/`dealloc` fonctionnent ; `Vec` et `String` utilisables dans le kernel |
| **6** | Timer + IRQ | le PIT ou l'APIC déclenche des interruptions ; un compteur de ticks progresse |
| **7** | PCI | énumération du bus : chaque périphérique QEMU listé sur le port série avec vendor/device id |
| **8** | Driver VirtIO block | lecture du secteur 0 d'un disque préparé, contenu exact vérifié |
| **9** | Clavier PS/2 | une touche pressée dans QEMU produit le bon caractère sur le port série |
| **10** | VFS lecture seule | montage d'une image, listage d'un répertoire connu, lecture d'un fichier connu |

Les missions 7 à 10 sont « les drivers ». Les six premières sont ce qui rend
leur écriture possible.

## Qui fait quoi

Le routage est déjà en place (`agents/00-master.md`) et le sandbox l'applique :

| Missions | Agent propriétaire |
|---|---|
| 2, 3, 4, 6 | **Kernel Specialist** — `arch/`, `mm/`, `interrupts/`, `task/` |
| 7, 8, 9 | **Drivers Agent** — `drivers/`, `pci/` |
| 10 | **Filesystem Agent** — `fs/`, `block/` |
| 1, 5 | **Codeur** — le reste de `kernel/` |

À chaque étape : l'Architecte conçoit, le spécialiste implémente, la CI tranche,
Security audite tout ce qui touche à `unsafe` ou aux privilèges.

## Une mission à la fois

Les missions suivantes ne sont créées qu'une fois la précédente au vert.

Ce n'est pas de la prudence excessive : le Maître fait un cycle de décision par
mission active, et les crédits NVIDIA sont finis. Cinq missions ouvertes en
parallèle, ce sont cinq cycles à chaque changement d'état, pour un travail qui
reste séquentiel de toute façon — l'étape 8 ne peut pas commencer avant que
l'étape 6 existe.

## Ce qui reste hors périmètre pour l'instant

Espace utilisateur, ordonnanceur préemptif, appels système, réseau, écriture
sur disque. Chacun mérite sa propre feuille de route, et aucun n'a de sens
avant les dix étapes ci-dessus.
