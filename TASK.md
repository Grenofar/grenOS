# TASK.md — ce qui est planifié, dans l'ordre

> Tenu par Claude. Mis à jour le 2026-11-26. Une session qui reprend
> le travail lit d'abord ce fichier, puis `CLAUDE.md` (local, RÈGLE N°1 :
> plug and play pour un non-développeur).

## 00000000. EN COURS — 26 novembre soir : temprep, et le pilote de la GT 1030

**27 novembre, midi** : pilote PUBLIÉ (linux-20261127-0749, APT 1.0.202611270714).
Sur la GT 1030, après redémarrage : Xorg sur NVIDIA, glxinfo « GeForce GT 1030 ».
Rocket League (Heroic, FLATPAK) plantait : il manquait l'extension Flatpak du
pilote (org.freedesktop.Platform.GL/GL32.nvidia-580-178-04), que Flatpak ne
pose que si le pilote est chargé quand il installe/met à jour une appli.
Posée à la main → « c bon » (Grenofar). Pour tous : `grenos-flatpak-nvidia`
(+ service au démarrage, + appelé par le bouton), essai 10 cas. Le PC est un
i5-3470 (Ivy Bridge, sans AVX2) ; Proton-CachyOS y tourne.

**27 novembre, matin (Grenofar parti à 7 h 45, « prêt avant midi »)** :
- La CI du 26 au soir (Pilote signe 45897346153) : variante fermée ROUGE.
  Le BUILD d'Ubuntu lie les objets par /usr/bin/ld.bfd puis exige les
  empreintes de Canonical : avec binutils 2.44 de Debian, « checksums did
  NOT match ». Canonical lie en 2.46. Le ld 2.46 d'Ubuntu, extrait avec sa
  glibc et lancé seul, redonne les cinq empreintes [prouvé ici, objets
  7.0.0-44.44]. `grenos-pilote-signe` : latelink=false pour le paquet, puis
  le BUILD de Canonical TEL QUEL avec ce ld (unshare -m, mount --bind).
- SUR SON PC à GT 1030, par temprep, la séquence exacte du bouton
  (installer.rs) avec les fichiers du dépôt [fait, vu] : 5 modules « OK »,
  nvidia.ko signé Canonical, module et bibliothèques 580.178.04, chaîne
  posée, dpkg propre, seul nvidia-driver-580.list branché. Les modules
  « open » étaient revenus pendant la nuit/le matin : retirés par
  `retirer_autre_variante`. PAS redémarré (sa session était ouverte) :
  [à voir] au redémarrage, Xorg sur NVIDIA, glxinfo, Rocket League.
- Réouverture automatique de temprep après redémarrage : REFUSÉE par le
  contrôle de sécurité de Claude Code (accès root à distance sans la
  personne) — non faite. Page /tempcorr remise 10 min puis retirée.


Grenofar a installé grenOS sur un 2e PC (GT 1030, Pascal, 10de:1d01). Demandes : un outil pour que Claude envoie
des commandes à un autre PC (« temprep », dans grenget) ; retirer le pilote
« douteux » posé à la main ; une mise à jour qui rende les GT 1030 (et AMD,
« toutes les cartes ») compatibles. Il dort ; demain matin : mise à jour,
redémarrage, « Préparer le matériel », redémarrage, Rocket League à 60 i/s.

- **temprep** [écrit, essayé ici puis sur SON PC] : `/usr/lib/grenos/temprep`,
  `grenget temprep ouvrir|envoyer|poser|prendre|trouver|fermer`. Réseau local,
  TLS 1.3 à clé pré-partagée tirée du code (scrypt), faux PC à certificat
  refusé, tout affiché sur le PC aidé, Ctrl+C / `fermer` / inactivité.
  Livré ce soir par une page temporaire du site (/tempcorr, RETIRÉE à 22 h 20).
- **Ce qu'avait le PC à GT 1030** [vu par temprep] : à 18 h 20 le bouton avait
  posé 580-OPEN (« not supported by open nvidia.ko… GSP ») ; à 19 h 00, posé à
  la main, `nvidia-driver-580` FERMÉ + une règle apt à la main. Le fermé
  tournait (nvidia-smi : GT 1030) MAIS X était resté sur l'image du firmware
  (10-grenos-image-firmware.conf, écrit au démarrage de 18 h 31, AccelMethod
  none) : llvmpipe, d'où « 16 fps sur Rocket League ». Un seul noyau : Ubuntu
  7.0.0-44 (Debian retiré à 18 h 29). Secure Boot désactivé sur ce PC.
- **Retiré avec son accord** (« tout retirer ») [fait, vérifié] : 21 paquets
  nvidia/libnvidia/linux-modules-nvidia (open ET fermé) purgés, règle à la
  main supprimée ; noyau, initramfs (22 h 16), menu et EFI intacts, dpkg
  propre ; marque materiel-prepare effacée (le raccourci revient). PAS
  redémarré. Le dépôt d'Ubuntu (nvidia-driver-580-open.list) reste : le
  bouton le remplacera par celui de la variante fermée.
- **La variante suit la carte** [écrit ; essai-cartes-graphiques.sh vert ici
  contre les 449 identifiants des Modaliases d'Ubuntu] : grenos-nvidia-variante
  (1340–1dff → 580 fermé ; 1e00+ → 580-open ; avant → aucun, pilote libre),
  depots/nvidia-driver-580.*, grenos-pilote-signe preparer <paquet>, Préparer
  le matériel / assistant / Jeux. Le module fermé vit dans
  kernel/nvidia-580/nvidia.ko, signé « Canonical Ltd. Kernel Module Signing »
  [vu sur SON PC]. AMD/Intel : déjà tout dans l'image (vérifié par l'essai).
- **CI** : la branche d'avant était rouge (panneau du son : la barre
  n'annonçait sa position qu'une fois, le dock s'élargit → clic sur « ^ » ;
  UEFI et maj-au-démarrage tombaient derrière, QEMU resté vivant). Corrigé
  (barre.rs : redit les positions qui changent). Publication par la branche
  `claude/maj-gt1030` (SANS fichier de workflow : le jeton de ce PC n'a pas
  le droit `workflow`). Restent sur `claude/temprep-cartes-graphiques` : le
  commit natif.yml de l'adaptateur Xbox (ae07578) et les ajouts de
  linux.yml / pilote-signe.yml (essai des cartes, matrice des variantes) —
  à pousser après `gh auth refresh -h github.com -s workflow`.
- [à faire] 1. branche verte → main → dépôt APT publié ; 2. demain sur SON
  PC : la vraie preuve (glxinfo : NVIDIA, Rocket League) ; 3. pousser les
  workflows ; 4. rien n'efface 10-grenos-image-firmware.conf quand un pilote
  arrive SANS redémarrage (le bouton demande de redémarrer, donc sans effet
  aujourd'hui).

## 0000000. PUBLIÉ — 23 novembre soir : NVIDIA en un bouton, Secure Boot gardé, sans écran bleu

- 20 h 33, sur SON PC (1803 installée) : « Préparer le matériel » →
  « grenos-pilote-signe: Permission denied ». Écrits sous Windows, ce script
  et grenos-pilote-refuse étaient 100644 dans Git ; l'essai faisait chmod.
  Corrigé, verif-shell refuse tout script 100644, l'essai garde les droits
  de Git [prouvé : Pilote signe 45250830111]. **PUBLIÉ : linux-20261123-2027**
  (main d1916c4, construction 45250825746 verte), par mise à jour depuis grenOS.

- **linux-20261123-1803** (main 25da8cb, construction 45224717254 verte).
  Sa clé F: la porte [prouvé : 292 fichiers relus, 0 faute, mode « firmware »
  et rapports gardés]. Le Samsung : vide le 23 à 12 h 59 avec son accord,
  puis apparemment réinstallé en 1237 vers 16 h 35 ; il veut tout réinstaller.
- « Préparer le matériel » / assistant / Jeux → `nvidia-driver-580-open` :
  `grenos-pilote-signe` pose le noyau d'Ubuntu 26.04 et son pilote 580
  signé par Canonical, puis le shim et le GRUB d'Ubuntu DANS EFI/debian
  (fichiers signés de Debian détournés, dpkg-divert) ; noyau de Debian retiré
  au démarrage suivant ; `suivre` après chaque mise à jour.
  [prouvé : Pilote signe 45223329963 — 1237 installée, bouton joué, deux
  démarrages Secure Boot (clés Microsoft) : noyau 7.0.0-44-generic,
  « NVRM: No NVIDIA GPU found », MokNew vide, aucune ligne « prohibited »,
  écran de grenOS (assistant) ouvert, noyau Debian retiré].
- GRUB d'Ubuntu : n'a que efi_gop → `01_grenos-video-signe` redéfinit
  load_video (sinon « prohibited by secure boot policy » + « Press any key »).
- [à faire] 1. sur SON PC : RTX 5060 sous 580, deux écrans, 300 Hz ;
  2. l'essai n'est allé que jusqu'à l'assistant, pas au bureau ;
  3. rien ne retire les vieux noyaux d'Ubuntu (suivre en ajoute).

## (avant) À DÉCIDER — 23 novembre soir : NVIDIA sans écran bleu, Secure Boot gardé

Grenofar : « garde secure boot et fais tout », « ça fonctionne sur Windows ».
Windows n'a pas d'écran bleu parce que Microsoft signe le pilote. DKMS signe
avec une clé à nous : MokManager est inévitable (présence physique exigée).

- PROUVÉ (Pilote signe, run 45211185451, branche nvidia/signe) : un Debian
  trixie démarré par la chaîne d'Ubuntu 26.04 (shim signé Microsoft, GRUB et
  noyau 7.0.0-44 signés Canonical), QEMU en Secure Boot avec les clés de
  Microsoft, aucune clé en attente : un module sans signature est refusé
  (« Key was rejected by service ») et `nvidia.ko` 580-open de Canonical
  (« Canonical Ltd. Kernel Module Signing ») se charge : « NVRM: No NVIDIA
  GPU found. » (pas de carte dans QEMU). Aucun écran bleu.
- Ce que ça demande [à faire, plusieurs jours] : sur grenOS INSTALLÉ, la
  chaîne de démarrage et le noyau d'Ubuntu (dépôt resolute épinglé aux seuls
  paquets du noyau, shim/GRUB posés sur l'EFI par nous), le pilote
  linux-modules-nvidia-580-open-<noyau> et ses bibliothèques à la même
  version, posés par l'installateur quand une NVIDIA est là ; les mises à
  jour du noyau tirent le module signé du même noyau. Preuve en CI avant
  tout ; la vraie preuve sur la RTX 5060 (580 la connaît).

## 000000. EN COURS — 23 novembre midi : grenOS ne démarrait plus (pilote NVIDIA)

Grenofar, depuis Windows : « grenos veut plus démarrer à cause d'un driver ».
À l'écran : MokManager, logo grenOS, trois lignes, puis plus rien. Il a fait
« Continue boot » à l'écran bleu, sans le code (« j'ai pas le code »).

- Cause [déduite, journaux à lire] : `nvidia-kernel-open-dkms` 620 posé à la
  main le matin, signé par la clé de grenOS EN ATTENTE ; écran bleu passé,
  clé pas inscrite, le Secure Boot refuse le module, `nouveau` est écarté
  par le paquet NVIDIA. Contraire à la RÈGLE N°1 : un écran bleu à passer
  sans faute, sinon plus de bureau.
- Son Samsung [écrit, posé et relu depuis Windows, pas encore démarré] :
  sur la partition EFI, `grenos-reparation/` + un `EFI/debian/grub.cfg` qui
  démarre UNE fois avec `systemd.run=` (deux essais au plus, comptés dans un
  grubenv) : journaux des trois démarrages d'avant copiés sur l'EFI, demande
  MokManager retirée, paquets nvidia/cuda purgés (firmwares gardés, refus si
  apt emportait grenOS/X/noyau), dépôt NVIDIA retiré, grub.cfg remis, reboot.
  Puis `grenos-apres-reparation` écrit `apres.log` et le journal du démarrage
  normal sur l'EFI (lisibles depuis Windows) et se retire. Sources :
  %USERPROFILE%\grenos-reparation.
- Pour tous [écrit, essayé avec de faux outils, 3 cas] :
  `grenos-pilote-refuse.service`, avant l'affichage : Secure Boot + module
  nvidia de DKMS + clé non inscrite + module pas chargé → purge, demande
  MokManager retirée, redémarrage sur l'écran d'avant ; la session le dit
  (« Pilote NVIDIA retiré ») et remet « Préparer le matériel ».
- [à faire] 1. lire `reparation.log`, `journal-1.log`, `apres.log` sur l'EFI
  du Samsung : la vraie raison de l'écran noir ; 2. publication de la garde
  en CI ; 3. un pilote NVIDIA sous Secure Boot SANS écran bleu n'existe pas
  avec DKMS : décider avec Grenofar (le geste reste, mais il ne casse plus).

## 00000. FAIT le 23 novembre au matin — première séance DEPUIS grenOS

Grenofar travaille désormais sur grenOS installé (« je ne vais plus revenir
sur Windows ») : RTX 5060 (GB206, 10de:2d05), Secure Boot actif, deux écrans.
Demandes, dans l'ordre : 2e écran noir ; glisser une app vers bureau/dock
« carré noir sans plus » ; essayer le MCP d'écran ; « Préparer le matériel »
après l'installation ; curseur de Claude séparé ; gestionnaire des tâches
(Processus, Performance, Jeu, Démarrage, « à part les process de grenOS ») ;
capture « copier seulement » ; documentation sur grenos.vercel.app ; tout
dans TASK.md ; publier (mise à jour auto). Il est parti, séance autonome.

- **2e écran noir** [diagnostiqué] : `nouveau` écarté (`choix garde (ecran
  juge illisible ici)` au démarrage), X sur fbdev/efifb = UNE sortie. Grenofar
  a ensuite posé LUI-MÊME le pilote officiel (les 3 commandes manuelles, 07:25)
  : `nvidia-kernel-open-dkms` 620.70.06 compilé et signé pour 7.3.4, clé
  grenOS EN ATTENTE d'inscription (`mokutil --list-new`). [à voir sur son PC]
  au redémarrage : écran bleu MokManager (code donné dans son terminal), puis
  deux écrans. `affichage-reessai` est aussi armé (Réglages, 07:19) : sans
  effet si `nvidia` prend la carte (nouveau en liste noire par son paquet).
- **MCP grenos** [écrit, essayé] : Claude Desktop lance ses serveurs MCP avec
  HOME/LOGNAME/PATH/SHELL/USER SEULEMENT → « pas d'écran X ». `grenos-mcp`
  reprend DISPLAY, XAUTHORITY, bus… de `systemctl --user show-environment`
  (repli : /tmp/.X11-unix + ~/.Xauthority). Essayé avec l'environnement réduit.
- **Curseur de Claude** [écrit, vu à l'écran] : `grenos-mcp-curseur` (fenêtre
  POPUP découpée SHAPE, forme d'entrée VIDE = clics traversants) dessine la
  flèche du thème de curseurs de la personne (Breeze_Light ici) repeinte en
  orange #D97757 + étiquette « Claude ». Clic/glisser/défiler : pointeur de la
  personne CACHÉ (XFixes), posé (XTest), geste, RENDU à sa place — vérifié au
  pixel près pendant que Grenofar bougeait sa souris. `souris_deplacer` ne
  bouge plus que le curseur orange (`survol=true` pour un menu au survol).
  Inspiré de claude-free-plan-mcp-powershell (même protocole JSON par ligne).
- **Glisser menu → bureau/dock** [écrit, vu] : au début du glisser, le menu
  relâchait SON attrape (`siege.ungrab()`), ce qui coupait celle de GTK
  (une seule attrape par client X) → icône plantée. `Attrape::ceder`. Le dock
  accepte maintenant le dépôt (épingle), l'icône suivie est celle de l'app.
- **Bulle d'aperçu bloquée au-dessus du dock** [écrit, vu] : `close()` est
  ignoré par GTK tant qu'une attrape GTK existe (menu contextuel, glisser) ;
  la bulle était oubliée mais restait. `hide()` + `destroy()`.
- **Clic dans le dock** : Grenofar « en fait ça fonctionne » — rien changé.
- **Capture « copier seulement »** [écrit, essayé] : `xfce4-screenshooter -c`
  perd l'image à sa sortie (pas de gestionnaire de presse-papier).
  `grenos-capture zone|ecran|fenetre` garde l'image jusqu'à la copie suivante.
  Meta+Maj+S (zone), Ctrl+Impr (écran), Ctrl+Alt+Impr (fenêtre).
- **Gestionnaire des tâches** [écrit, vu] : 4 onglets sur les mesures de
  grenPerf. Processus regroupés, TOUT sauf grenOS (PROTEGES élargi, scripts
  Python nommés par leur script) ; arrêter un groupe système = UNE demande
  pkexec. Performance : courbes 2 min. Démarrage : couper / retirer /
  « Ajouter une application » (copie de la fiche dans ~/.config/autostart),
  fiches de grenOS masquées. Jeu : libérer la RAM (mesuré) + mode jeu.
- **Préparer le matériel** [écrit, vu, PAS cliqué] : `grenos-ui preparer`
  + `grenos-preparer-materiel` (session) qui pose le raccourci du bureau sur
  un grenOS installé dont une NVIDIA n'a pas `nvidia`, et le retire ensuite.
  Installe par `installer` (dépôt NVIDIA, DKMS, `--apres`), affiche le code
  MokManager EN GRAND et les étapes, bouton Redémarrer.
- **Code MokManager faux au 2e passage** [écrit, essayé avec un faux mokutil] :
  `grenos-depot-editeur --apres` ne reconnaissait que « already enrolled ».
  Clé déjà EN ATTENTE → `mokutil --import` fait « SKIP » et garde l'ancien
  code, mais un code neuf était affiché. Il retire la demande
  (`--revoke-import`) et en pose une neuve. Pas exécuté sur le PC de Grenofar
  (sa demande en attente garde le code de son terminal).
- **CI rouge run 45129195984** (« taches ouvert » jamais venu) : le nouveau
  gestionnaire lisait grenPerf et n'appelait plus `taches_mesure`, qui dit
  cette preuve. Rappelé une fois à l'ouverture [écrit]. Le dépôt APT de ce
  run était PUBLIÉ (étape 34 verte) ; seule la Release a attendu.
- **Documentation** [écrit] : grenos-web-download `/documentation` (FR/EN),
  lien en-tête/pied/Télécharger, sitemap. typecheck et build verts.
- Vérifié en local : natif 34 essais, grenos-ui 76, verif-accents/fenetres/
  shell/recherches et les essais de linux.yml verts SAUF essai-installer.sh,
  qui échoue aussi sans ces changements (il lit les vrais disques de ce PC).
- Outils posés sur ce PC pour compiler ici : cargo/rustc-web 1.96 (Debian,
  Tauri exige ≥ 1.88), libwebkit2gtk-4.1-dev, nodejs 20, git.
- **PUBLIÉ** : `linux-20261123-0847` (run 45135686165, commit 2525f6d) —
  paquet, dépôt APT et Release verts, les cinq preuves de démarrage vertes.
- [à faire] 1. (fait : CI verte, voir ci-dessus) ;
  2. sur son PC : redémarrage, MokManager, deux écrans ; relancer Claude
  Desktop pour charger le nouveau grenos-mcp ; 3. MCP : un `souris_glisser`
  rend la main au pointeur de la personne pendant ~0,6 s — si elle bouge, le
  glisser bouge (dit dans la doc) ; 4. la page Préparer n'a pas d'essai
  automatique de l'installation (pas de carte NVIDIA en CI).

## 0000. EN COURS — 22 novembre au soir : le disque installé démarre droit

Grenofar, 19 h, grenOS installé sur son Samsung : « dans le BIOS […] on voit
grenOS et pas debian », le menu (« enlève »), « l'OOBE apparaît pendant 3 sec
[…] un chargement de grenOS qui ne se termine pas », « l'OOBE seulement dans
grenOS de base installé ».

- DEUX pannes, deux gestes manquants avant X [prouvé : 45053914956, puis
  45067830100 ; sources de Plymouth, LightDM et du noyau] :
  1) Plymouth vivant rappelle le terminal 1 à 10 s, par-dessus le lobby ;
  2) Plymouth arrêté avant d'avoir rien peint laisse le terminal en
  KD_GRAPHICS + VT_AUTO, où le noyau IGNORE tout changement : X attend sans
  fin. Attendre sa mort ne suffisait donc PAS (45067830100, rouge).
  `grenos-ecran-libre` attend sa mort PUIS rend le terminal en texte, avant
  le lobby et lightdm [écrit].
- Sur le disque de la 1719, SANS rien changer, Ctrl+Alt+F7 ramène le lobby,
  puis ouvre le bureau au démarrage suivant [prouvé dans QEMU : 45068030925 ;
  à voir sur son PC]. C'est le chemin de la réparation par mise à jour.
- Lobby seulement sur le grenOS installé ; la clé ouvre son bureau, Bienvenue
  propose d'installer (plus jamais sur une machine installée) [écrit]. La
  machine VirtualBox publiée est une image live : elle n'a plus de lobby.
- BIOS : « grenOS » (`grenos-demarrage`, au démarrage et à la pose de GRUB).
  Le dossier reste EFI/debian (GRUB signé). grub-install en --no-nvram : il
  effaçait les « debian » des autres disques et copiait fbx64 (écran bleu au
  chemin de secours) [écrit]. Une ligne « UEFI OS » (chemin de secours, nommée
  par la carte mère) peut rester à côté [à voir sur son PC].
- Ni menu ni texte de GRUB, ligne du noyau de la clé [écrit]. Windows : Fin
  de session › « Redémarrer sous Windows » (BootNext, une fois) [écrit].
- « Une erreur EVO » : très probablement « error: prohibited by secure boot
  policy. », à CHAQUE démarrage installé en Secure Boot [prouvé : séries de
  45053914956 et 45067830100] : `insmod bli` (25_bli de GRUB 2.12), absent de
  l'image signée. 25_bli perd son bit d'exécution ; la ligne du noyau de la
  clé fait taire le reste [écrit]. Si elle revient : lui demander une photo.
- Antivirus (demandé le 22 au soir) : Debian arme TROIS unités freshclam, on
  n'en coupait qu'une ; 40 s de retard sans réseau, des centaines de Mo avec
  [prouvé : journal de 45053914956]. Les trois coupées, garde à la
  construction, essai-antivirus 10 cas [écrit].
- PROUVÉ, cle.yml 45072143683 (9 essais verts, commit d7ae3bd), sans pilote
  d'écran et en Secure Boot, sur un disque neuf ET sur la 1719 mise à jour
  par nos paquets (avec mot de passe) : aucun menu ni texte à l'écran
  (captures regardées), lobby 25 s à l'écran puis fini, « grenOS » au
  micrologiciel et plus de « debian », bureau aux deux démarrages, aucune
  erreur de Secure Boot, antivirus muet, démarrage complet en 3 s sans
  réseau, « Redémarrer sous Windows » à la souris : le micrologiciel essaie
  Windows une fois, puis rouvre grenOS. Et la fenêtre d'installation entière
  (clé sans lobby, 12 écrans sur le disque, bureau).
- [à faire, dans l'ordre, accord de Grenofar du 22 à 22 h 14 : « publie si
  tout est vert », Samsung effacé une fois la clé prête] 1. construction
  complète verte (45072143479) ; 2. publier ; 3. image publiée jugée nue ;
  4. clé F: écrite et relue ; 5. réveil de 6 h 30 (tâche « grenOS - reveil
  6h30 », %LOCALAPPDATA%\grenos-reveil) : il ne sonne que si la clé est prête
  (cle-prete.txt), et ouvre alors l'autorisation Windows pour effacer le
  Samsung ; 6. vu sur son PC : BIOS, lobby, bureau, Windows par Fin de
  session, « UEFI OS » éventuel à côté de « grenOS ».

## 000. EN COURS — 22 novembre : NOTRE installateur, toutes ses pages

Grenofar : « refais le système de partition, des fois c'est pas cliquable
[…] avec supprimer une partition et créer pour grenOS, et taille Gio
changeable », puis « l'interface, mets-la pour toutes les pages ».

- Samsung 860 EVO vidé depuis Windows, avec son accord pour CET effacement
  seulement [prouvé : relu depuis Windows, aucune partition, C: et D: intacts].
- « Installer grenOS » ouvre notre installateur (grenos-ui, page
  « installation », 7 pages) ; moteur root `usr/lib/grenos/grenos-installation`.
  Les disques sont lus une fois, chaque clic n'est qu'un calcul [prouvé
  sous WebKitGTK en CI, voir plus bas ; jamais vu sur son PC].
- Moteur : tout est vérifié avant la première écriture ; la clé et le disque
  du système ne sont jamais proposés [prouvé en CI ; essai-installation-moteur.py
  40 cas, essai-installation-ecran.py 20000 tirages] ; de bout en bout sur
  disques virtuels [prouvé : cle.yml 45014857052 — le disque démarre seul en
  Secure Boot jusqu'à l'assistant ; faux Windows et partition gardée intacts].
- Un double-clic s'annulait (« Supprimer » redevenait « Garder ») et, au
  compte, lançait l'installation ; Entrée au résumé aussi. Reproduit dans
  l'aperçu avec de vrais gestes, corrigé, rejoué [prouvé : 45019134134].
- L'assistant du disque installé ne redemande ni langue, ni clavier, ni
  compte [prouvé : 45019134134] ; n'efface pas le mot de passe [écrit ;
  essais Rust, jamais démarré jusqu'à la connexion avec un mot de passe].
- Calamares reste en secours (`grenos-installer --secours`) ET pour les PC
  démarrés en BIOS : notre moteur n'installe qu'en UEFI. Le recul a été vu
  sur la capture de l'image complète (45020774130), avant publication
  [prouvé : sa fenêtre exigée et vue en BIOS, 45039267660 et 45043924806].
  [à faire] installer en BIOS avec notre installateur.
- Par la vraie fenêtre [prouvé : cle.yml 45016476457 — sept pages sous
  WebKitGTK (captures regardées), installé, Alt+F4 sans effet en pleine copie,
  redémarré sur le disque avec la clé branchée et en tête de l'ordre, 12
  écrans d'assistant au lieu de 15, bureau ouvert, Secure Boot ; sans réseau].
  `bootindex` de QEMU faussait le redémarrage (OVMF efface l'entrée du disque).
- À LA SOURIS, sur le scénario demandé [prouvé : cle.yml 45019134134 —
  supprimer, créer, 25 Gio, double-clics, table exacte ; les deux partitions
  gardées et un faux Windows (EFI + NTFS) identiques à l'octet près ; accueil
  du disque installé en français].
- GRAVE, d'avant l'installateur, trouvé par cet essai : la clé montait en
  écriture les partitions des disques du PC à chaque démarrage (live-boot,
  `persistence` cherche aussi des fichiers). [prouvé] sans le correctif
  (45019065101) les partitions Linux changent, le faux Windows non ; avec
  `persistence-storage=filesystem` (45019134134), rien ne change. La clé
  publiée 2146 a encore le défaut. La persistance elle-même : jugée par la
  prochaine construction (« persistance : active », retour au 2e démarrage).
- **PUBLIÉE : linux-20261122-1719** (main 55c5cb5, 15 commits, construction
  45043924806, paquets 1.0.202611221636), avec son accord du 22 au soir.
  [prouvé sur l'image publiée SEULE, sans couche du dépôt : cle.yml
  45049086586 — souris, 25 Gio, Windows au menu et intact, zéro montage en
  écriture au démarrage, disque démarré en Secure Boot jusqu'au bureau].
- **Sa clé F: porte la 1719** [prouvé : relue fichier par fichier contre
  sha256sum.txt ; mode simple et rapports identiques].
- [à faire, dans l'ordre] 1. vu sur son PC : RTX 5060, sa carte mère repart-
  elle sur le disque après l'installation ?, mot de passe jusqu'à l'écran de
  connexion, fenêtre polkit ; 2. lire le rapport que la clé aura écrit ;
  3. publier la barre cliquable et l'essai « image nue » (claude/suite, PAS
  publiés) : demander son accord, et passer d'abord une construction
  complète sur une branche (sur main, catalogue et dépôt APT partent AVANT
  les essais de l'image).
- CAUSE du « pas cliquable » [prouvé : rapport 20261122-094033 de sa clé, lu
  le 22 au soir, et ChoicePage.cpp de Calamares 3.3.14] : quatre clics sur la
  partition de 218 Gio dans la barre (« Clicked row 2 »), ignorés, car
  Calamares met la barre en NoSelection tant qu'aucun choix n'est fait
  au-dessus. Rien ne faisait attendre : les disques étaient lus en 1 s.
  Chez nous la barre ignorait aussi ces clics : elle surligne désormais la
  ligne cliquée [prouvé sous WebKitGTK : 45044025025, capture regardée ;
  PAS dans la 1719].
- L'écriture de la clé sur les disques [prouvé SUR SON PC] : même rapport,
  sdb1, sdb2 et sdb3 montées r/w à 18 s, avant le bureau.
- Place minimale ramenée à celle de Calamares : 15 Gio au lieu de 20,5
  [prouvé : installé sur 15 Gio et démarré en Secure Boot, 45040347252].
- Windows d'un autre disque dans le menu du disque installé (os-prober, menu
  5 s) [prouvé : 45040347252 et 45049086586, sur un faux Windows].
- Aussi en moins que Calamares : 16 claviers seulement, nom du PC fixe
  (« grenos »), un disque MBR ne se prend qu'en entier.
- Ce que Calamares offrait et que notre installateur n'a PAS : chiffrement
  du disque, partition d'échange (il reste zram, la mémoire compressée),
  choix btrfs / xfs, points de montage à la main. À dire avant de publier.
- [à faire] réduire Windows pour installer sur le même disque : Calamares le
  proposait, notre moteur ne le sait pas. L'écran le dit et ne conseille plus
  de supprimer ; sur un PC à un seul disque plein, c'est un manque (RÈGLE N°1).
- [à faire] « English » dans l'assistant de la clé est enregistré et jamais
  appliqué : une case sans effet. Pas reprise dans l'installateur.

## 00. EN COURS — 20 novembre au soir : « termine l'OS, une bêta, pas une pré-alpha »

Grenofar teste sur SON PC (RTX 5060, Secure Boot actif, clé USB FAT32 G:).
Version publiée : `linux-20261120-2059`. Ce qu'il a vu et où on en est :

1. **Écran noir après le chargement (2059)** — CAUSE PROUVÉE (cle.yml, run
   44823344458) : noyau 7.3.4 sans pilote DRM pour l'image du firmware
   (CONFIG_FB_EFI=y seulement), et `xserver-xorg-video-fbdev` absent →
   « no screens found ». Corrigé (paquet + `grenos-affichage-firmware`) et
   VU dans QEMU : l'assistant s'affiche sur la carte sans pilote.
2. **Écran vert** — vu AUSSI dans QEMU sur l'image du firmware : GRUB
   (`gfxmode=auto` + gfxterm) change le mode, le noyau peint en 1280×800 et
   l'écran affiche 800×600 → rayures vertes. Essai d'un config.cfg qui ne
   touche plus au mode (linux/correctif/cle/boot/grub/config.cfg) en cours.
   Sur `nouveau`, le vert est apparu avec Mesa 26.1 (glamor) : X tourne
   désormais sans glamor sur nouveau, jamais vu sur sa machine.
3. **RTX 50 : nouveau réessayé sous un filet** (grenos-affichage-garde) :
   premier écran de l'assistant non passé en 180 s, ou coupure au bouton
   → image du firmware et redémarrage tout seul. Choix gardé sur la clé.
4. **Rapport de démarrage écrit sur la clé** (grenos-rapports/) : le lire
   dès qu'il rebranche la clé sur Windows (G:\grenos-rapports).
5. **Correctif de clé** (linux/correctif, cle.yml) : une couche squashfs que
   live-boot empile ; se copie sur la clé en G:\live\grenos-correctif.squashfs
   sans regraver. Calamares la recopie aussi (unpackfs.conf du correctif).
6. Applications plus rapides : `grenos-ui --resident` + `grenos-prechauffer`
   [écrit] — dans la prochaine construction complète seulement.
7. MokManager retiré du parcours normal (écran Pilotes : Dev ou sans
   Secure Boot seulement) ; la phrase fausse « il faut désactiver le Secure
   Boot » corrigée dans les deux assistants.

### 21 novembre, nuit — retour de Grenofar sur la 2306 (son PC)
« Un peu mieux » : **300 Hz et curseur marchent** (nouveau, sans glamor,
sur sa RTX 5060). Restaient, et où ça en est :
- **L'installateur bloquait au partitionnement** (« on ne peut plus
  choisir ») : Calamares retire Effacer/Remplacer/À côté sur un disque dont
  une partition est MONTÉE (Windows par grenos-windows, son SSD ouvert dans
  « Ce PC »). `grenos-installer` referme tout avant `pkexec calamares`
  [écrit, 7 cas exercés — pas encore vu sur sa machine]. Le journal de
  Calamares va maintenant dans le rapport de la clé (60-installateur.txt).
- **Réglages › Écran** : choisir l'écran, sa référence (EDID), ses limites,
  plus aucune fréquence « à essayer », « Garder » et « Disposer » notent
  TOUS les écrans [écrit, vu dans l'aperçu].
- **Son** : « il n'a pas trouvé tous mes périphériques audio » — cause
  INCONNUE ; le rapport de la clé dit désormais tout du son (70-son.txt).
  À LIRE dès qu'il rebranche la clé sur Windows.
- **Créateur de clé USB** (`createur/`, Rust/Tauri, Windows) : liste les
  seules clés USB, télécharge et vérifie la dernière ISO, FAT32, copie,
  revérifie [écrit, vu dans l'aperçu ; createur.yml le prouve sur une clé
  virtuelle puis le publie dans la version « createur »]. Site : carte
  « Créateur de clé USB » (cachée tant que le .exe n'existe pas), et les
  phrases fausses « Secure Boot doit être désactivé » retirées.
  21 nov. : sur SA clé (« USB DISK 2.0 », média amovible), échec à
  « Préparer la clé » — `Initialize-Disk : The disk has already been
  initialized`. Windows laisse une clé amovible en MBR après Clear-Disk (vu :
  MBR, zéro partition) ; le disque virtuel de la CI est fixe et redevient RAW,
  d'où l'angle mort. Corrigé (style relu après l'effacement, dd8f76f) ; la CI
  fait maintenant deux passages, dont une clé déjà utilisée [prouvé sur
  disque virtuel : run 44929758992, RAW puis MBR, 292/292 fichiers
  identiques les deux fois ; .exe republié à 17:30Z]. Le chemin
  amovible lui-même [prouvé sur SA clé] : il l'a refaite avec le nouveau
  .exe (F:, FAT32 « GRENOS », 1551) et elle a démarré.
- **Écran vert de 18 h 50 (21 nov., 1551, clé neuve donc `nouveau` essayé)**.
  Ce que la clé et Windows disent :
  - le filet a marché : `grenos/affichage = firmware` écrit à 18:50:00 ;
    Windows redémarre à 18:51:00. MAIS le redémarrage est reparti sous
    WINDOWS (clé prise au menu F12 : un choix d'une fois) — il n'a vu que
    « écran vert puis Windows ». Réparé : `grenos-revenir-ici` (BootNext =
    BootCurrent sur reboot.target, jamais par-dessus un BootNext déjà posé)
    [écrit ; essai-revenir-ici.sh, 6 cas].
  - AUCUN rapport sur la clé : le filet redémarre ~35-40 s après le
    démarrage, avant le premier instantané ; celui de l'arrêt interrogeait
    X et l'écran figés et a été tué au bout de 45 s. Réparé : rapport en
    deux temps (premier temps sur la clé AVANT de parler à X), `borne` qui
    abandonne au lieu d'attendre (état D), lien PCIe dans le résumé
    [écrit ; essai-rapport-cle.sh, 6 cas, échoue sur l'ancien `borne`].
  - le système installé repartait de zéro (écran vert au premier
    démarrage) : le choix de la clé suit désormais sur le disque
    (calamares-bootloader-config) [prouvé en CI sur la 1908 : cle.yml run
    44939531360, « le choix d affichage de la cle a suivi sur le disque :
    firmware », installation jusqu'à l'assistant en Secure Boot].
  - Les trois réparations sont dans `linux-20261121-1908` (construction
    44934751490 verte, essais rapport-cle et revenir-ici passés en CI).
    Sur SA machine : [à voir] — retour sur la clé après le filet (dépend
    de sa carte mère), rapport du démarrage figé lisible sur la clé.
- **Installation 1908 ratée à 20 h 32 (21 nov.)** — le journal de Calamares
  était sur la clé (rapport « arret », 60-installateur.txt) [prouvé, lu] :
  - NOS réglages étaient bien lus (`grenos-grub-install … --bootloader-id=
    debian`) ; l'erreur : `failed to make directory '/boot/efi/EFI/debian'`
    — /boot/efi était la CLÉ (sdd1, montée en lecture seule).
  - pourquoi : un SECOND clic sur « Installer grenOS » (pkexec 4156 à
    72,1 s) ; son Calamares s'est refermé (déjà ouvert) et son lanceur a
    RENDU la clé à 72,2 s — avant que le premier Calamares ne lise les
    disques (72,4 s). Clé dans la liste, « Installer à côté » choisi sur le
    Samsung (sans partition EFI), Calamares a pris la clé (MBR, partition
    « active » = « boot » pour kpmcore) comme partition EFI.
  - réparé [écrit ; essai-installer.sh 16 cas, échoue sur l'ancien lanceur
    au second clic] : verrou (mkdir atomique) — un second lancement s'en va
    sans rien toucher ; et la clé n'est jamais rendue tant qu'un calamares
    tourne.
  - fuseau : aucun locale.conf chez Debian → Calamares présélectionnait
    New York et passait la session de la clé à l'heure de New York (photo :
    « 14:33 » à 20 h 33). Réparé : locale.conf (Europe/Paris,
    adjustLiveTimezone: false) [écrit, gardé au build et dans
    essai-installation.sh].
  - horloge : Windows retarde encore d'une heure après la session de la
    clé. Le noyau lit l'horloge en UTC, aucun « RTC configured in local
    time » de systemd (ni sur la clé, ni sur le disque installé en CI), puis
    timesyncd recale et le noyau réécrit en UTC. L'image est censée dire
    LOCAL (garde du hook 0100). [à faire] : le prochain rapport dit ce que
    le système croit en marche ; essai-installation.sh affiche l'adjtime de
    l'image.
  - écarté : ne plus marquer la partition de la clé « active » dans le
    Créateur — la clé démarre ainsi chez lui ; changer sans essai sur sa
    carte mère risquerait une clé qui ne démarre plus.
- **2037 (21 nov., 21 h 40)** : construction 44943231283 et cle.yml
  44947459584 verts (installation jusqu'à l'assistant, Paris, choix
  d'affichage suivi). Sa clé F: porte la 2037 (292 fichiers, mode simple).
  Faits ensuite (le soir même) :
  - `/etc/adjtime` disait **UTC dans l'image** : CAUSE TROUVÉE — le crochet
    `8000-remove-adjtime-configuration` de live-build (« Reset generated
    file ») l'écrit en UTC, APRÈS nos crochets 0100/0900 (codesearch.debian
    .net). `lb config` ne pose le lien vers le sien que si aucun fichier de
    ce nom n'existe : le nôtre, même nom, pose LOCAL ; `9999-grenos-dernier-
    mot` refuse l'image si ce n'est pas LOCAL à la toute fin ; second témoin
    (horloge-locale-2) pour remettre LOCAL une fois sur les machines déjà
    installées ; essai-installation.sh juge désormais l'horloge [prouvé
    sur l'image 2146 : cle.yml 44953502640, « /etc/adjtime dit LOCAL,
    temoins horloge-locale horloge-locale-2 » ; pas encore vu sur son PC].
  - après une INSTALLATION réussie, « Redémarrer maintenant » ramenait sur la
    CLÉ (BootNext = clé). Désormais, si le journal de Calamares dit
    « completion: succeeded », BootNext vise la première entrée « debian »
    de l'ordre (celle que grub-install vient de poser) ; installation ratée :
    retour sur la clé, pour réessayer [écrit ; essai-revenir-ici.sh 9 cas,
    l'ancienne version échoue sur les deux cas d'après installation].
  - Sa clé F: porte la 2146 (292 fichiers, mode simple), le 21 à 22 h 55.
  - CAUSE PROBABLE de l'écran vert, NON PROUVÉE : le lien PCIe de la RTX
    5060 (Gen5 x8, port 00:01.0 8086:A70D, BIOS F4 du 27/09/2024). Sous
    Windows : 12 441 « WHEA-Logger 17 » (erreur PCIe corrigée) en 3 min,
    ~70/s. Sous Linux, à CHAQUE démarrage, même en mode simple : RxErr ; et
    sous nouveau, « Replay Timer Timeout » côté carte dès 25,3 s, premier
    `wndw-0: timeout` à 27,3 s — AVANT X (30,2 s) : ni Mesa ni l'interface
    n'y sont pour rien (rapport 093509). Le mode simple marche malgré les
    erreurs (la carte n'émet presque rien). grenOS ne peut PAS brider le
    lien en Gen4 : verrouillage `integrity` (Secure Boot) → `setpci` refusé,
    et Debian n'active pas CONFIG_PCIE_THERMAL (bwctrl). Pistes [à faire] :
    lire l'ASPM du lien dans le prochain rapport ; Gen4 au BIOS (lui seul,
    s'il le veut — RÈGLE N°1 : grenOS ne l'exige pas, le mode simple tient).
- Correctif de clé (delta depuis la 2306) prêt : à copier en
  G:\live\grenos-correctif.squashfs quand la clé est branchée.
- Construction complète relancée : run 44839968086 (la précédente a été
  annulée par un push qui touchait linux/ — voir la mémoire
  « pousser-main-plage »).

### 21 novembre, 1 h — LU dans le rapport de sa clé (20261121-002001)
Sa machine : Gigabyte B760M G P WIFI DDR4, i5-14400F, 16 Go, RTX 5060
(nouveau OK, 2 écrans DP 1920×1080, écran principal « SKG 2509 »/H25Y7),
casque USB « OW810 » (3206:0861, sans nom de produit), adaptateur Xbox
(pris pour une carte Wi-Fi par mt76x2u), Bluetooth Realtek 8851BU (marche),
carte mère ALC897 (marche).
- L'assistant ne s'affichait JAMAIS sous nouveau : « drmSetMaster failed »
  (plymouth tenait la carte). `plymouth deactivate`, puis `quit
  --retain-splash`, essayés dans grenos-premier-x : X figé sur TOUTES les
  machines d'essai (runs 44842167737, 44843240153) → RETIRÉS. OUVERT.
  PISTE lue dans lightdm 1.32.0 (seat-local.c, get_vt) : plymouth sur un
  terminal sous minimum-vt (7) — notre cas, tty1 — n'est NI désactivé NI
  quitté « retain-splash » : lightdm lance `plymouth quit` tout court, en
  ATTENDANT qu'il finisse, puis X sur vt7 -novtswitch, et active lui-même
  le terminal (`vt_set_active`) une fois X prêt. À essayer dans
  grenos-premier-x par cle.yml AVANT toute version.
- Le filet a REDÉMARRÉ sa session qui marchait (il n'attendait que
  l'assistant) → un bureau ouvert vaut confirmation, plus jamais de
  redémarrage sous les yeux. Le choix de sa clé remis à « nouveau ».
- Casque introuvable = nom illisible → règle WirePlumber « Casque OW810 ».
  Le son de l'écran H25Y7 (DP) : À VÉRIFIER dans 70-son.txt.
- Erreurs PCIe « corrigibles » en rafale sur le lien de la RTX (356 en
  210 s) : elles ont noyé le début de dmesg. Piste : ASPM du lien de la
  carte (udev, par carte) — pas encore fait, à mesurer d'abord.

### 21 novembre, 2 h — deux pannes trouvées en préparant sa version
- **L'initrd gardait les pilotes réseau** (deux constructions refusées par
  la garde, 82 Mo, runs 44835521238 et 44844273146) : le crochet de
  live-boot les inscrit par DOSSIER (`auto_add_modules net` →
  « =drivers/net/ethernet »), le filtre par nom les laissait passer, et
  mkinitramfs les recopiait APRÈS les crochets, Netronome (35 Mo) et
  LiquidIO compris. Corrigé dans zz-grenos-sans-pilotes-graphiques
  [prouvé par la construction : « initrd 64 Mo, sans pilote graphique ni
  carte reseau », runs 44847898227 et 44853254724].
- **Windows retardait d'une heure après une session grenOS** (vu sur son
  PC) : Debian écrit l'heure en UTC dans la carte mère, Windows la lit à
  l'heure locale — et l'installateur faisait pareil (étape hwclock de
  Calamares). /etc/adjtime à LOCAL (appliquer-systeme, une fois par
  machine), étape hwclock retirée, fuseau de l'image à Paris [écrit ; la
  garde de construction passe, pas encore vu sur sa machine]. Son horloge
  Windows reste fausse jusqu'à sa prochaine synchronisation.
- **21 novembre au matin, la 0248 sur son PC : ÉCRAN VERT** (rapport
  20261121-093509). `nouveau` écrit « drm: wndw-0/2: timeout » toutes les
  2 s dès l'écran de chargement (plymouth tient les deux écrans), X et
  l'assistant démarrent dans le vide ; erreurs PCIe 5× plus fréquentes que
  la veille (8 300 en 47 s, lien Gen5 x8 sur un port PEG Raptor Lake).
  Même noyau, même carte que le démarrage de la veille qui marchait : le
  blocage va et vient. Clé passée à « firmware » (G:\grenos\affichage). Le
  filet guette désormais ces « timeout » à CHAQUE démarrage sous nouveau
  d'une RTX 50 (3 → image du firmware + redémarrage), et un bureau ouvert ne
  vaut plus preuve tant qu'ils existent [écrit, détection essayée sur ses
  deux rapports réels : 10 → bloqué, 0 → sain]. PISTE à mesurer : brider
  le lien à Gen4 avant nouveau (setpci, LnkCtl2) si les erreurs PCIe en
  sont la cause.
- **21 novembre, midi — la 0935 sur son PC, clé en « firmware »** (rapport
  20261121-104453) : l'assistant s'affiche (11 images/s, dessin au
  processeur), il le finit en 30 s — puis ÉCRAN NOIR. lightdm attendait
  sans fin que logind déclare seat0 graphique : systemd ne donne
  « master-of-seat » à /dev/fb0 qu'avec `nomodeset` (71-seat.rules). Règle
  udev 72-grenos-image-firmware.rules [PROUVÉ en QEMU : scénario
  firmware-bureau de cle.yml, run 44892873792 — bureau, barre et heure à
  110 s sur l'image du firmware]. « La souris dans l'OOBE » : X avait bien
  sa Corsair (libinput) ; le curseur est CACHÉ tant que rien ne bouge (les
  captures de 45 s n'en montrent aucun) et APPARAÎT dès que la souris bouge
  (capture de 75 s, même run). premier_pret le pose désormais au centre,
  visible d'emblée [écrit, compilé ; à voir dans la construction].
  « Des % au chargement » : vrai pourcentage dans « Mise en place… »
  (premier-etape, vu dans l'aperçu), barre sous la roulette au démarrage.
  Il passe sur grenOS seul dès la version suivante (construction
  44892885877).
- **21 novembre, après-midi — L'INSTALLATION, quatre échecs chez lui, puis
  l'essai qui va au bout.** Chaque tentative cassait une étape plus loin :
  1. « impossible de trouver unsquashfs » : squashfs-tools seulement
     recommandé par calamares → dans la liste ;
  2. (relu avant qu'il y arrive) packages.conf de Debian : un seul apt-get
     de neuf paquets, dont quatre absents, sans listes d'apt → try_remove ;
     bootloader-config de Debian : apt-get install grub-efi (aucun dépôt,
     aucun DNS dans le chroot) puis update-grub sans /boot/grub → le nôtre ;
  3. « grub-install … code 1 » : en « À côté », Calamares a pris LA CLÉ pour
     partition EFI (kpmcore : démarrable = ESP), montée en lecture seule →
     la clé passe en lecture seule (blockdev) le temps de l'installateur,
     kpmcore saute alors le disque ; grenos-grub-install (shim au chemin de
     secours, jamais sur celui de Windows), installEFIFallback coupé ;
  4. essai-installation.sh (cle.yml, job installation) : installe sur un
     disque SATA vierge avec les étapes et arguments de Calamares, puis
     démarre CE disque seul sous OVMF Secure Boot. Il a trouvé : GRUB signé
     à l'invite « grub> » (préfixe /EFI/debian gravé → efiBootloaderId
     « debian ») ; puis AUCUN service grenOS sur le disque installé —
     grenos-systeme dépendait de calamares : retirer l'installateur
     emportait grenos-systeme puis grenos-desktop, TOUT grenOS (make-deb
     exclut désormais la liste de packages.conf ; garde à la construction).
  [PROUVÉ en QEMU sur l'image publiée 20261121-1551 (cle.yml run
  44921626358) : disque vierge → installation → grenOS reste installé
  (grenos-systeme, grenos-desktop, 16 services) → démarrage SEUL sous OVMF
  Secure Boot, clés Microsoft → shim → GRUB → « Secure boot enabled » →
  l'ASSISTANT grenOS à 4,7 s, curseur au centre. Pas encore vu sur son PC.]
  La clé G: porte la 1551 (292 fichiers vérifiés), affichage-reessai gardé
  (il a cliqué « Réessayer » : la clé retentera nouveau, filet compris).
- Publiées : 20261121-0154 (l'initrd), puis 20261121-0248 (l'horloge). La
  clé G: porte la **0248** (292 fichiers vérifiés contre sha256sum.txt,
  affichage « nouveau » et rapports gardés) : prête pour installer sur son
  Samsung 860 EVO.

RESTE : GRUB du système INSTALLÉ vérifié en vrai (entrée Windows) ;
installation Secure Boot menée au bout (jamais prouvée) ; le son de l'écran ;
les erreurs PCIe.

## 0. En cours — le 18 novembre au soir (« termine tout, dans cet ordre »)

1. [FAIT] Les 11 fenêtres en grenos-ui : PROUVÉES en machine (construction
   44321910904) — Fin de session, Bienvenue, Réglages (10 pages et ses 4
   preuves), GrenPlace (233), Jeux, grenHub, Tâches, Son natif, Explorateur
   (Ctrl+H, recherche, Ctrl+Maj+N, F2, onglets).
2. [FAIT] Corrections K3 poussées ; la double construction tourne
   (reproductible.yml sur `main`). Premier passage : 5 différences, dont 3
   corrigées et 2 venant de la mesure (propriétaires lus par nom).
3. [EN COURS] `main` = claude/k3 (4cd7e91) : la construction publie la
   version si tout est vert. Ensuite le site (il lit la dernière Release).
4. [ÉCRIT, branche `claude/suite`] la suite de todo.md, à fusionner après
   la version 3 : NVMe/USB à chaud/veille prouvés en CI, B1, B5 (le
   chiffrement de l'installateur ne pouvait pas redémarrer), B6, C2-C9 et
   C12 (grenRecovery), E11 (fuzzing — a trouvé une vraie panique dans
   GrenLaunch), F3, F4, G3, G11, G23, G31, G32 (Agenda), H1/H3/H4/H6/H9/H11
   (GrenAI, OCR). Voir todo.md : chaque ligne dit [écrit] ou [prouvé].
5. RESTE vraiment : B3 (retour arrière atomique), D2, D9 (Secure Boot
   signé), E10 (charge sur plusieurs jours), G12, G22 et J3 (matériel absent
   de la machine d'essai), G28, H5, H8, H10, A9, A10.

## (historique) la publication de la version grenos-ui

- Branche `claude/grenos-ui` (grenOS) : grenos-ui en Rust/Tauri — premier
  démarrage, Mises à jour, grenPerf, GrenLaunch (126 jeux, « Mes jeux Steam »),
  auto-update corrigé (rien à chaud, Flatpak, choix de l'OOBE appliqué).
- CI `linux` : passes déjà vertes → compilation Rust, clé du dépôt, paquet,
  essai de mise à jour. Trois pannes corrigées en route (garde des recherches,
  verif-commandes qui lisait le binaire, hook qui exigeait l'install à chaud).
- **Quand la CI est verte sur la branche** (run 44156326449, commit `efa6efa`) :
  fusionner dans `main` CE commit vérifié, pas la pointe de branche :
  `git checkout main && git merge --no-ff efa6efa && git push`. Le commit
  `64904e0` (logo + fond, local, non poussé) part ensuite sur la branche pour
  sa propre CI, puis une version suivante.
  La CI de `main` publie la Release (ISO + zip VirtualBox) et le dépôt APT.
- **Puis le site** : dépôt `grenos-web-download`, branche locale
  `claude/theme-grenos-ui` (thème grenos-ui + section « Dans cette version »),
  à fusionner dans `main` et pousser — Vercel déploie. Seulement APRÈS la
  Release, pour ne pas annoncer une version introuvable.
- Supabase (projet `tpqzhzuoyqpfairatdrw`, org « kaka », plan gratuit) se met
  en pause après ~7 jours sans activité : « Restore » dans le tableau de bord,
  données conservées. Plafond de dépense activé : jamais facturé.

## FAIT le 18 novembre vers 12:30 — version `linux-20261118-1129` PUBLIÉE

Cinq preuves vertes (run 44285528012), site à jour.
- Fin du premier démarrage : la page reçoit le résultat par DEUX chemins
  (réponse IPC + événement « premier-fini ») ; les deux filets (12 s dans
  grenos-ui, 30 s dans le lanceur) restent et n'ont pas servi sur cette
  version. Si « filet » ou « on le ferme » apparaît un jour dans un journal
  série, la cause est ailleurs que dans la réponse IPC.
- I8 Ctrl+Alt+Échap → mode jeu ; I13 sauvegardes (grenos-sauvegarde) ;
  I17 écrans Dev (zram/swappiness, ufw, journaux) ; D1 ufw. Voir todo.md.

MIGRATION DES FENÊTRES PYTHON → grenos-ui (18 novembre, après 1129) — les 11
[écrites], branche `claude/ui-arret`, preuve en machine À VENIR (CI complète) :
- Arrêt, Bienvenue, Secours, Connexions, Tâches, grenHub, Jeux, GrenPlace,
  Réglages, Fichiers → pages grenos-ui (`ui/src/<page>.rs`, `web/<page>.js`).
  Chaque `/usr/bin/<programme>` Python fait `exec grenos-ui <page>` ;
  `--python` garde l'ancienne fenêtre en repli.
- Son → `grenos-son-natif` (gtk-rs, `linux/natif`) : un panneau qui attrape
  le pointeur se fait mieux en natif qu'en WebKit. `natif.passer_la_main(…,
  permanent=False)` : ses départs ne comptent pas comme des chutes.
- Les BIBLIOTHÈQUES Python restent (sante, jeux, magasin, ecran, heure,
  composants, securite, son, materiel, maj) : elles sont éprouvées par nos
  essais. `usr/lib/grenos/pont.py` les appelle en JSON pour grenos-ui, avec
  une liste blanche de fonctions.
- Socle grenos-ui ajouté : billets série dans la session (avant, les pages
  grenos-ui de la session ne disaient RIEN à la CI : pas de droit sur ttyS0),
  « ouverture : … » et « fenetre … : tient » pour chaque page (le contrôle
  720p), tailles ramenées à l'écran, `page:onglet` (`reglages:son`,
  `fichiers:/chemin`), plusieurs fenêtres pour l'explorateur, installateur
  commun (Flathub/Debian), liste blanche `ouvrir_programme`.
- Toutes les lignes série exigées sont reprises au mot près (« son ouvert »,
  « taches ouvert », « reglages ouvert », « reglages securite : », « reglages
  composants : », « reglages son : melangeur », « jeux ouvert », « grenplace
  ouvert », « fichiers : Ctrl+H … », F2, Ctrl+Maj+N, onglets…).
- Vérifié ici : 43 essais Rust, chaque page dans l'aperçu, la séquence
  clavier de la CI rejouée sur l'explorateur. PAS vérifié : sur une image.
  À lire dans le journal de la CI : les lignes ci-dessus, « (natif) » pour le
  son, « fenetre … : tient » pour les nouvelles pages.

RESTE, et pourquoi ce n'est pas fait :
- (FAIT, voir plus haut) les onze fenêtres Python : écrites, en attente de la
  preuve en machine (construction du 18 novembre, 38a8426).
- Connexion Google sur /grenget : il faut un identifiant OAuth Google créé
  par Grenofar (console Google Cloud, gratuit) — je ne crée pas de compte.
- K3 constructions reproductibles : un chantier de recherche (horodatages,
  ordre des fichiers dans squashfs), pas une correction.
- Le reste de todo.md (A, B, C, D, E, F) : la feuille de route d'un OS.

## FAIT le 18 novembre vers 10:00 — version `linux-20261118-0945` PUBLIÉE

Cinq preuves vertes (run 44270687371) : barre et bureau natifs vus dans les
quatre démarrages (« (native) », « (natif) »), grensh posé, catalogue 233 et
jeux 190 publiés, site en présentation + /grenget en ligne.
- RESTE : la course de fin du premier démarrage (réponse de premier_terminer
  perdue, vue 3 fois sur ~8 machines le 18). Deux filets la rattrapent et le
  DISENT (« fermeture par le filet », « on le ferme ») : chercher ces lignes
  dans les prochains journaux série pour trouver la cause.
- RESTE : connexion Google pour /grenget (annoncée « bientôt » en texte).
- Le workflow grenget.yml n'a pas encore vu de vraie proposition.

## (historique) le 18 novembre (matin) — séance autonome, Grenofar absent 5 h

Demandé : « publie, ajoute un max d'apps au launcher et à GrenPlace, customise
l'UI du cmd, crée le langage de grenOS comme PowerShell avec grenget, une page
grenos.vercel.app/grenget (proposer une app → en attente → vérifiée ; login
Google bientôt), refais le site en présentation avec Télécharger tout en haut ».

Branche de travail `natif/barre-bureau` (grenOS), poussée aussi sur
`claude/grenos-ui` pour la CI complète :
- [écrit] barre et bureau en Rust natif : `linux/natif` (gtk-rs + x11rb),
  binaires `/usr/lib/grenos/grenos-{shell,bureau}-natif`. `/usr/bin/grenos-shell`
  (Python) reste l'entrée et fait `exec` du natif (`natif.py`) ; 3 chutes en
  2 min → Python reprend, et le dit. `grenos-shell --python` force l'ancienne.
  Compile et 15 tests verts (workflow `natif.yml`, ~2 min).
- [écrit] terminal : xfconf `xfce4-terminal.xml` (palette nuit, Hack, sans
  barres), `grensh.sh` (invite 2 lignes, Verbe-Nom FR, complétion), `gren`,
  `grenget`. grenos-session pose la ligne dans ~/.bashrc une fois.
- [écrit] GrenPlace 121 → 233 (vérifiés sur Flathub), GrenLaunch 126 → 190.
- [écrit] propositions grenget : `.github/ISSUE_TEMPLATE/grenget.yml`,
  `.github/workflows/grenget.yml`, `scripts/grenget-verifier.py`. N'agit
  qu'une fois sur `main` (les workflows `issues` lisent la branche par défaut).
- Site : branche `claude/presentation` (grenos-web-download) — présentation en
  chapitres, en-tête avec Télécharger, page /grenget. À fusionner APRÈS la
  Release, comme d'habitude.
- Google : annoncé « bientôt » en texte, aucun bouton. Il faudra un client
  OAuth Google (gratuit) + Supabase Auth, ou NextAuth sur Vercel.

## FAIT le 18 novembre vers 05:00 — version `linux-20261118-0449` PUBLIÉE

claude/grenos-ui fusionnée dans main (5459886) : coins de fenêtre arrondis
(grenos-coins, x11rb SHAPE), trois ronds de couleur dans la barre de titre,
fondus d'ouverture/fermeture de grenos-ui. Cinq preuves vertes sur main
(run 44235178436), coins vus sur la capture ecran-perf de la CI.
ISO + VirtualBox en Release, dépôt APT et catalogue publiés,
grenos.vercel.app propose 20261118-0449.

## FAIT le 18 novembre vers 03:00 — version `linux-20261118-0233` PUBLIÉE

- Release GitHub (ISO 1,6 Go + zip VirtualBox), dépôt APT et catalogue publiés
  par la CI de `main` (run 44218497249, cinq preuves vertes).
- Site https://grenos.vercel.app en ligne avec le thème grenos-ui, la marque
  sans lettre et « Dans cette version ».
- PROCHAINE VERSION : la branche `claude/grenos-ui` (coins arrondis, ronds de
  couleur, fondus) — pousser, CI verte, fusionner dans `main`.

## (historique) 18 novembre — la publication

- Run 44191198865 : 4 preuves sur 5 vertes ; l'UEFI échouait parce que son
  essai coupait la machine à 90 s fixes, avant l'ouverture de la session.
  Corrigé (`8206a6b`) : il attend la preuve (240 s max) et écrit le temps mis.
- Branche `claude/grenos-ui-release` = la version vérifiée (`1d89c52`) + ce
  seul correctif : C'EST ELLE QU'ON FUSIONNE DANS `main` quand sa CI est
  verte (`git checkout main && git merge --no-ff claude/grenos-ui-release &&
  git push`), puis le site (`grenos-web-download`, branche
  `claude/theme-grenos-ui` → main).
- `claude/grenos-ui` porte en plus coins arrondis + fondus : sa CI tourne en
  parallèle ; elle fera la version suivante.

## 0bis. Résultat du run 44156326449 (fin vers 21:09) — À TRAITER EN PREMIER

- VERT : construction, démarrage BIOS (grenos-ui OOBE affiché en 1 s, 11 Entrée,
  « nom choisi, la machine est a ci@grenos », apparence obéie), mise à jour au
  démarrage, VirtualBox. grenos-ui marche sur la machine d'essai.
- ROUGE « sans matériel » : l'essai attendait l'écran du nom sans appuyer sur
  Entrée (le nom n'est plus le 1er écran). CORRIGÉ dans
  `scripts/ci-sans-materiel.sh` (commit local, non poussé).
- ROUGE « UEFI » : écran tout noir, rien sur la persistance. À diagnostiquer :
  `gh run download 44156326449 -n grenos-linux-captures`, lire
  `serial-uefi.log` et `uefi-juge.log`. Soupçon : grenos-ui (WebKit) sur le
  framebuffer UEFI de QEMU ; le lanceur rebascule sur Python après 120 s — voir
  si la ligne « premier : grenos-ui ne s est pas affiche » est présente.
- Ensuite : pousser la branche (fix sans-matériel + logo + fond), attendre la CI,
  puis fusionner et publier comme ci-dessous.

## 1. Demandé le 17 novembre au soir — le visuel

1. **Logo grenOS sans lettre** — [écrit], commit local `visuel` : une pousse
   (deux feuilles + un point de lumière) dans `icones.py` (`marque`, `feuille`),
   les icônes de grenos-ui, le SVG `marque` de `web/commun.js` (logos de
   l'OOBE) et `marque-pastille` du site. Reste : vérifier sur capture QEMU.
2. **Fond d'écran** — VALIDÉ PAR GRENOFAR (commit local `4824a10`, « garde
   ça ») : nuit = noir bleuté, indigo discret en haut à gauche, violet en bas
   à droite, feuille géante en filigrane ; jour = lavande pâle, feuille tracée
   en violet. NE PLUS LE CHANGER sans qu'il le demande.
3. **Barres de titre des fenêtres** — [écrit], commit local `cff5b47` (nuit
   du 17 au 18) : démon Rust `linux/coins` (`grenos-coins`, x11rb, sans
   WebKit, 4 essais `cargo test`) lancé par `grenos-session` sous
   `grenos-veilleur` ; masque SHAPE de 10 px sur chaque cadre d'openbox,
   reposé quand openbox le remet à zéro (ShapeNotify « sans forme ») ; ignore
   agrandies / plein écran / DOCK / DESKTOP / fenêtres déjà formées. Compilé
   par la CI dans la même étape que grenos-ui (`.github/workflows/linux.yml`),
   binaire ignoré par git. `themerc` : bordure 0, padding 10/9, barre
   `#131b2e`, ronds 12 px façon feux (fermer `#ef4d5e`, réduire `#fbbf24`,
   agrandir `#2bc48a`), glyphe creusé au survol (`boutons-fenetre.py`,
   `--apercu` pour les relire en texte). RESTE : le voir sur capture QEMU
   (la CI de la branche le montrera) ; si openbox ignorait les couleurs par
   bouton (`window.active.button.close.unpressed.image.color`), les ronds
   seraient tous gris clair.
4. **Animations** — [écrit], commit local `0cf0e1e` : les fenêtres de
   grenos-ui naissent du fond (180 ms) et s'y effacent (140 ms) ; Rust retient
   la fermeture d'openbox, ajoute `.fermeture`, détruit après le fondu ; rien
   en « performance » ; un essai vérifie que Rust et `style.css` disent la
   même durée. Openbox anime déjà la réduction (`animateIconify`). RESTE : les
   menus et fenêtres GTK (Python) — sans compositeur, seule piste : animer le
   contenu, pas la fenêtre ; à faire quand barre/bureau passeront en gtk-rs.
5. Ne pas émettre de son pendant les essais (demande explicite).

## 2. Ensuite — plus aucun processus Python au quotidien

- **Barre (`grenos-shell`) et bureau (`grenos-bureau`) en Rust NATIF (gtk-rs)**,
  pas en Tauri : ils tournent en permanence, WebKit coûterait plus que Python.
- Les autres fenêtres (Fichiers, Réglages, GrenPlace, Tâches, Son, Connexions,
  grenHub, Jeux, Bienvenue, Secours, Arrêt) deviennent des pages de grenos-ui.
- Les assistants Python de secours (premier démarrage, maj) restent livrés.

## 3. Reste de `todo.md`

Section I (OOBE) : I8 mode jeu Ctrl+Alt+Échap, I13 sauvegardes, I17 écrans
Dev. Section K3 constructions reproductibles. Voir `todo.md`.

## Outils à disposition

- Depuis le 23 novembre, Grenofar est SUR grenOS : MCP `grenos` (captures,
  curseur orange de Claude, souris, clavier, fenêtres, bash). Arrêt
  d'urgence : pointeur dans le coin en haut à gauche.
- (Windows, avant) MCP `powershell-local` : captures d'écran et contrôle du PC (navigateur,
  VirtualBox pour essayer l'ISO). **Aucun son.**
- Aperçu des pages grenos-ui : `.claude/launch.json` → `grenos-ui-apercu`
  (port 8765, données factices dans `linux/ui/web/apercu.js`).
- `linux/tools/catalogue-jeux.py` régénère le catalogue de GrenLaunch.
