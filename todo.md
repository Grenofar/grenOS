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
| A1 | Compatibilité **NVMe** : voir et utiliser un disque NVMe | **[prouvé]** (18 nov.) — la machine d'essai a un disque NVMe ; « nvme0n1 4G : le disque NVMe est vu » dans deux passages verts du même commit (CI 44343831083 sur claude/suite, 44354350634 sur main) |
| A2 | Pilotes **USB au-delà du clavier et de la souris** : manette, webcam, casque | **[écrit] en partie** (18 nov.) — manettes : `steam-devices` ouvre leurs règles udev ; webcams (UVC) et casques USB (audio class) sont des pilotes du noyau Debian, sans rien à ajouter. Rien n'est prouvé : la machine d'essai n'a ni l'un ni l'autre |
| A3 | Plus de **matériel Wi-Fi** reconnu | **[écrit]** — **la cause première est trouvée, et ce n'étaient pas les micrologiciels** : `wpasupplicant` etait absent, donc aucune carte ne pouvait rejoindre un réseau WPA. Corrigé le 14 octobre, avec un garde. Les micrologiciels par carte restent [à faire] |
| A4 | **Son fonctionnel à 100 %**, en VirtualBox comme sur l'ISO | [sa main] — la CI entend un son propre, sa machine reste muette ; la page Son nomme maintenant la sortie |
| A5 | **UEFI/GPT** vérifié et durci pour les machines modernes | [prouvé] pour le démarrage UEFI (partition EFI au secteur 540, OVMF démarre) ; GPT sur disque installé : [à faire] |
| A6 | **SMP** : exploiter tous les cœurs | [prouvé] — Linux le fait seul ; ce qui limite, c'est l'hôte |
| A7 | **AHCI/SATA** réellement testé — « il se peut qu'il ne soit juste pas fonctionnel » | [prouvé] partiellement : la machine d'essai démarre sur un disque SATA et garde ses fichiers |
| A8 | Voir **tous les stockages USB / SATA / NVMe** depuis l'explorateur | [prouvé] pour SATA interne (`montage : sans mot de passe — amovible oui, interne oui`) ; USB et NVMe : [à faire] |
| A9 | **FAT32 est privilégié et ne convient pas** à un usage bureau moderne | **[écrit]** (18 nov.) — l'installateur n'offre pas FAT32 (ext4 par défaut, btrfs, xfs). Là où FAT32 arrive vraiment — les clés USB du commerce — l'explorateur montre « FAT32 » à côté du disque dans Ce PC, et dit pourquoi ça gêne (4 Go par fichier au plus, fragile à la coupure) et quoi choisir (exFAT pour partager, ext4 pour grenOS seul, par Disques › Formater). Rien n'est reformaté sans qu'on le demande. Testé (`conseil_fs`) et vu dans l'aperçu ; pas encore sur une vraie clé |
| A10 | Pilotes **Ethernet** — « très ciblé, une machine peut ne pas avoir de réseau » | **[à faire]**, et une fausse piste à ne pas reprendre. Le 15 octobre j'ai cru trouver « la conséquence la plus visible » : la mise à jour d'avant-bureau *tirait* le réseau, donc une machine sans réseau attendrait une minute devant un écran noir. Le raisonnement était juste, **la prémisse était fausse** — cette unité est `disabled` à un démarrage ordinaire, l'image le dit elle-même (`maj: installation au demarrage presente, etat disabled`), et une unité désactivée n'entre pas dans la transaction. Il n'y avait pas d'attente. Le correctif, lui, a cassé la mise à jour au redémarrage : apt tournait sans adresse, ne joignait pas le dépôt, n'installait rien, et la construction est restée **verte sans rien publier**. Remis en place le 15 octobre. *Les deux seules unités qui tirent vraiment le réseau — `grenos-maj-auto`, `grenos-maj-verif` — n'ont aucun `WantedBy=` : elles partent sur minuterie, pas au démarrage.* |
| A11 | **Gestion des erreurs matérielles** : un périphérique attendu et absent doit donner un repli propre, jamais un échec de démarrage | **[prouvé en partie]** — **la moitié décisive est faite** — Calamares écrivait `defaults` sur chaque ligne du fstab, donc aucun `nofail` : un second disque absent envoyait la machine en mode secours derrière un mot de passe root qui n'existe pas. Vérifié dans le code de calamares 3.3.14 et dans `local-fs.target` de systemd, corrigé le 15 octobre avec un garde. **Et la preuve est posée** : la CI démarre une machine **sans aucun disque et sans carte réseau** et exige qu'elle ouvre son bureau. Le verdict refuse de conclure si l'absence n'a pas eu lieu — un essai qui retire une carte sans vérifier qu'elle est partie dirait vert même si QEMU avait ignoré l'option. **Et il a refusé à son premier passage, pour une raison que je n'avais pas prévue** : la machine ouvrait bien son bureau sans disque ni carte réseau, mais la sonde écrivait `maj: depot joignable et signature acceptee` — `apt-get update` rend **0** quand il ne joint rien. Une mesure fausse, verte et stable, sur la ligne même qu'on lit pour croire qu'une mise à jour arrivera. Corrigé par `--error-on=any`, lu dans `apt-get(8)` de trixie — et la machine dit maintenant vrai des deux côtés : `INJOIGNABLE` sans réseau, `joignable` avec. **Vu vert deux fois, donc exigé** : une image dont le bureau ne s'ouvre pas sans disque ni carte réseau n'est plus publiée |
| A12 | Pilotes **NVIDIA / AMD / Intel** modernes, GPU et processeurs | [prouvé] pour les microcodes et les micrologiciels ; les pilotes propriétaires NVIDIA : [à faire] **RTX 50 (Blackwell), 19 nov.** : sur le PC de Grenofar (RTX 5060, i5-14400F sans puce graphique), 800×600 pixelisé puis écran noir. Deux causes : notre GRUB léguait `gfxmode=800x600` au noyau (passé à `auto`), et trixie (noyau 6.12, firmware d'avril 2025) ne connaît pas Blackwell — `nouveau` le gère depuis 6.15 avec le firmware GSP `nvidia/gb20x`, absents de trixie et présents dans trixie-backports. **[écrit]** : noyau 7.2 et firmwares de backports épinglés (`config/archives/grenos-noyau.pref.*`), la construction refuse un noyau < 6.15 ou l'absence de `nvidia/gb206`. Pas encore vu sur sa machine. Les machines DÉJÀ installées ne reçoivent pas ce noyau par la mise à jour (sources backports absentes) **20 nov., premier démarrage sur sa machine** : le bureau s'affiche (noyau 7.2). Restait : souris « à 10 Hz » — OpenGL sur llvmpipe (Mesa 25.0 n'accélère pas Blackwell) et picom lancé quand même ; Mesa 26.1 de backports épinglé (NVK + zink), picom ne démarre plus sur un rendu logiciel, la construction refuse Mesa < 25.1. Réglages qui « bloquent les clics » : `xrandr --query` réinterrogeait les écrans en bloquant X, passé à `--current` ; l'écran principal est réglé en premier. Alerte « 16 °C, à 4 °C de l'arrêt » : faux seuil critique de l'ACPI (20,8 °C) ignoré, arrêt compté à 105 °C, sondes processeur et carte graphique lues dans hwmon. **[écrit]**, pas encore revu sur sa machine **20 nov. au soir — trois causes PROUVÉES par cle.yml** (clé FAT32 comme la sienne, UEFI Secure Boot, carte PCI sans pilote, rapport relu sur la clé) : (1) l'écran NOIR de 2059 = le noyau 7.3.4 n'a pas de pilote DRM pour l'image du firmware (efifb seul) et `xserver-xorg-video-fbdev` manquait → « no screens found » ; (2) l'image du firmware RAYÉE ET VERTE = GRUB (`gfxmode=auto` + gfxterm) changeait le mode, le noyau peignait en 1280×800 sur un écran en 800×600 ; (3) les deux corrigés, la même machine affiche l'assistant net. **[prouvé en QEMU]**, correctif déposé sur sa clé (G:\live\grenos-correctif.squashfs + config.cfg de GRUB). Les RTX 50 réessaient `nouveau` (X sans glamor : le vert sous nouveau est apparu avec Mesa 26.1) sous un filet — premier écran non passé en 180 s ou coupure au bouton → image du firmware, redémarrage seul. **[écrit]**, pas encore vu sur sa machine. Rapport de démarrage écrit sur la clé (grenos-rapports/) |
| A13 | **Garder Windows à côté** : installer grenOS en système principal sans perdre Windows, et retrouver ses dossiers (Téléchargements, Images, Vidéos…) | **[écrit]** (19 nov.) — l'installateur ne préchoisit rien de destructeur (« à côté de Windows » est proposé par Calamares). `grenos-windows.service` cherche au démarrage la partition NTFS qui porte `Windows/System32` et `Users`, la monte **en lecture seule** sur `/media/windows` (un Windows en veille prolongée ou en démarrage rapide s'abîmerait si on y écrivait) ; l'explorateur montre ses dossiers sous « Windows · lecture seule », OneDrive compris, un titre par compte s'il y en a plusieurs. Testé en unitaire (`les_dossiers_de_windows`) ; pas encore vu sur un vrai PC avec Windows. **Gérer les partitions** : GParted ajouté (déplacer, réduire un NTFS, tout effacer), posé sur le bureau de la clé à côté de « Installer grenOS », action polkit ouverte comme Disques ; pas encore vu sur une image |

## B. Disques, fichiers, démarrage

| # | Demande | État |
|---|---|---|
| B1 | **Protection contre la corruption du système de fichiers** | **[écrit]** (18 nov.) — ext4 monté avec `errors=remount-ro` (Calamares ne l'écrivait pas, lu dans son paquet) : au premier défaut, lecture seule au lieu d'écrire par-dessus ; btrfs vérifié chaque mois (btrfsmaintenance). Aucune installation n'a encore été menée au bout pour le voir dans un fstab |
| B2 | **Chaîne d'approvisionnement sûre** : paquets signés, dépôt authentifié, dépendances vérifiées | [prouvé] pour notre dépôt (clé OpenPGP, signature acceptée, vu à chaque démarrage) ; les dépendances vérifiées : [à faire] |
| B3 | **Retour arrière impossible à casser, même sur coupure de courant** | [à faire] — étude en cours |
| B4 | **Gestion de disques** : partitions, formatage, montage, SMART | **[écrit]** (18 nov.) — Disques (gnome-disk-utility) : partitions, formatage, SMART, images ; ses actions udisks ouvertes à la session locale active (règle polkit 49). Pas vu sur une image |
| B5 | **Chiffrement du disque**, idéalement complet, avec gestion correcte des clés | **[écrit]** (18 nov.) — la case « Chiffrer le système » de l'installateur existait, mais `cryptsetup-initramfs` manquait : un système chiffré n'aurait pas redémarré. Ajouté, et LUKS1 pour que GRUB ouvre /boot. Aucune installation chiffrée n'a encore été menée au bout |
| B6 | **Chiffrement de fichiers** | **[écrit]** (18 nov.) — explorateur › clic droit › « Chiffrer… » : une copie `.gpg` (GnuPG, AES-256, phrase demandée deux fois, transmise par l'entrée standard et jamais en argument) ; « Déchiffrer… » pour l'inverse. Lisible ailleurs, Windows compris. Pas encore vu sur une image |

## C. Sûreté : instantanés, retour arrière, grenRecovery, sauvegardes

> C'est le bloc qu'il a le plus détaillé, et celui dont la panne est la plus
> grave : une machine qui ne revient pas est une machine perdue.

| # | Demande | État |
|---|---|---|
| C1 | **Un instantané avant chaque** installation de pilote, mise à jour de noyau, mise à jour d'OS | **[écrit] pour la moitié qui rend le reste possible** — timeshift 24.06.6-2 est livré, et ses **deux** actions polkit sont ouvertes sans mot de passe. Vérifié dans son `.deb`, pas supposé : un seul fichier, `in.teejeetech.pkexec.timeshift.policy`, qui déclare `…timeshift-gtk` → `/usr/bin/timeshift-gtk` et `…timeshift` → `/usr/bin/timeshift`, **toutes deux en `auth_admin`** — donc mortes sur une machine sans mot de passe. C'est la panne de l'installateur du 25 septembre et celle des disques du 7 octobre, une troisième fois. La session interroge les deux **plus un témoin** (`modify-device-system`, que notre règle n'ouvre pas) : sans lui, un polkit qui dirait oui à tout rendrait la même réponse et la mesure serait morte. **Et l'instantané automatique est écrit** : `grenos-instantane` est appelé par la mise à jour d'avant-bureau, juste avant que dpkg ne touche au système — le seul moment où il vaut quelque chose. Il **rend toujours 0 et est borné à dix minutes** : il tourne avant lightdm, donc tout ce qu'il ferait échouer serait le démarrage de quelqu'un. 7 cas exercés, tous à 0, dont celui qui compte — timeshift qui n'en finit pas est coupé et le démarrage continue. Un disque d'instantanés absent est le cas ordinaire, pas une panne. **Reste** : couvrir l'installation d'un pilote isolé depuis le magasin, qui ne passe pas par là |
| C2 | **Retour arrière** : en cas de problème, ça redevient comme avant | **[écrit], par instantané** (18 nov.) — un instantané Timeshift avant les mises à jour (C1), restaurable par grenRecovery › « Restaurer un instantané » depuis la clé. Ce n'est PAS le retour arrière atomique de B3 : Timeshift recopie, une coupure pendant la restauration peut la laisser à moitié |
| C3 | **grenRecovery** — réparer le démarrage | **[écrit]** « Réparer le démarrage » : GRUB réinstallé (UEFI ou BIOS) dans un chroot, puis son menu. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |
| C4 | grenRecovery — restaurer une sauvegarde | **[écrit]** « Ouvrir les sauvegardes » : l'explorateur sur les disques externes, où grenos-sauvegarde range ses copies datées. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |
| C5 | grenRecovery — réinitialiser le noyau | **[écrit]** « Reconstruire le noyau » : l'initrd de tous les noyaux et le menu de GRUB refaits. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |
| C6 | grenRecovery — vérifier le système de fichiers | **[écrit]** « Vérifier le disque » : fsck (ou btrfs check, xfs_repair) sur la partition FERMÉE, et le verdict dit si des erreurs restent. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |
| C7 | grenRecovery — restaurer un instantané | **[écrit]** « Restaurer un instantané » : Timeshift, qui restaure vers un autre disque depuis la clé. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |
| C8 | grenRecovery — récupérer des fichiers | **[écrit]** « Récupérer des fichiers » : la partition ouverte EN LECTURE SEULE dans l'explorateur. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |
| C9 | grenRecovery — réinitialiser un mot de passe | **[écrit]** « Retirer un mot de passe » : passwd -d dans le chroot, comme grenOS à l'installation. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |
| C10 | **Sauvegardes automatiques intelligentes** | **[écrit]** (18 nov.) — `grenos-sauvegarde` : une copie par jour dès qu'un disque externe est branché, minuteur de session. Pas encore « intelligente » (pas de choix de fichiers) |
| C11 | Sauvegarde **locale, disque externe, réseau** | **[écrit] pour le disque externe** (18 nov.) ; le réseau (NAS, serveur) : [à faire] |
| C12 | Sauvegardes **chiffrées** | **[écrit], par le disque** (18 nov.) — pas de format à nous : un disque de sauvegarde chiffré LUKS (Disques › Formater) chiffre chaque copie et se déverrouille en le branchant ; `gren sauvegarde etat` dit si le disque l'est, et comment le rendre tel |
| C13 | Sauvegardes **versionnées** | **[écrit]** (18 nov.) — trente copies datées, liens physiques pour ce qui n'a pas changé |
| C14 | **Restauration fichier par fichier** | **[écrit]** (18 nov.) — chaque copie datée est un dossier ordinaire : on y reprend UN fichier dans l'explorateur |
| C15 | **Mode sans échec intelligent** : après un plantage noyau, un pilote fautif ou un échec de démarrage, démarrer seul dans un environnement minimal | **[écrit] pour la moitié qui manque à tout le reste** — *savoir* qu'un démarrage a échoué. Un démarrage raté ne laisse rien derrière lui, donc on prend le problème par l'autre bout : un témoin posé au début de chaque démarrage, effacé par l'ouverture du bureau. Un témoin encore là au démarrage suivant ne peut vouloir dire qu'une chose. Il porte le `boot_id` du noyau, sans quoi on relirait le sien et on compterait un échec à tous les coups — la faute exacte du témoin de persistance du 27 septembre. 8 cas exercés, **tous rendent 0** : un compteur d'échecs qui empêche de démarrer serait la panne qu'il prévient. **Et un défaut trouvé par la machine au premier démarrage réel** : la première version demandait à la session d'*effacer* un témoin posé par le service — donc un fichier appartenant à root, depuis une session qui tourne en tant que la personne. L'effacement était gardé derrière un `[ -w ]`, donc il échouait **en silence**, et la machine a aussitôt annoncé « le dernier démarrage n'a pas ouvert de bureau » sur un démarrage qui avait parfaitement ouvert le sien — une mesure qui aurait déclenché le mode sans échec à chaque démarrage réussi, indéfiniment. Le sens est inversé : **déposer demande moins de droits qu'effacer**. Le bureau dépose un jeton dans un dossier à bit collant (1777, comme `/tmp`). 10 cas exercés. **Reste** : démarrer vraiment en environnement minimal au bout de N échecs |
| C16 | Le mode sans échec **nomme la cause probable** : « le dernier démarrage a échoué à cause de… » | **[écrit]** — la cause est demandée au journal du démarrage précédent, pas devinée, et dans l'ordre du plus grave au plus explicite : panique noyau, puis pilote qui ne se charge pas (son exemple exact), puis service qui n'a pas démarré. **Et si le journal ne dit rien, il le dit** — « coupure de courant ou arrêt au bouton » — au lieu d'inventer une cause. Chaque interrogation est bornée à dix secondes : nous sommes avant le bureau |
| C17 | Le mode sans échec **propose** : désactiver le pilote, restaurer la version précédente, redémarrer en mode minimal, analyser | **[écrit]** — `grenos-secours`, une fenêtre qui dit la cause et offre quatre actions. **Un bouton impossible reste visible mais éteint, avec sa raison à côté** : le cacher ferait croire que grenOS ne sait pas le faire, l'allumer sans rien derrière serait la faute capitale de ce dépôt. « Désactiver le pilote » ne s'allume que si le journal **nomme** un module, et jamais pour `ahci`, `nvme`, `ext4` et leurs voisins — les mettre en liste noire empêcherait la machine de voir son disque, c'est-à-dire la panne que ce mode existe pour éviter. Le nom est celui du **module**, pas du micrologiciel : `iwlwifi-cc-a0-72.ucode` donne `iwlwifi`, sans quoi la liste noire n'aurait aucun effet — un correctif qui passe son garde sans rien corriger. 20 cas exercés sur la décision, qui vit dans `secours.py` sans aucun GTK. **Reste** : le redémarrage en environnement minimal, qui demande de toucher au chargeur |
| C18 | **Recovery plus solide** (sa première liste) | **[écrit]** la liste ci-dessus, dans une seule fenêtre ; le mode sans échec y mène. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |

## D. Sécurité

| # | Demande | État |
|---|---|---|
| D1 | **Pare-feu** | **[écrit]** (18 novembre) — ufw livré ; activé au premier démarrage par le parcours Dev (entrant refusé, sortant permis). Hors Dev : pas activé par défaut (LocalSend, KDE Connect écoutent) |
| D2 | **Validation stricte des paquets** réseau | [à faire] |
| D3 | Protection contre le **déni de service** | **[écrit]** (18 nov.) — `/etc/sysctl.d/50-grenos-durcissement.conf` : SYN cookies, pas de réponse aux diffusions. Rien ne mesure encore une vraie inondation |
| D4 | Protection contre l'**usurpation ARP** | **[écrit], partiel** (18 nov.) — filtre de chemin inverse (lâche, pour ne pas casser WireGuard), ARP limité à la bonne carte, redirections ICMP refusées. Aucun réglage du noyau n'empêche l'usurpation ARP sur un réseau partagé : c'est le pare-feu et le VPN qui protègent, et c'est écrit dans le fichier |
| D5 | **Clés privées jamais en clair** dans l'OS ni dans les images | [prouvé] — la clé de signature vit dans un seau Supabase privé, jamais dans le dépôt |
| D6 | **Isolation et sécurité du noyau** | **[écrit]** (18 nov.) — adresses du noyau masquées, dmesg à root, ptrace limité aux enfants, eBPF à root, liens et tubes protégés dans /tmp. Les espaces de noms restent ouverts (bacs à sable de Flatpak, Steam, navigateurs) |
| D7 | **Bac à sable** avec des profils existants (Windows 10/11, Debian, Ubuntu) | **[écrit], par GrenPlace** (18 nov.) — GNOME Boxes est au catalogue : il télécharge lui-même Debian, Ubuntu, Fedora et d'autres, et accepte une image Windows (qu'on apporte, avec sa licence). Rien dans l'image : une machine virtuelle se choisit |
| D8 | **Contrôle de permission par application** | **[écrit], pour Flatpak seulement** (18 nov.) — Flatseal est dans GrenPlace et règle les permissions de chaque application Flatpak ; rien pour les paquets Debian |
| D9 | **Gestion de Secure Boot** | **[écrit]** (20 nov., « je ne veux pas désactiver Secure Boot ») — la CLÉ : shim signé Microsoft, GRUB et noyau 7.3.4 signés Debian (lu dans l'ISO 1442). Le SYSTÈME INSTALLÉ : l'installateur posait `grub-efi`, donc un GRUB non signé ; `shim-signed` et `grub-efi-amd64-signed` sont maintenant dans l'image (la construction l'exige), pour que `grub-install` pose la chaîne signée. Le PILOTE NVIDIA : module signé par une clé grenOS (chemins DKMS), inscrite par MOK avec un code d'un seul usage donné à la fin de l'installation — **prouvé** en CI que nvidia.ko est signé par cette clé (run 44766908128). Démarrage de l'ISO dans OVMF secboot (clés Microsoft) : essai ajouté, rapporté. Pas encore vu sur un vrai PC, ni l'installation menée au bout avec le Secure Boot |
| D10 | **Gestion des clés SSH et GPG** | **[écrit]** (18 nov.) — Seahorse livré : clés SSH et GPG en fenêtre |
| D11 | **Gestionnaire de mots de passe** | **[écrit], à installer** (18 nov.) — KeePassXC est dans GrenPlace, pas dans l'image |
| D12 | **Politique de sécurité** : un chemin clair pour signaler et corriger une vulnérabilité | **[écrit]** (18 nov.) — `docs/SECURITE.md` à jour et `.github/SECURITY.md` pour que GitHub l'affiche. **Le signalement privé de GitHub est DÉSACTIVÉ** sur le dépôt : le lien de la politique ne mène nulle part tant que Grenofar ne l'active pas |

## E. Observation : grenHub, grenPerf, diagnostic, journaux

> **17 novembre — [écrit], pas encore vu sur une image :** grenPerf est passé
> dans grenos-ui (Rust/Tauri), sur le modèle de Perf Saver : processus
> regroupés par application avec processeur et mémoire mesurés dans /proc,
> fermer et changer la priorité d'un clic (la barre, le son et le bureau sont
> protégés), « Libérer la RAM » qui dit honnêtement ce qu'elle gagne, ce qui
> démarre avec la session, et un mode jeu réversible. Et **GrenLaunch**
> (`grenlaunch`) : la bibliothèque Steam (lue dans ses manifestes, donc un jeu
> installé par Steam y apparaît seul), Heroic (Epic, GOG), Flathub, 30 jeux au
> catalogue avec ce qui marche vraiment sous Linux — Fortnite en nuage,
> Valorant, LoL, Apex, CoD, PUBG et Destiny 2 dits « non disponibles ».

| # | Demande | État |
|---|---|---|
| E1 | **grenHub** : une application centrale qui affiche l'état complet de la machine | **[écrit]** (18 nov.) — grenHub, passé dans grenos-ui ; ses lignes série sont attendues par la CI |
| E2 | **Mode diagnostic automatique** : tester CPU, RAM, stockage, réseau, GPU, USB et produire un rapport | **[écrit]** (18 nov.) — le diagnostic de grenHub (processeur, mémoire, disque, réseau, affichage, USB, services) et son verdict |
| E3 | **grenPerf** : analyser les tâches d'arrière-plan inutiles au bon fonctionnement, et la gestion de la RAM | **[écrit]** — grenPerf (grenos-ui) nomme ce qui tourne et ce qui est de trop pour jouer |
| E4 | **grenOS ne doit pas beaucoup consommer** en arrière-plan | [prouvé] et mesuré : 17 services dont 16 sont de Debian ; le seul en trop a été retiré |
| E5 | **Moniteur système** avec température et état du matériel | **[écrit]** — grenHub montre les sondes **et les ventilateurs**, un ventilateur arrêté sur une machine chaude étant le signe le plus clair d'une panne. Dix cas sur de faux `/sys`, dont le portable qui n'a que des zones ACPI — et ce cas a trouvé un vrai défaut : sans `hwmon`, la lecture sortait avant de les regarder. La CI n'a ni sonde ni ventilateur, donc elle ne prouve que l'honnêteté de la page quand il n'y en a aucun |
| E6 | **Journaux persistants** | **[écrit]** — `Storage=persistent`, 100 Mo au plus, un mois au plus. On ne croit pas le fichier : la session regarde ce que journald a **fait**, puisqu'il ne crée `/var/log/journal` que s'il a lu le réglage — une directive mal orthographiée ne créerait rien et la mesure le dirait. Et le nombre de démarrages connus est ce qui transforme « persistant » en preuve : au second démarrage de la même machine il doit valoir au moins deux. C'est aussi ce qui rend possible le détecteur de démarrage raté, qui lit `journalctl -b -1` : sans journal persistant il aurait toujours répondu « rien dans le journal ». Verdict exercé sur sept journaux, le bloc extrait du workflow lui-même |
| E7 | **Vidages de plantage** (crash dump) | **[écrit]** (18 nov.) — systemd-coredump livré : `coredumpctl` liste les plantages |
| E8 | **Rapport de panique du noyau** | **[écrit]** (18 nov.) — systemd-pstore garde le rapport quand l'UEFI le conserve ; `kernel.panic = 10` redémarre au lieu de figer, et le mode sans échec prend la main |
| E9 | **Gestionnaire CPU / RAM / disque / réseau / processus**, propre | [prouvé] pour les processus (`taches ouvert, 103 processus`) ; le reste : [à faire] |
| E10 | **Tests de charge sur plusieurs jours** : CPU, RAM, disque, réseau | [à faire] — hors de portée de la CI, qui dure 40 minutes |
| E11 | **Fuzzing continu en CI** | **[écrit]** (18 nov.) — `linux/ui/src/aleatoire.rs` : 20 000 entrées au hasard par analyseur (nmcli, bluetoothctl, journal du noyau, dpkg, pourcentages, chemins, UTF-8 coupé), à chaque `cargo test` donc à chaque construction, graine du jour imprimée en cas d'échec. **Il a trouvé une panique réelle au premier passage** : la lecture des pourcentages coupait un caractère multi-octets (« ██42% » de flatpak), dans trois fichiers dont GrenLaunch, déjà publié. Corrigé. Pas encore de fuzzing guidé par la couverture (cargo-fuzz) |
| E12 | **Un test de régression par bogue corrigé** | [prouvé] comme pratique : chaque correctif de la semaine est arrivé avec son essai, exercé dans les deux sens |

## F. Énergie, écrans, périphériques à chaud

| # | Demande | État |
|---|---|---|
| F1 | **Veille et réveil** | **[écrit] pour la veille** — « Mettre en veille » est dans le menu d'arrêt, et **il n'apparaît que si le noyau sait le faire** : `/sys/power/state` est lu avant de l'offrir. Un bouton qui échoue au clic serait pire que pas de bouton. `org.freedesktop.login1.suspend` est en `allow_active = yes` — lu dans le `.deb` de systemd 257.13 — donc aucune règle polkit n'est nécessaire. La session dit ce que la machine sait faire, et **le mot compte plus que le oui** : `mem` est la vraie veille, `freeze` n'arrête que les processus. **Ce qui n'est pas prouvé** : le clic lui-même — l'endormir au milieu du parcours tuerait la construction, et personne ne serait là pour la réveiller. Réveil, extinction, batterie et thermique restent à faire |
| F2 | **Extinction et redémarrage** | [prouvé] — le menu d'arrêt, et l'écran d'extinction est noir et propre |
| F3 | **Batterie** | **[écrit]** (18 nov.) — la barre native montre la charge et l'état (en charge, sur batterie, pleine, branchée sans charger), lus dans /sys sans lancer de programme ; une notification sous 10 %, une seule fois. La machine d'essai n'a pas de batterie : quatre essais Rust la simulent |
| F4 | **Thermique** | **[écrit]** (18 nov.) — la barre native lit les zones thermiques toutes les 30 s (sans programme) et prévient UNE fois à 5 °C du seuil critique (95 °C sans seuil déclaré), réarmée 10 °C plus bas ; grenHub montrait déjà sondes et ventilateurs. La machine d'essai n'a pas de sonde : trois essais Rust |
| F5 | **Branchement à chaud** USB, réseau, stockage, sans plantage | **[prouvé] pour la clé USB, [à faire] pour le réseau** (18 nov.) — la même clé, branchée puis retirée à chaud ; la CI vérifie les deux annonces et que la barre ne tombe pas. Le réseau à chaud : pas encore — deux passages verts du même commit (CI 44343831083 sur claude/suite, 44354350634 sur main) |
| F6 | **Multi-écran** | **[écrit]** (18 nov.) — arandr, depuis Réglages › Écran › « Disposer les écrans » |
| F7 | **Résolution** | [prouvé] — page Écran, 13 définitions, 11 fréquences |
| F8 | **DPI** | **[écrit]** (18 nov.) — Réglages › Apparence › Taille de l'affichage (100 à 200 %), `Xft.dpi` gardé dans ~/.Xresources |

## G. Les applications — la grande liste

> Trois familles, et il faut les distinguer : ce que Debian livre déjà (on
> l'ajoute et on l'habille), ce qui existe chez nous (on l'améliore), et ce
> qu'il faut écrire.

### G.1 — Ce qui existe chez nous et qu'il faut pousser plus loin

| # | Demande | État |
|---|---|---|
| G1 | Gestionnaire de fichiers moderne — **onglets** | **[écrit]** — Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+1 à Ctrl+9, et une rangée qui se cache quand il n'y a qu'un onglet. **Un onglet n'est pas une seconde liste** : c'est un dossier mémorisé avec son histoire, donc ni le modèle, ni le filtre, ni le tri ne bougent — rien de ce qui est prouvé ne pouvait tomber. La mécanique vit dans `onglets.py`, sans GTK, exercée en 30 cas : fermer le dernier onglet, l'histoire propre à chaque onglet, Ctrl+7 dans une rangée de deux, et la racine qui n'a pas de nom court. Le clavier passe de 28 à 38 cas. La machine exerce Ctrl+T puis Ctrl+W et lit le compte |
| G2 | — **recherche** | **[écrit] — et ma propre liste avait tort** : le champ existe dans la barre depuis des semaines, avec Ctrl+F pour y aller et Échap pour l'effacer. Mais **rien ne l'essayait** : la machine exerçait Ctrl+H, Ctrl+Maj+N et F2, pas celui-là. *Un geste qu'aucune machine n'essaie est un geste dont on ne sait rien.* Elle tape maintenant « doc » et vérifie que le compte **descend** — un champ qui avalerait les lettres sans filtrer montrerait exactement le même écran |
| G3 | — **aperçu** | **[écrit]** (18 nov.) — Alt+P ou le bouton œil ouvre un volet : l'image (8 Mo au plus), le début d'un texte (16 Ko), le contenu d'un dossier. Vu dans l'aperçu du navigateur, pas encore sur une image |
| G4 | — **compression / décompression** | **[écrit]** — « Compresser en .zip » et « Extraire ici » dans le menu du clic droit. **Le .zip plutôt que le 7z** : les deux sont sur l'image et 7z compresse mieux, mais un .zip s'ouvre partout, et quelqu'un qui compresse le fait presque toujours pour envoyer. **Rien ne peut être écrasé** (`archive-2.zip`), et l'extraction va dans un dossier au nom de l'archive — trente fichiers déversés en vrac ne se rattrapent pas. `/usr/bin/7z` vérifié dans le `.deb` 25.01, le binaire ne s'appelle pas `7zz`. Un garde refuse l'image si le menu promet sans que `zip` et `7z` soient là, **même sans xarchiver** : 5 cas, 3 refus |
| G5 | — **montage de clé USB** | **[écrit]** (18 nov.) — l'explorateur monte un disque d'un clic dans « Ce PC » (udisksctl, sans mot de passe) |
| G6 | — **formatage de clé USB** | **[écrit]** (18 nov.) — par Disques (voir B4) |
| G7 | **Paramètres système centralisés** | [prouvé] — huit pages, ouvertes et photographiées |
| G8 | **Centre de mise à jour avec retour arrière** | [prouvé] pour la mise à jour (y compris hors session, avant le bureau) ; le retour arrière : [à faire] |
| G9 | **Barre des tâches** | [prouvé] |
| G10 | **Gestionnaire de fenêtres propre** | [prouvé] — openbox à notre thème, boutons dessinés, croix vérifiée au pixel |
| G11 | **Bureaux virtuels** | **[écrit]** (18 nov.) — deux bureaux existaient sans AUCUN raccourci pour y aller. Ctrl+Win+←/→ pour changer, avec Maj pour emmener la fenêtre, Ctrl+Win+D pour en ajouter un, Ctrl+Win+F4 pour fermer ; la barre ne montre plus que les fenêtres du bureau courant (essai Rust) |
| G12 | **Dock** | [à faire] — la barre fait office, ce n'est pas la même chose |
| G13 | **Notifications** | **[écrit]** (18 nov.) — dunst à notre thème ; AUCUN démon n'affichait les notifications jusqu'ici |
| G14 | **Gestion audio complète** | [prouvé] pour le panneau et le mélangeur |
| G15 | **Volume par application** | **[écrit]** — le panneau du son règle chaque application qui joue (natif depuis le 18 nov.) |
| G16 | **Accélération vidéo GPU** | [prouvé] — sept pilotes VA-API, libavcodec, et Firefox configuré selon la carte |
| G17 | **Navigateur web** | [prouvé] — Firefox ESR |
| G18 | **grenStore** (le nom qu'il donne à GrenPlace) | [prouvé] pour le magasin, 99 applications en ligne ; le nom et la refonte façon Microsoft Store : [à faire] |

### G.2 — Ce que Debian livre et qu'il faut intégrer

| # | Demande | État |
|---|---|---|
| G19 | **Lecteur PDF** | **[écrit]** (18 nov.) — Atril livré |
| G20 | **Visionneuse d'images** | **[écrit]** (18 nov.) — Ristretto livré |
| G21 | **Lecteur vidéo et audio** façon VLC | **[écrit]** (18 nov.) — Celluloid (mpv) livré ; VLC dans GrenPlace |
| G22 | **Bluetooth** | [à faire] — la page Connexions existe, le matériel n'est pas prouvé |
| G23 | **Support manette** | **[écrit]** (18 nov.) — `steam-devices` : les règles udev qui ouvrent les manettes Xbox, PlayStation, Switch et Steam à un compte ordinaire. 24 nov. : Xbox en Bluetooth — bluez réglé pour qu’elles se reconnectent (`appliquer-systeme`, rejoué à chaque mise à jour) ; 25 nov. : l'adaptateur sans fil (clé USB 045e:02fe) a son pilote, `grenos-adaptateur-xbox` (linux/adaptateur-xbox : réécriture de xow en Rust, sans module noyau, rien à signer) — essayé sur la machine de Grenofar avec une manette Series : appairée, connectée, entrées reçues |
| G24 | **Client SSH** | **[écrit]** (18 nov.) — openssh-client livré |
| G25 | **Outils de diagnostic réseau** | **[écrit]** (18 nov.) — mtr, traceroute, dig livrés |
| G26 | **VPN** | **[écrit]** (18 nov.) — OpenVPN dans la fenêtre des connexions, WireGuard compris par NetworkManager |
| G27 | **Pare-feu avec interface graphique** | **[écrit]** (18 nov.) — Gufw livré, son action polkit ouverte (lue dans le .deb) |
| G28 | **Gestionnaire de téléchargement** | **[écrit]** (18 nov.) — uGet livré (2.2.3, trixie) avec aria2 : pause, reprise après coupure, planification, plusieurs connexions par fichier ; ne tourne que si on l'ouvre |
| G29 | **Gestion de disques** (partitions, formatage, montage, SMART) | **[écrit]** (18 nov.) — voir B4 |

### G.3 — Ce qu'il faut écrire

| # | Demande | État |
|---|---|---|
| G30 | **Terminal avec un shell grenOS** | **[écrit]** (18 nov.) — grensh : invite, verbes Verbe-Nom en français, `gren` et `grenget` |
| G31 | **Presse-papier avec historique** (Windows+V) | **[écrit]** (18 nov.) — `grenos-presse-natif` (Rust natif, toute la session) garde les 25 derniers textes copiés, EN MÉMOIRE seulement ; Win+V les montre, Entrée en recopie un, Suppr en retire un. Il ne colle pas tout seul comme Windows (il faudrait simuler Ctrl+V) : la fenêtre le dit. Quatre essais |
| G32 | **Calendrier** | **[écrit]** (18 nov.) — l'Agenda (grenos-ui) : le mois, les jours fériés français (Pâques calculé), des événements gardés dans ~/.local/share/grenos/agenda.json (écrit par renommage, jamais à moitié). Pas encore de rappel à l'heure, et la page le dit |
| G33 | **Horloge** | **[écrit]** (18 nov.) — l'application Horloge (grenos-ui) : l'heure ici et dans huit villes |
| G34 | **Minuteur** | **[écrit]** (18 nov.) — Horloge › Minuteur, qui prévient par une notification ; et un chronomètre avec les tours |
| G35 | **Bloc-notes** | **[écrit]** — Mousepad est livré ; pas d'application à nous |

## H. GrenAI

| # | Demande | État |
|---|---|---|
| H1 | **Lancer des IA locales** téléchargées depuis Hugging Face | **[écrit], par Ollama** (18 nov.) — GrenAI parle à un modèle qui tourne sur la machine (Ollama, 127.0.0.1:11434) ; Debian trixie n'a ni llama.cpp ni Ollama : on l'installe par GrenPlace (Alpaca l'embarque, avec les modèles d'Ollama et de Hugging Face) |
| H2 | Ou **une alternative** si ce n'est pas possible | [prouvé à la source] — **Ollama n'existe ni dans Debian trixie ni sur Flathub** (vérifié le 1er octobre : l'API de Flathub ne rend que des *clients*). L'équivalent livrable est Alpaca (`com.jeffser.Alpaca`, 2,26 Go), qui embarque son moteur |
| H3 | **Clés API** avec le choix entre GPT, Grok et Gemini | **[écrit]** (18 nov.) — GrenAI : OpenAI (GPT), Google (Gemini), xAI (Grok), avec la clé de la personne, gardée dans un fichier 0600 et passée à curl par un fichier d'en-têtes effacé aussitôt — jamais en argument ni dans l'adresse (essai). La page dit à chaque instant où partent les mots |
| H4 | **Assistant de terminal** capable de corriger des commandes | **[écrit]** (18 nov.) — `corrige` (grensh) reprend la commande d'avant et demande à GrenAI la version corrigée, avec ce qui n'allait pas ; `ia « question »` aussi. RIEN n'est relancé : c'est la personne qui décide. Mêmes clés et même code que la fenêtre (`grenos-ui --grenai`) |
| H5 | **Automatisation de tâches** | [à faire] |
| H6 | **OCR** intégré à la capture d'écran et aux PDF | **[écrit] pour les fichiers** (18 nov.) — explorateur › clic droit › « Extraire le texte (OCR) » : Tesseract (français, anglais) écrit un .txt à côté d'une image, ocrmypdf rend un PDF scanné cherchable. Tout en local. Pas encore dans la capture d'écran |
| H7 | **Traduction hors ligne** | **[écrit], pour le Web** (18 nov.) — Firefox ESR traduit les pages SUR la machine (Firefox Translations, sans serveur). Les documents : par GrenAI avec Ollama, en local aussi. Pas de traducteur hors ligne à nous |
| H8 | **Dictée vocale locale** | [à faire] |
| H9 | **Résumé automatique de documents** | **[écrit]** (18 nov.) — GrenAI › « Résumer un document » : un texte ou un PDF (pdftotext), coupé à 12 000 caractères et la page le dit, envoyé au modèle choisi |
| H10 | **Recherche sémantique dans les fichiers** | [à faire] |
| H11 | **Jamais en arrière-plan** sauf s'il le choisit | **[écrit]** (18 nov.) — par construction : GrenAI n'a aucun service, aucun minuteur ; il n'existe que pendant que sa fenêtre est ouverte, et chaque requête part d'un clic |

## I. L'assistant de première configuration (OOBE)

> **17 novembre : l'assistant est réécrit en Rust/Tauri (`linux/ui`, `grenos-ui premier`).**
> L'assistant Python reste livré et reprend la main tout seul si la nouvelle
> fenêtre ne s'affiche pas en deux minutes ou se ferme sans avoir nommé la
> machine (`grenos-premier-lanceur`). Nouveau et **[écrit]**, pas encore vu
> tourner sur une image : I4 (réseau), le mot de passe de I5, la couleur
> d'accent et le niveau d'effets de I6, le disque/Wi-Fi/Bluetooth/son de I14,
> l'écran final de I15. Et I11 est enfin **appliqué** : le choix des mises à
> jour n'était rangé dans aucun fichier que quelqu'un relisait.

> Quatorze écrans, dans son ordre exact. `grenos-premier` existe déjà et fait
> aujourd'hui **deux** de ces quatorze écrans (le nom, et le pays/clavier).

| # | Écran | État |
|---|---|---|
| I1 | Au tout début **en anglais automatiquement**, puis « Welcome to grenOS », « configurons votre ordinateur en quelques étapes », un bouton **Start** | **[prouvé]** — la machine a traversé l'écran « Welcome to grenOS » et son bouton **Start** : `premier : ecran bienvenue (1/10)`. En anglais, et c'est la seule page qui l'est — quelqu'un qui ne lit pas le français doit pouvoir atteindre l'écran où l'on choisit la langue |
| I2 | **Langue** — toutes les langues du monde, ou anglais et français si c'est trop long —, **région**, **format de date et d'heure**, **en ligne pour avoir l'heure**, bouton Continuer | [écrit] à moitié : le pays et le fuseau existent. **L'heure en ligne était une promesse vide** — l'interrupteur des Réglages appelle `set-ntp`, et aucun démon n'était livré ; corrigé le 14 octobre, avec un garde. La **langue**, elle, demande d'abord que nos fenêtres soient traduisibles : aucune ne l'est, tout le texte est du français en dur |
| I3 | **Clavier** : azerty, qwerty, etc., **avec un champ pour le tester**, bouton Continuer | **[prouvé]** — et je l'avais marqué « à faire » à tort : le champ existe (`grenos-premier:215`), la bascule est immédiate (`clavier.appliquer_maintenant`), 16 dispositions. Démenti en relisant le code, pas en le croyant |
| I4 | **Connexion à Internet** : une liste nette et fluide des réseaux, le mot de passe au clic, un bouton « continuer hors connexion », puis Continuer | **[écrit]** (17 nov.) — dans grenos-ui : la liste des réseaux par `nmcli`, le mot de passe au clic, « Continuer hors connexion ». Aucune machine d'essai n'a de Wi-Fi : seule la sienne pourra le prouver |
| I5 | **Votre compte** : un nom qui devient `…@grenos`, un mot de passe au login s'il le souhaite, Continuer | [prouvé] pour le nom et le compte ; **le mot de passe optionnel est [écrit]** (17 nov., grenos-ui → `grenos-compte definir`) |
| I6 | **Apparence** : sombre ou clair ou auto, choix de couleurs, **aperçu du bureau à droite**, Continuer | **[prouvé] pour les trois modes** — `premier : ecran apparence (6/10)`, et le choix est gardé. **L'aperçu du bureau à droite n'y est pas, et c'est délibéré** : l'assistant tourne avant qu'un bureau existe, donc l'aperçu serait un dessin qui lui ressemble sans en être un, et le jour où il divergerait il mentirait à l'endroit même où il sert à décider. Le choix de couleur d'accent reste à faire |
| I7 | **Usage du PC** — s'il choisit *développement* : un bouton installe VS Code, Git, un terminal amélioré, les outils de compilation et GrenAI | **[écrit]** — quatre profils (bureautique, développement, jeu, montage), et c'est le **seul écran dont le choix installe vraiment quelque chose** : les applications partent dans la file d'avant-bureau, qui est prouvée depuis le 27 septembre. Chaque profil ne nomme que des fiches du catalogue, et un garde refuse un profil qui nommerait une fiche absente — 17 cas. **GrenAI n'est pas proposée** : zéro ligne écrite sur onze demandes, et la nommer serait une promesse de plus. Le dépôt du travail n'a pas encore été vu sur une machine |
| I8 | — s'il choisit *jeu* : **Ctrl+Alt+Échap** ouvre un mode jeu qui ne laisse tourner que l'arrière-plan de sécurité, coupe GrenAI, et propose de fermer les autres applications | **[écrit]** (18 novembre) — Ctrl+Alt+Échap ouvre grenPerf sur son onglet Mode jeu (`grenos-ui jeu`) : ce qui est coché se ferme, le reste passe en priorité basse, Steam/Proton gardent la leur, réversible. et depuis le 18 nov. au soir il coupe GrenAI : sa fenêtre se ferme, et Ollama (le modèle local, plusieurs Go en mémoire) est arrêté s'il tourne pour la personne |
| I9 | **Confidentialité** : « vos données vous appartiennent » écrit clairement, choix de télémétrie (aucune / minimale anonyme / complète), **aucune par défaut**, Continuer | **[prouvé]** — `premier : choix gardes — telemetrie aucune`. Les trois niveaux, chacun disant ce qu'il envoie, et **aucune par défaut** comme il l'a demandé |
| I10 | **Sécurité** : « protégeons votre ordinateur », l'état (Secure Boot, noyau signé, mises à jour auto…), et activer le chiffrement du disque | **[écrit]** — l'état, et **aucun interrupteur**, selon son propre garde-fou (« il ne faut pas que ça se retourne contre nous »). Secure Boot est lu dans sa variable UEFI, pas deviné, et la phrase dit la vérité qui dérange : grenOS n'est pas signé, donc il faut le désactiver. Le chiffrement se décide à l'installation sur disque — l'offrir ici n'aurait rien à chiffrer |
| I11 | **Mises à jour** : automatiques, me demander, ou manuelles | **[prouvé]** — `premier : choix gardes — majs automatiques`. Automatiques (recommandé), me demander, manuelles |
| I12 | **Applications** : proposer les applications à installer (navigateur et lecteur multimédia déjà là ; grenPerf, GrenAI proposés ici et dans grenStore, pas installés d'office) | **[prouvé] pour la proposition** — `premier : ecran applications (9/10)`, et **aucune case cochée** : « grenPerf et GrenAI ne s'installent pas tout seuls ». L'installation effective reste à brancher sur GrenPlace |
| I13 | **Sauvegardes** : activées ou non, destination (disque externe, NAS, serveur, nuage), **et tester la récupération** | **[écrit]** (18 novembre) — écran Sauvegardes : copie quotidienne sur disque externe (`grenos-sauvegarde`, rsync versionné `--link-dest`, 30 copies), **chaque copie relue** (un fichier tiré au hasard comparé octet par octet). `gren sauvegarde etat|activer|maintenant`. Réseau (NAS, serveur) et nuage : [à faire], annoncés comme tels |
| I14 | **Optimisation automatique** : « nous avons détecté votre matériel… », la liste (CPU, GPU, RAM, SSD, NVMe, HDD, Wi-Fi, Bluetooth, audio), « grenOS a trouvé des optimisations possibles », bouton Optimiser | **[écrit] en partie** — processeur, mémoire, carte graphique et pilote, lus par `materiel.py` ; la case d'optimisation dépose `grenos-performances` dans la file plutôt que de toucher au système pendant qu'on répond. Manquent le Wi-Fi, le Bluetooth et le genre de disque |
| I15 | **Écran final** : il met l'OS en place, installe les applications choisies, règle Ctrl+Alt+Échap selon les choix, puis **« grenOS est prêt »** apparaît | **[écrit]** (18 nov.) — l'écran existait déjà : « Mise en place… » coche les gestes (compte, clavier, apparence, mises à jour, applications déposées pour le prochain démarrage), puis « grenOS est prêt » et le bureau. La CI le traversait (« écran final (15/15) », « choix gardés ») sans voir l'écran de fin : la page dit maintenant « écran grenOS est prêt affiché » une fois dessiné, attendu par la CI (rapporté jusqu'à deux passages verts). Ctrl+Alt+Échap ne dépend pas des choix : il ouvre le mode jeu pour tous (I8) |
| I16 | Après le clavier, la langue et le nom : le choix entre **Rapide**, **Personnalisé (recommandé)** et **Dev** | **[prouvé]** — `premier : ecran parcours (5/10)`, **après** la langue, le clavier et le nom comme il l'a demandé. Rapide / Personnalisé (recommandé) / Dev, et la mécanique est exercée en 35 cas : « tout cela plus » n'est pas une façon de parler, Rapide est inclus dans Personnalisé et Personnalisé dans Dev |
| I17 | Le parcours **Dev** ajoute : partitionnement, chiffrement, Secure Boot, swap, ordonnanceur, services, réseau, pare-feu, télémétrie, journaux, mises à jour, bac à sable, paramètres du noyau | **[écrit] en partie** (18 novembre) — Dev ajoute Système (zram, swappiness), Pare-feu (ufw, entrant refusé) et Journaux (durée/taille), tous appliqués par `premier.rs`. Partitionnement, Secure Boot et bac à sable restent hors de la liste : rien ne pourrait encore les tenir |

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
| J6 | Brancher une clé USB | **[prouvé]** (18 nov.) — une clé FAT32 branchée PENDANT la session (moniteur QEMU, USB 3) ; udev la dit avec son étiquette et son système de fichiers, et l'explorateur la monte d'un clic — deux passages verts du même commit (CI 44343831083 sur claude/suite, 44354350634 sur main) |
| J7 | Naviguer | [prouvé] |
| J8 | Lire une vidéo | **[écrit]** (18 nov.) — Celluloid est livré ; le décodage était déjà prouvé |
| J9 | **Mettre en veille** | **[prouvé]** (18 nov.) — la machine d'essai ouvre Fin de session, appuie sur V (touche d'accès, comme le menu de Windows), puis QEMU la réveille ; un crochet systemd-sleep dit « veille : entree » et « veille : reveil ». « la machine est partie en veille » puis « elle s'est réveillée » dans deux passages verts du même commit (CI 44343831083 sur claude/suite, 44354350634 sur main) |
| J10 | Redémarrer | [prouvé] |
| J11 | **Récupérer d'une erreur** | **[écrit]** le mode sans échec dit la cause et propose ; grenRecovery répare de l'extérieur. (18 nov.) — grenRecovery (grenos-ui, depuis la clé grenOS) trouve les installations grenOS sur les disques ; tout le travail root passe par `grenos-recuperation`, qui vérifie chaque partition et démonte toujours. Pas encore exercé sur une vraie installation cassée |

## K. La fabrique et la documentation

| # | Demande | État |
|---|---|---|
| K1 | **Mise à jour des applications**, pas seulement du noyau | **[écrit]** (17 nov.) — les Flatpak se mettent à jour depuis la fenêtre Mises à jour, chaque jour en mode automatique, et au redémarrage de mise à jour. Pas encore vu sur une machine |
| K2 | **Que les mises à jour fonctionnent correctement** (il l'a écrit deux fois) | [prouvé] pour le dépôt, le bouton, le service, la fenêtre d'avant-bureau et le retour du bureau |
| K3 | **Constructions reproductibles** : deux constructions indépendantes doivent donner exactement le même noyau | **[écrit]** (18 nov.) — Debian figé (snapshot.debian.org, `linux/debian-instantane`), date unique (SOURCE_DATE_EPOCH), Rust et Debian de construction figés, Cargo.lock suivis, PNG sans date, Flathub sans téléchargement. `reproductible.yml` construit DEUX fois et compare à l'octet : pas encore vert ; le 18 nov. au soir, il ne restait que deux différences, corrigées dans 121826a : le nom tiré au sort du clavier dans l'initrd (crochet `zz-grenos-clavier-fixe`) et les icônes, qui dépendaient des instructions SIMD du processeur (`octets()` arrondit avant de tronquer). Double construction relancée (run 44374598459). Résultat (run 44374598459) : les icônes et le clavier sont réglés ; il restait UNE différence, le microcode AMD présent dans un initrd et pas dans l'autre — Debian n'y met que celui du processeur qui construit. `/etc/default/{amd64,intel}-microcode` passent en « early » : les deux, toujours (et une clé construite sur un Intel démarrait les AMD sans leur microcode) |
| K4 | **Documentation développeur** : architecture, pilotes, appels système, contribution, débogage | **[écrit]** (18 nov.) — `docs/DEVELOPPEUR.md` remis à jour : l'architecture Rust, ajouter une fenêtre, construire, les raccourcis de CI |

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
