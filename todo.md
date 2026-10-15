# todo — tout ce que Grenofar a demandé

> Tenu par Claude. Réécrit le 2026-10-14 à partir de ses deux listes (la liste
> « règle : » et le bloc du 8 octobre au soir), **sans rien retirer**. Chaque
> ligne est numérotée pour qu'on puisse en parler sans la recopier.
>
> Sa consigne : « pour la prochaine maj je veux qu'il y ait tout ce que j'ai
> mis dans todo.md ».

## La seule phrase honnête qu'il faut lire avant la liste

**Il y a 118 demandes ci-dessous. Une image en porte entre cinq et quinze.**
Ce n'est pas un refus : c'est la raison pour laquelle ce fichier existe sous
cette forme. Chaque image publiée dira, en toutes lettres, lesquelles de ces
118 lignes elle porte et lesquelles elle ne porte pas — plutôt que d'annoncer
« c'est fait » sur un travail que personne n'a vu tourner. C'est exactement
l'erreur qui a coûté une image inutilisable le 18 septembre, et une annonce
fausse deux fois le 1er octobre.

Ce qui change à partir d'aujourd'hui : **plus rien n'est coché sans preuve**,
et l'ordre de travail (en fin de fichier) est public, donc discutable.

### Les quatre états, jamais mélangés

| État | Ce que ça veut dire |
|---|---|
| **[prouvé]** | la machine d'essai l'a vu, ou c'est lu à la source |
| **[écrit]** | le code existe, personne ne l'a jamais exercé |
| **[à faire]** | rien n'existe |
| **[sa main]** | ne peut pas être prouvé sans lui : son écran, sa carte, son compte |

---

## A. Le matériel et les pilotes — « c'est le plus gros problème de grenOS »

| # | Demande | État |
|---|---|---|
| A1 | Compatibilité **NVMe** : voir et utiliser un disque NVMe | [à faire] — rien ne le vérifie, la machine d'essai n'a que du SATA |
| A2 | Pilotes **USB au-delà du clavier et de la souris** : manette, webcam, casque | [à faire] |
| A3 | Plus de **matériel Wi-Fi** reconnu | **la cause première est trouvée, et ce n'étaient pas les micrologiciels** : `wpasupplicant` etait absent, donc aucune carte ne pouvait rejoindre un réseau WPA. Corrigé le 14 octobre, avec un garde. Les micrologiciels par carte restent [à faire] |
| A4 | **Son fonctionnel à 100 %**, en VirtualBox comme sur l'ISO | [sa main] — la CI entend un son propre, sa machine reste muette ; la page Son nomme maintenant la sortie |
| A5 | **UEFI/GPT** vérifié et durci pour les machines modernes | [prouvé] pour le démarrage UEFI (partition EFI au secteur 540, OVMF démarre) ; GPT sur disque installé : [à faire] |
| A6 | **SMP** : exploiter tous les cœurs | [prouvé] — Linux le fait seul ; ce qui limite, c'est l'hôte |
| A7 | **AHCI/SATA** réellement testé — « il se peut qu'il ne soit juste pas fonctionnel » | [prouvé] partiellement : la machine d'essai démarre sur un disque SATA et garde ses fichiers |
| A8 | Voir **tous les stockages USB / SATA / NVMe** depuis l'explorateur | [prouvé] pour SATA interne (`montage : sans mot de passe — amovible oui, interne oui`) ; USB et NVMe : [à faire] |
| A9 | **FAT32 est privilégié et ne convient pas** à un usage bureau moderne | [à faire] — l'installateur offre ext4/btrfs/xfs, mais rien ne guide le choix |
| A10 | Pilotes **Ethernet** — « très ciblé, une machine peut ne pas avoir de réseau » | **[à faire]**, et une fausse piste à ne pas reprendre. Le 15 octobre j'ai cru trouver « la conséquence la plus visible » : la mise à jour d'avant-bureau *tirait* le réseau, donc une machine sans réseau attendrait une minute devant un écran noir. Le raisonnement était juste, **la prémisse était fausse** — cette unité est `disabled` à un démarrage ordinaire, l'image le dit elle-même (`maj: installation au demarrage presente, etat disabled`), et une unité désactivée n'entre pas dans la transaction. Il n'y avait pas d'attente. Le correctif, lui, a cassé la mise à jour au redémarrage : apt tournait sans adresse, ne joignait pas le dépôt, n'installait rien, et la construction est restée **verte sans rien publier**. Remis en place le 15 octobre. *Les deux seules unités qui tirent vraiment le réseau — `grenos-maj-auto`, `grenos-maj-verif` — n'ont aucun `WantedBy=` : elles partent sur minuterie, pas au démarrage.* |
| A11 | **Gestion des erreurs matérielles** : un périphérique attendu et absent doit donner un repli propre, jamais un échec de démarrage | **la moitié décisive est faite** — Calamares écrivait `defaults` sur chaque ligne du fstab, donc aucun `nofail` : un second disque absent envoyait la machine en mode secours derrière un mot de passe root qui n'existe pas. Vérifié dans le code de calamares 3.3.14 et dans `local-fs.target` de systemd, corrigé le 15 octobre avec un garde. **Et la preuve est posée** : la CI démarre une machine **sans aucun disque et sans carte réseau** et exige qu'elle ouvre son bureau. Le verdict refuse de conclure si l'absence n'a pas eu lieu — un essai qui retire une carte sans vérifier qu'elle est partie dirait vert même si QEMU avait ignoré l'option. **Et il a refusé à son premier passage, pour une raison que je n'avais pas prévue** : la machine ouvrait bien son bureau sans disque ni carte réseau, mais la sonde écrivait `maj: depot joignable et signature acceptee` — `apt-get update` rend **0** quand il ne joint rien. Une mesure fausse, verte et stable, sur la ligne même qu'on lit pour croire qu'une mise à jour arrivera. Corrigé par `--error-on=any`, lu dans `apt-get(8)` de trixie — et la machine dit maintenant vrai des deux côtés : `INJOIGNABLE` sans réseau, `joignable` avec. **Vu vert deux fois, donc exigé** : une image dont le bureau ne s'ouvre pas sans disque ni carte réseau n'est plus publiée |
| A12 | Pilotes **NVIDIA / AMD / Intel** modernes, GPU et processeurs | [prouvé] pour les microcodes et les micrologiciels ; les pilotes propriétaires NVIDIA : [à faire] |

## B. Disques, fichiers, démarrage

| # | Demande | État |
|---|---|---|
| B1 | **Protection contre la corruption du système de fichiers** | [à faire] |
| B2 | **Chaîne d'approvisionnement sûre** : paquets signés, dépôt authentifié, dépendances vérifiées | [prouvé] pour notre dépôt (clé OpenPGP, signature acceptée, vu à chaque démarrage) ; les dépendances vérifiées : [à faire] |
| B3 | **Retour arrière impossible à casser, même sur coupure de courant** | [à faire] — étude en cours |
| B4 | **Gestion de disques** : partitions, formatage, montage, SMART | [à faire] |
| B5 | **Chiffrement du disque**, idéalement complet, avec gestion correcte des clés | [à faire] — et il a écrit lui-même « faut pas que ça se retourne contre nous » |
| B6 | **Chiffrement de fichiers** | [à faire] |

## C. Sûreté : instantanés, retour arrière, grenRecovery, sauvegardes

> C'est le bloc qu'il a le plus détaillé, et celui dont la panne est la plus
> grave : une machine qui ne revient pas est une machine perdue.

| # | Demande | État |
|---|---|---|
| C1 | **Un instantané avant chaque** installation de pilote, mise à jour de noyau, mise à jour d'OS | **[écrit] pour la moitié qui rend le reste possible** — timeshift 24.06.6-2 est livré, et ses **deux** actions polkit sont ouvertes sans mot de passe. Vérifié dans son `.deb`, pas supposé : un seul fichier, `in.teejeetech.pkexec.timeshift.policy`, qui déclare `…timeshift-gtk` → `/usr/bin/timeshift-gtk` et `…timeshift` → `/usr/bin/timeshift`, **toutes deux en `auth_admin`** — donc mortes sur une machine sans mot de passe. C'est la panne de l'installateur du 25 septembre et celle des disques du 7 octobre, une troisième fois. La session interroge les deux **plus un témoin** (`modify-device-system`, que notre règle n'ouvre pas) : sans lui, un polkit qui dirait oui à tout rendrait la même réponse et la mesure serait morte. **Et l'instantané automatique est écrit** : `grenos-instantane` est appelé par la mise à jour d'avant-bureau, juste avant que dpkg ne touche au système — le seul moment où il vaut quelque chose. Il **rend toujours 0 et est borné à dix minutes** : il tourne avant lightdm, donc tout ce qu'il ferait échouer serait le démarrage de quelqu'un. 7 cas exercés, tous à 0, dont celui qui compte — timeshift qui n'en finit pas est coupé et le démarrage continue. Un disque d'instantanés absent est le cas ordinaire, pas une panne. **Reste** : couvrir l'installation d'un pilote isolé depuis le magasin, qui ne passe pas par là |
| C2 | **Retour arrière** : en cas de problème, ça redevient comme avant | **[à faire]**, et il faut dire ce que timeshift ne donnera pas : sa restauration sur ext4 se fait par `rsync`, donc elle **n'est pas atomique**. Une coupure de courant en plein milieu laisse un système à moitié restauré. Sa demande — « le rollback doit être impossible à casser, même sur une coupure de courant » — demande autre chose : soit btrfs et ses sous-volumes, soit un basculement de l'ancienne racine vers la nouvelle en un seul geste. Livrer timeshift et annoncer C2 serait une promesse à l'écran sans rien derrière. **Premier pas posé** : `grenos-retour` ne promet plus la même chose partout — il lit le système de fichiers de la racine et dit ce que CETTE machine garantit : `atomique` sur btrfs, `partielle` sur ext4 et xfs (« une coupure au milieu laisse un système à moitié restauré »), `aucune` sur une image live. 5 cas exercés. **Et la décision qui reste à prendre** : `partition.conf` pose `defaultFileSystemType: "ext4"`, btrfs offert à côté — tant que ce défaut ne change pas, la plupart des machines n'auront jamais C2 tel qu'il le demande |
| C3 | **grenRecovery** — réparer le démarrage | [à faire] |
| C4 | grenRecovery — restaurer une sauvegarde | [à faire] |
| C5 | grenRecovery — réinitialiser le noyau | [à faire] |
| C6 | grenRecovery — vérifier le système de fichiers | [à faire] |
| C7 | grenRecovery — restaurer un instantané | [à faire] |
| C8 | grenRecovery — récupérer des fichiers | [à faire] |
| C9 | grenRecovery — réinitialiser un mot de passe | [à faire] |
| C10 | **Sauvegardes automatiques intelligentes** | [à faire] |
| C11 | Sauvegarde **locale, disque externe, réseau** | [à faire] |
| C12 | Sauvegardes **chiffrées** | [à faire] |
| C13 | Sauvegardes **versionnées** | [à faire] |
| C14 | **Restauration fichier par fichier** | [à faire] |
| C15 | **Mode sans échec intelligent** : après un plantage noyau, un pilote fautif ou un échec de démarrage, démarrer seul dans un environnement minimal | **[écrit] pour la moitié qui manque à tout le reste** — *savoir* qu'un démarrage a échoué. Un démarrage raté ne laisse rien derrière lui, donc on prend le problème par l'autre bout : un témoin posé au début de chaque démarrage, effacé par l'ouverture du bureau. Un témoin encore là au démarrage suivant ne peut vouloir dire qu'une chose. Il porte le `boot_id` du noyau, sans quoi on relirait le sien et on compterait un échec à tous les coups — la faute exacte du témoin de persistance du 27 septembre. 8 cas exercés, **tous rendent 0** : un compteur d'échecs qui empêche de démarrer serait la panne qu'il prévient. **Reste** : démarrer vraiment en environnement minimal au bout de N échecs |
| C16 | Le mode sans échec **nomme la cause probable** : « le dernier démarrage a échoué à cause de… » | **[écrit]** — la cause est demandée au journal du démarrage précédent, pas devinée, et dans l'ordre du plus grave au plus explicite : panique noyau, puis pilote qui ne se charge pas (son exemple exact), puis service qui n'a pas démarré. **Et si le journal ne dit rien, il le dit** — « coupure de courant ou arrêt au bouton » — au lieu d'inventer une cause. Chaque interrogation est bornée à dix secondes : nous sommes avant le bureau |
| C17 | Le mode sans échec **propose** : désactiver le pilote, restaurer la version précédente, redémarrer en mode minimal, analyser | [à faire] |
| C18 | **Recovery plus solide** (sa première liste) | [à faire] |

## D. Sécurité

| # | Demande | État |
|---|---|---|
| D1 | **Pare-feu** | [à faire] |
| D2 | **Validation stricte des paquets** réseau | [à faire] |
| D3 | Protection contre le **déni de service** | [à faire] |
| D4 | Protection contre l'**usurpation ARP** | [à faire] |
| D5 | **Clés privées jamais en clair** dans l'OS ni dans les images | [prouvé] — la clé de signature vit dans un seau Supabase privé, jamais dans le dépôt |
| D6 | **Isolation et sécurité du noyau** | [à faire] |
| D7 | **Bac à sable** avec des profils existants (Windows 10/11, Debian, Ubuntu) | [à faire] |
| D8 | **Contrôle de permission par application** | [à faire] |
| D9 | **Gestion de Secure Boot** | [à faire] — nous livrons `grub-efi-amd64-bin` sans `shim-signed` : Secure Boot doit rester désactivé, et la page le dit |
| D10 | **Gestion des clés SSH et GPG** | [à faire] |
| D11 | **Gestionnaire de mots de passe** | [à faire] |
| D12 | **Politique de sécurité** : un chemin clair pour signaler et corriger une vulnérabilité | [à faire] |

## E. Observation : grenHub, grenPerf, diagnostic, journaux

| # | Demande | État |
|---|---|---|
| E1 | **grenHub** : une application centrale qui affiche l'état complet de la machine | [à faire] |
| E2 | **Mode diagnostic automatique** : tester CPU, RAM, stockage, réseau, GPU, USB et produire un rapport | [à faire] |
| E3 | **grenPerf** : analyser les tâches d'arrière-plan inutiles au bon fonctionnement, et la gestion de la RAM | [à faire] — la mesure existe déjà (`perf : 17 service(s) en marche, 125 processus`), la fenêtre non |
| E4 | **grenOS ne doit pas beaucoup consommer** en arrière-plan | [prouvé] et mesuré : 17 services dont 16 sont de Debian ; le seul en trop a été retiré |
| E5 | **Moniteur système** avec température et état du matériel | [à faire] — le gestionnaire de tâches existe, la température non |
| E6 | **Journaux persistants** | [à faire] |
| E7 | **Vidages de plantage** (crash dump) | [à faire] |
| E8 | **Rapport de panique du noyau** | [à faire] |
| E9 | **Gestionnaire CPU / RAM / disque / réseau / processus**, propre | [prouvé] pour les processus (`taches ouvert, 103 processus`) ; le reste : [à faire] |
| E10 | **Tests de charge sur plusieurs jours** : CPU, RAM, disque, réseau | [à faire] — hors de portée de la CI, qui dure 40 minutes |
| E11 | **Fuzzing continu en CI** | [à faire] |
| E12 | **Un test de régression par bogue corrigé** | [prouvé] comme pratique : chaque correctif de la semaine est arrivé avec son essai, exercé dans les deux sens |

## F. Énergie, écrans, périphériques à chaud

| # | Demande | État |
|---|---|---|
| F1 | **Veille et réveil** | [à faire] — un des trois gestes manquants de sa promesse « sans terminal » |
| F2 | **Extinction et redémarrage** | [prouvé] — le menu d'arrêt, et l'écran d'extinction est noir et propre |
| F3 | **Batterie** | [à faire] |
| F4 | **Thermique** | [à faire] |
| F5 | **Branchement à chaud** USB, réseau, stockage, sans plantage | [à faire] |
| F6 | **Multi-écran** | [à faire] |
| F7 | **Résolution** | [prouvé] — page Écran, 13 définitions, 11 fréquences |
| F8 | **DPI** | [à faire] |

## G. Les applications — la grande liste

> Trois familles, et il faut les distinguer : ce que Debian livre déjà (on
> l'ajoute et on l'habille), ce qui existe chez nous (on l'améliore), et ce
> qu'il faut écrire.

### G.1 — Ce qui existe chez nous et qu'il faut pousser plus loin

| # | Demande | État |
|---|---|---|
| G1 | Gestionnaire de fichiers moderne — **onglets** | [à faire] |
| G2 | — **recherche** | [à faire] |
| G3 | — **aperçu** | [à faire] |
| G4 | — **compression / décompression** | [à faire] |
| G5 | — **montage de clé USB** | [à faire] |
| G6 | — **formatage de clé USB** | [à faire] |
| G7 | **Paramètres système centralisés** | [prouvé] — huit pages, ouvertes et photographiées |
| G8 | **Centre de mise à jour avec retour arrière** | [prouvé] pour la mise à jour (y compris hors session, avant le bureau) ; le retour arrière : [à faire] |
| G9 | **Barre des tâches** | [prouvé] |
| G10 | **Gestionnaire de fenêtres propre** | [prouvé] — openbox à notre thème, boutons dessinés, croix vérifiée au pixel |
| G11 | **Bureaux virtuels** | [à faire] |
| G12 | **Dock** | [à faire] — la barre fait office, ce n'est pas la même chose |
| G13 | **Notifications** | [à faire] |
| G14 | **Gestion audio complète** | [prouvé] pour le panneau et le mélangeur |
| G15 | **Volume par application** | [à faire] |
| G16 | **Accélération vidéo GPU** | [prouvé] — sept pilotes VA-API, libavcodec, et Firefox configuré selon la carte |
| G17 | **Navigateur web** | [prouvé] — Firefox ESR |
| G18 | **grenStore** (le nom qu'il donne à GrenPlace) | [prouvé] pour le magasin, 99 applications en ligne ; le nom et la refonte façon Microsoft Store : [à faire] |

### G.2 — Ce que Debian livre et qu'il faut intégrer

| # | Demande | État |
|---|---|---|
| G19 | **Lecteur PDF** | [à faire] |
| G20 | **Visionneuse d'images** | [à faire] |
| G21 | **Lecteur vidéo et audio** façon VLC | [à faire] |
| G22 | **Bluetooth** | [à faire] — la page Connexions existe, le matériel n'est pas prouvé |
| G23 | **Support manette** | [à faire] |
| G24 | **Client SSH** | [à faire] |
| G25 | **Outils de diagnostic réseau** | [à faire] |
| G26 | **VPN** | [à faire] |
| G27 | **Pare-feu avec interface graphique** | [à faire] |
| G28 | **Gestionnaire de téléchargement** | [à faire] |
| G29 | **Gestion de disques** (partitions, formatage, montage, SMART) | [à faire] — voir B4 |

### G.3 — Ce qu'il faut écrire

| # | Demande | État |
|---|---|---|
| G30 | **Terminal avec un shell grenOS** | [à faire] — un terminal existe, il n'est pas à nous |
| G31 | **Presse-papier avec historique** (Windows+V) | [à faire] |
| G32 | **Calendrier** | [à faire] |
| G33 | **Horloge** | [prouvé] dans la barre ; une application horloge : [à faire] |
| G34 | **Minuteur** | [à faire] |
| G35 | **Bloc-notes** | [à faire] |

## H. GrenAI

| # | Demande | État |
|---|---|---|
| H1 | **Lancer des IA locales** téléchargées depuis Hugging Face | [à faire] |
| H2 | Ou **une alternative** si ce n'est pas possible | [prouvé à la source] — **Ollama n'existe ni dans Debian trixie ni sur Flathub** (vérifié le 1er octobre : l'API de Flathub ne rend que des *clients*). L'équivalent livrable est Alpaca (`com.jeffser.Alpaca`, 2,26 Go), qui embarque son moteur |
| H3 | **Clés API** avec le choix entre GPT, Grok et Gemini | [à faire] |
| H4 | **Assistant de terminal** capable de corriger des commandes | [à faire] |
| H5 | **Automatisation de tâches** | [à faire] |
| H6 | **OCR** intégré à la capture d'écran et aux PDF | [à faire] |
| H7 | **Traduction hors ligne** | [à faire] |
| H8 | **Dictée vocale locale** | [à faire] |
| H9 | **Résumé automatique de documents** | [à faire] |
| H10 | **Recherche sémantique dans les fichiers** | [à faire] |
| H11 | **Jamais en arrière-plan** sauf s'il le choisit | [à faire] — et c'est la ligne qui doit être tenue en premier |

## I. L'assistant de première configuration (OOBE)

> Quatorze écrans, dans son ordre exact. `grenos-premier` existe déjà et fait
> aujourd'hui **deux** de ces quatorze écrans (le nom, et le pays/clavier).

| # | Écran | État |
|---|---|---|
| I1 | Au tout début **en anglais automatiquement**, puis « Welcome to grenOS », « configurons votre ordinateur en quelques étapes », un bouton **Start** | [à faire] |
| I2 | **Langue** — toutes les langues du monde, ou anglais et français si c'est trop long —, **région**, **format de date et d'heure**, **en ligne pour avoir l'heure**, bouton Continuer | [écrit] à moitié : le pays et le fuseau existent. **L'heure en ligne était une promesse vide** — l'interrupteur des Réglages appelle `set-ntp`, et aucun démon n'était livré ; corrigé le 14 octobre, avec un garde. La **langue**, elle, demande d'abord que nos fenêtres soient traduisibles : aucune ne l'est, tout le texte est du français en dur |
| I3 | **Clavier** : azerty, qwerty, etc., **avec un champ pour le tester**, bouton Continuer | **[prouvé]** — et je l'avais marqué « à faire » à tort : le champ existe (`grenos-premier:215`), la bascule est immédiate (`clavier.appliquer_maintenant`), 16 dispositions. Démenti en relisant le code, pas en le croyant |
| I4 | **Connexion à Internet** : une liste nette et fluide des réseaux, le mot de passe au clic, un bouton « continuer hors connexion », puis Continuer | [à faire] |
| I5 | **Votre compte** : un nom qui devient `…@grenos`, un mot de passe au login s'il le souhaite, Continuer | [prouvé] pour le nom et le compte ; le mot de passe optionnel ici : [à faire] |
| I6 | **Apparence** : sombre ou clair ou auto, choix de couleurs, **aperçu du bureau à droite**, Continuer | [à faire] |
| I7 | **Usage du PC** — s'il choisit *développement* : un bouton installe VS Code, Git, un terminal amélioré, les outils de compilation et GrenAI | [à faire] |
| I8 | — s'il choisit *jeu* : **Ctrl+Alt+Échap** ouvre un mode jeu qui ne laisse tourner que l'arrière-plan de sécurité, coupe GrenAI, et propose de fermer les autres applications | [à faire] |
| I9 | **Confidentialité** : « vos données vous appartiennent » écrit clairement, choix de télémétrie (aucune / minimale anonyme / complète), **aucune par défaut**, Continuer | [à faire] |
| I10 | **Sécurité** : « protégeons votre ordinateur », l'état (Secure Boot, noyau signé, mises à jour auto…), et activer le chiffrement du disque | [à faire] |
| I11 | **Mises à jour** : automatiques, me demander, ou manuelles | [à faire] |
| I12 | **Applications** : proposer les applications à installer (navigateur et lecteur multimédia déjà là ; grenPerf, GrenAI proposés ici et dans grenStore, pas installés d'office) | [à faire] |
| I13 | **Sauvegardes** : activées ou non, destination (disque externe, NAS, serveur, nuage), **et tester la récupération** | [à faire] |
| I14 | **Optimisation automatique** : « nous avons détecté votre matériel… », la liste (CPU, GPU, RAM, SSD, NVMe, HDD, Wi-Fi, Bluetooth, audio), « grenOS a trouvé des optimisations possibles », bouton Optimiser | [à faire] |
| I15 | **Écran final** : il met l'OS en place, installe les applications choisies, règle Ctrl+Alt+Échap selon les choix, puis **« grenOS est prêt »** apparaît | [à faire] |
| I16 | Après le clavier, la langue et le nom : le choix entre **Rapide**, **Personnalisé (recommandé)** et **Dev** | [à faire] |
| I17 | Le parcours **Dev** ajoute : partitionnement, chiffrement, Secure Boot, swap, ordonnanceur, services, réseau, pare-feu, télémétrie, journaux, mises à jour, bac à sable, paramètres du noyau | [à faire] |

## J. La promesse « sans une ligne de terminal »

> Onze gestes. Il faut pouvoir les faire tous sans terminal — c'est sa phrase,
> et c'est mesurable geste par geste.

| # | Geste | État |
|---|---|---|
| J1 | Installer | [écrit] — l'installateur s'ouvre et est habillé ; aucune installation n'a été menée à son terme |
| J2 | Démarrer | [prouvé] |
| J3 | **Se connecter au Wi-Fi** | [à faire] — mais le maillon manquant est nommé : sans `wpasupplicant`, c'était impossible quelle que soit la carte. Corrigé ; QEMU n'ayant pas de Wi-Fi, seule sa machine pourra le prouver |
| J4 | Installer une application | [prouvé] — Flathub répond, une application s'installe |
| J5 | Mettre à jour | [prouvé] — y compris au redémarrage, avant le bureau |
| J6 | Brancher une clé USB | [à faire] |
| J7 | Naviguer | [prouvé] |
| J8 | Lire une vidéo | [prouvé] pour le décodage ; aucun lecteur local n'est livré |
| J9 | **Mettre en veille** | [à faire] |
| J10 | Redémarrer | [prouvé] |
| J11 | **Récupérer d'une erreur** | [à faire] — c'est tout le bloc C |

## K. La fabrique et la documentation

| # | Demande | État |
|---|---|---|
| K1 | **Mise à jour des applications**, pas seulement du noyau | [à faire] — la page Mises à jour de GrenPlace existe, les applications Flatpak ne sont pas mises à jour |
| K2 | **Que les mises à jour fonctionnent correctement** (il l'a écrit deux fois) | [prouvé] pour le dépôt, le bouton, le service, la fenêtre d'avant-bureau et le retour du bureau |
| K3 | **Constructions reproductibles** : deux constructions indépendantes doivent donner exactement le même noyau | [à faire] |
| K4 | **Documentation développeur** : architecture, pilotes, appels système, contribution, débogage | [à faire] |

---

## L'ordre de travail

> Il n'est pas fait au hasard. Trois règles le décident : ce dont la panne est
> **irréversible** passe devant ; ce qui est **prouvable par la machine** passe
> devant ce qui demande sa main ; et une pièce dont dix autres dépendent passe
> devant les dix.

1. **A11** — un périphérique absent ne doit jamais empêcher le démarrage.
   C'est la seule ligne dont l'échec rend une machine inutilisable, et elle ne
   coûte presque rien : des unités qui n'exigent pas ce qui peut manquer.
2. **C1 + C2** — l'instantané et le retour arrière. Tout le reste de la liste
   consiste à *changer* le système ; sans filet, chaque changement est un
   risque pris sur sa machine.
3. **C15 → C17** — le mode sans échec, qui est le filet du filet.
4. **E1 + E2 + E3** — grenHub, le diagnostic et grenPerf. Ils sont prouvables
   par la machine, ils répondent à trois demandes d'un coup, et ils donnent
   enfin un endroit où lire l'état de **sa** machine à lui, ce qui débloque A1
   à A12 (aujourd'hui je devine son matériel, je ne le lis pas).
5. **A1, A2, A3, A8, A10** — la couverture matérielle, une fois qu'on sait la
   mesurer chez lui.
6. **I1 → I17** — l'OOBE. Gros, visible, et c'est la première chose que voit
   quelqu'un qui installe.
7. **G1 → G6** — l'explorateur : onglets, recherche, aperçu, compression, USB.
8. **D, G.2, G.3, H** — sécurité, applications livrées, applications à écrire,
   GrenAI.

## Ce qui ne peut pas être prouvé sans lui

Inchangé, et il faut le redire à chaque fois plutôt que de le laisser filer :

- **entendre le son sur sa carte** (A4) — ouvrir Réglages → Son ; si elle nomme
  une sortie HDMI alors que ses enceintes sont analogiques, la cause est
  trouvée et le sélecteur est juste au-dessus ;
- **une installation sur disque menée à son terme** (J1) ;
- **Rocket League et Roblox**, qui demandent son compte et une vraie carte 3D ;
- **l'arbitrage de Papirus** : les ~610 ms par lancement ne partiront qu'en
  changeant de thème d'icônes. C'est un choix d'allure, pas une optimisation ;
- **redémarrer la fabrique d'agents** : le verrou ne bat plus depuis le
  2026-10-05 à 18:15 UTC. `agents_paused` est faux — ce n'est pas une pause, le
  processus est mort. Un worker lancé depuis une session d'assistant ne
  survivrait pas à cette session, donc c'est `npm run worker` chez lui.
